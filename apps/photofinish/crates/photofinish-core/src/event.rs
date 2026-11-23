use chrono::Utc;
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use walkdir::WalkDir;

use crate::db::{
    backups_dir, bundle_dir, event_db_path, is_event_bundle, open_ro_connection,
    open_rw_connection, previews_dir, run_migrations, thumbnails_dir, EVENT_MIGRATIONS,
};
use crate::id;
use crate::orders::{self, CreateOrderRequest, OrderRecord, ProductRecord};
use crate::preset::{
    extract_subject_number, find_preset, parse_relative_location, scaffold_folders, ScaffoldConfig,
};
use crate::roster::{
    natural_sort_key, AssignFolderResult, FolderParentSummary, ImportRosterResult,
    RosterEntryInput, UnassignedFolder, UnassignedFoldersResponse,
};
use crate::{Error, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateEventParams {
    pub root_path: PathBuf,
    pub name: String,
    pub description: Option<String>,
    pub preset_id: Option<String>,
    pub subject_label: Option<String>,
    pub folder_levels: Option<Vec<String>>,
    pub order_code: Option<String>,
    pub station_code: Option<String>,
    pub tax_rate_basis_points: Option<i64>,
    pub scaffold: Option<ScaffoldConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventSummary {
    pub id: String,
    pub root_path: String,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub status: String,
    pub preset_id: String,
    pub subject_label: String,
    pub folder_levels: Vec<String>,
    pub order_code: String,
    pub station_code: String,
    pub tax_rate_basis_points: i64,
    pub photo_counts: PhotoCounts,
    pub subject_count: i64,
    pub order_count: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotoCounts {
    pub total: i64,
    pub ready: i64,
    pub pending: i64,
    pub error: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubjectRecord {
    pub id: String,
    pub number: Option<String>,
    pub first_name: String,
    pub last_name: Option<String>,
    pub display_name: String,
    pub group_label: Option<String>,
    pub team_name: Option<String>,
    pub photo_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotoRecord {
    pub id: String,
    pub subject_id: Option<String>,
    pub relative_path: String,
    pub folder_path: String,
    pub leaf_folder: Option<String>,
    pub filename: String,
    pub status: String,
    pub thumbnail_rel_path: Option<String>,
    pub preview_rel_path: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    /// Sport-agnostic grouping section (e.g. "Beam"/"Floor" for gymnastics, "Routine" for dance).
    pub section_label: String,
    pub tags: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingPhotoJob {
    pub photo_id: String,
    pub relative_path: String,
}

#[derive(Debug, Clone)]
pub enum PhotoProcessUpdate {
    Ready {
        photo_id: String,
        thumbnail_rel_path: String,
        preview_rel_path: String,
        width: u32,
        height: u32,
    },
    Error {
        photo_id: String,
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub photos_found: usize,
    pub photos_new: usize,
    pub photos_skipped: usize,
    pub pending_jobs: Vec<PendingPhotoJob>,
}

#[derive(Debug, Clone)]
pub struct IngestFileInput {
    pub relative_path: String,
    pub original_filename: String,
    pub file_size_bytes: i64,
}

#[derive(Debug, Clone)]
pub struct IngestBatchMeta {
    pub reader_id: Option<String>,
    pub reader_name: Option<String>,
    pub volume_name: Option<String>,
    pub photographer: Option<String>,
    pub source_path: Option<String>,
    pub destination_rel_path: String,
    pub tags: BTreeMap<String, String>,
}

/// Handle to a portable PhotoFinish Event Bundle (`<root>/_photofinish/photofinish.db`).
/// Writes serialize cleanly through `write_conn`; concurrent LAN reads open short-lived WAL connections.
pub struct EventBundle {
    root: PathBuf,
    db_path: PathBuf,
    write_conn: Mutex<Connection>,
}

impl EventBundle {
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    pub fn read_conn(&self) -> Result<Connection> {
        open_ro_connection(&self.db_path)
    }

    /// Creates a brand-new portable Event Bundle at `params.root_path`.
    pub fn create(params: CreateEventParams) -> Result<Self> {
        let name = params.name.trim();
        if name.is_empty() {
            return Err(Error::InvalidInput("Event name cannot be empty".into()));
        }

        fs::create_dir_all(&params.root_path)
            .map_err(|e| Error::Io(format!("{}: {}", params.root_path.display(), e)))?;
        fs::create_dir_all(bundle_dir(&params.root_path))
            .map_err(|e| Error::Io(format!("create _photofinish: {}", e)))?;
        fs::create_dir_all(thumbnails_dir(&params.root_path))
            .map_err(|e| Error::Io(format!("create thumbnails dir: {}", e)))?;
        fs::create_dir_all(previews_dir(&params.root_path))
            .map_err(|e| Error::Io(format!("create previews dir: {}", e)))?;

        let db_path = event_db_path(&params.root_path);
        let mut conn = open_rw_connection(&db_path)?;
        run_migrations(
            &mut conn,
            EVENT_MIGRATIONS,
            Some(&backups_dir(&params.root_path)),
            "photofinish",
        )?;

        let preset_id = params.preset_id.unwrap_or_else(|| "gymnastics".to_string());
        let preset = find_preset(&preset_id);

        let subject_label = params
            .subject_label
            .filter(|s| !s.trim().is_empty())
            .or_else(|| preset.as_ref().map(|p| p.subject_label.clone()))
            .unwrap_or_else(|| "Participant".to_string());

        let folder_levels = params
            .folder_levels
            .filter(|v| !v.is_empty())
            .or_else(|| preset.as_ref().map(|p| p.folder_levels.clone()))
            .unwrap_or_else(|| vec!["Group".to_string(), "Subject".to_string()]);

        let slug = slugify(name);
        let order_code = params
            .order_code
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.trim().to_uppercase())
            .unwrap_or_else(|| default_order_code(name));

        let station_code = params
            .station_code
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.trim().to_uppercase())
            .unwrap_or_else(|| "A".to_string());

        let tax_rate_basis_points = params.tax_rate_basis_points.unwrap_or(850);
        let preset_config_json = serde_json::to_string(&params.scaffold.clone().unwrap_or_default())?;
        let folder_levels_json = serde_json::to_string(&folder_levels)?;
        let now = Utc::now().to_rfc3339();
        let event_id = id::event_id();

        conn.execute(
            "INSERT INTO event_config (
                id, name, slug, description, status, preset_id, subject_label,
                folder_levels_json, preset_config_json, order_code, station_code,
                next_order_number, tax_rate_basis_points, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, 'active', ?5, ?6, ?7, ?8, ?9, ?10, 0, ?11, ?12, ?13)",
            rusqlite::params![
                event_id,
                name,
                slug,
                params.description,
                preset_id,
                subject_label,
                folder_levels_json,
                preset_config_json,
                order_code,
                station_code,
                tax_rate_basis_points,
                now,
                now,
            ],
        )?;

        seed_default_products(&conn, &now)?;

        if let Some(ref scaffold) = params.scaffold {
            scaffold_folders(&params.root_path, scaffold)?;
        }

        Ok(Self {
            root: params.root_path,
            db_path,
            write_conn: Mutex::new(conn),
        })
    }

    /// Opens an existing portable Event Bundle, running safe auto-backed-up migrations if needed.
    pub fn open(root_path: impl AsRef<Path>) -> Result<Self> {
        let root = root_path.as_ref().to_path_buf();
        if !is_event_bundle(&root) {
            return Err(Error::NotAnEventBundle(root.display().to_string()));
        }

        fs::create_dir_all(thumbnails_dir(&root))
            .map_err(|e| Error::Io(format!("create thumbnails dir: {}", e)))?;
        fs::create_dir_all(previews_dir(&root))
            .map_err(|e| Error::Io(format!("create previews dir: {}", e)))?;

        let db_path = event_db_path(&root);
        let mut conn = open_rw_connection(&db_path)?;
        run_migrations(
            &mut conn,
            EVENT_MIGRATIONS,
            Some(&backups_dir(&root)),
            "photofinish",
        )?;

        Ok(Self {
            root,
            db_path,
            write_conn: Mutex::new(conn),
        })
    }

    /// Discovers all PhotoFinish Event Bundles inside a parent folder (e.g. `/NAS/2026-Season/`).
    /// Checks both `parent` itself and immediate/nested child directories without descending inside bundles.
    pub fn discover_in_directory(parent: impl AsRef<Path>) -> Result<Vec<EventSummary>> {
        let parent = parent.as_ref();
        if !parent.is_dir() {
            return Err(Error::InvalidInput(format!(
                "Not a directory: {}",
                parent.display()
            )));
        }

        let mut bundle_roots = Vec::new();
        if is_event_bundle(parent) {
            bundle_roots.push(parent.to_path_buf());
        } else {
            for entry in fs::read_dir(parent)
                .map_err(|e| Error::Io(format!("{}: {}", parent.display(), e)))?
                .flatten()
            {
                let path = entry.path();
                if path.is_dir() && is_event_bundle(&path) {
                    bundle_roots.push(path);
                }
            }
        }

        bundle_roots.sort();
        let mut summaries = Vec::new();
        for root in bundle_roots {
            if let Ok(bundle) = EventBundle::open(&root) {
                if let Ok(summary) = bundle.summary() {
                    summaries.push(summary);
                }
            }
        }

        Ok(summaries)
    }

    pub fn summary(&self) -> Result<EventSummary> {
        let conn = self.read_conn()?;
        let (
            id,
            name,
            slug,
            description,
            status,
            preset_id,
            subject_label,
            folder_levels_json,
            order_code,
            station_code,
            tax_rate_basis_points,
            created_at,
        ): (
            String,
            String,
            String,
            Option<String>,
            String,
            String,
            String,
            String,
            String,
            String,
            i64,
            String,
        ) = conn.query_row(
            "SELECT id, name, slug, description, status, preset_id, subject_label,
                    folder_levels_json, order_code, station_code, tax_rate_basis_points, created_at
             FROM event_config LIMIT 1",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                    row.get(10)?,
                    row.get(11)?,
                ))
            },
        )?;

        let folder_levels: Vec<String> = serde_json::from_str(&folder_levels_json)?;
        let photo_counts = query_photo_counts(&conn)?;
        let subject_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM subjects WHERE is_active = 1", [], |r| {
                r.get(0)
            })?;
        let order_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM orders", [], |r| r.get(0))?;

        Ok(EventSummary {
            id,
            root_path: self.root.to_string_lossy().to_string(),
            name,
            slug,
            description,
            status,
            preset_id,
            subject_label,
            folder_levels,
            order_code,
            station_code,
            tax_rate_basis_points,
            photo_counts,
            subject_count,
            order_count,
            created_at,
        })
    }

    pub fn photo_counts(&self) -> Result<PhotoCounts> {
        let conn = self.read_conn()?;
        query_photo_counts(&conn)
    }

    pub fn add_subject(
        &self,
        number: Option<&str>,
        first_name: &str,
        last_name: Option<&str>,
        group_label: Option<&str>,
        team_name: Option<&str>,
    ) -> Result<SubjectRecord> {
        let conn = self.write_conn.lock().unwrap();
        let subject_id = id::subject_id();
        let now = Utc::now().to_rfc3339();
        let display_name = build_display_name(number, first_name, last_name);

        conn.execute(
            "INSERT INTO subjects (
                id, number, first_name, last_name, display_name,
                group_label, team_name, is_active, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?8, ?9)",
            rusqlite::params![
                subject_id,
                number,
                first_name,
                last_name,
                display_name,
                group_label,
                team_name,
                now,
                now,
            ],
        )?;

        Ok(SubjectRecord {
            id: subject_id,
            number: number.map(str::to_string),
            first_name: first_name.to_string(),
            last_name: last_name.map(str::to_string),
            display_name,
            group_label: group_label.map(str::to_string),
            team_name: team_name.map(str::to_string),
            photo_count: 0,
        })
    }

    /// Bulk-imports subjects (upserting by `number` when present) and immediately
    /// auto-matches any previously unassigned photo folders whose leading number
    /// matches a subject in the roster.
    pub fn import_subjects(&self, entries: &[RosterEntryInput]) -> Result<ImportRosterResult> {
        let mut conn = self.write_conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        let tx = conn.transaction()?;

        let mut imported_new = 0usize;
        let mut updated = 0usize;

        {
            let mut find_by_num = tx.prepare("SELECT id FROM subjects WHERE number = ?1")?;
            let mut update_stmt = tx.prepare(
                "UPDATE subjects
                 SET first_name = ?2, last_name = ?3, display_name = ?4,
                     group_label = COALESCE(?5, group_label),
                     team_name = COALESCE(?6, team_name),
                     external_id = COALESCE(?7, external_id),
                     is_active = 1, updated_at = ?8
                 WHERE id = ?1",
            )?;
            let mut insert_stmt = tx.prepare(
                "INSERT INTO subjects (
                    id, number, first_name, last_name, display_name,
                    group_label, team_name, external_id, is_active, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, ?10)",
            )?;

            for entry in entries {
                let first = entry.first_name.trim();
                if first.is_empty() {
                    continue;
                }
                let num = entry.number.as_deref().map(str::trim).filter(|s| !s.is_empty());
                let last = entry.last_name.as_deref().map(str::trim).filter(|s| !s.is_empty());
                let group = entry.group_label.as_deref().map(str::trim).filter(|s| !s.is_empty());
                let team = entry.team_name.as_deref().map(str::trim).filter(|s| !s.is_empty());
                let ext = entry.external_id.as_deref().map(str::trim).filter(|s| !s.is_empty());
                let display_name = build_display_name(num, first, last);

                let existing_id: Option<String> = match num {
                    Some(n) => find_by_num.query_row([n], |r| r.get(0)).optional()?,
                    None => None,
                };

                if let Some(id) = existing_id {
                    update_stmt.execute(rusqlite::params![
                        id,
                        first,
                        last,
                        display_name,
                        group,
                        team,
                        ext,
                        now,
                    ])?;
                    updated += 1;
                } else {
                    insert_stmt.execute(rusqlite::params![
                        id::subject_id(),
                        num,
                        first,
                        last,
                        display_name,
                        group,
                        team,
                        ext,
                        now,
                        now,
                    ])?;
                    imported_new += 1;
                }
            }
        }

        let auto_matched_photos = auto_match_unassigned_in_tx(&tx, &now)?;
        tx.commit()?;

        Ok(ImportRosterResult {
            imported_new,
            updated,
            auto_matched_photos,
        })
    }

    /// Scans all unassigned photos (`subject_id IS NULL`) and links any folder whose
    /// `leaf_folder` begins with a matching subject number (e.g. `"105 Julia B"` -> `"105"`).
    pub fn auto_match_unassigned_photos(&self) -> Result<usize> {
        let mut conn = self.write_conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        let tx = conn.transaction()?;
        let count = auto_match_unassigned_in_tx(&tx, &now)?;
        tx.commit()?;
        Ok(count)
    }

    pub fn list_subject_groups(&self) -> Result<Vec<String>> {
        let conn = self.read_conn()?;
        let mut stmt = conn.prepare(
            "SELECT DISTINCT group_label FROM subjects
             WHERE is_active = 1 AND group_label IS NOT NULL AND TRIM(group_label) != ''",
        )?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut groups: Vec<String> = rows.filter_map(|r| r.ok()).collect();
        groups.sort_by_key(|g| natural_sort_key(g));
        Ok(groups)
    }

    pub fn list_subjects(
        &self,
        query: Option<&str>,
        group_label: Option<&str>,
        limit: i64,
    ) -> Result<Vec<SubjectRecord>> {
        let conn = self.read_conn()?;
        let trimmed = query.unwrap_or("").trim();
        let pattern = format!("%{}%", trimmed);
        let group = group_label.map(str::trim).filter(|s| !s.is_empty());

        let mut stmt = conn.prepare(
            "SELECT s.id, s.number, s.first_name, s.last_name, s.display_name,
                    s.group_label, s.team_name,
                    COUNT(CASE WHEN p.status = 'ready' THEN p.id END) AS photo_count
             FROM subjects s
             LEFT JOIN photos p ON p.subject_id = s.id
             WHERE s.is_active = 1
               AND (?1 IS NULL OR s.group_label = ?1)
               AND (?2 = ''
                    OR s.display_name LIKE ?3 COLLATE NOCASE
                    OR COALESCE(s.number, '') LIKE ?3 COLLATE NOCASE
                    OR COALESCE(s.team_name, '') LIKE ?3 COLLATE NOCASE)
             GROUP BY s.id
             LIMIT ?4",
        )?;
        let rows = stmt.query_map(rusqlite::params![group, trimmed, pattern, limit], |r| {
            Ok(SubjectRecord {
                id: r.get(0)?,
                number: r.get(1)?,
                first_name: r.get(2)?,
                last_name: r.get(3)?,
                display_name: r.get(4)?,
                group_label: r.get(5)?,
                team_name: r.get(6)?,
                photo_count: r.get(7)?,
            })
        })?;
        let mut out: Vec<SubjectRecord> = rows.collect::<std::result::Result<_, _>>()?;
        out.sort_by_key(|s| natural_sort_key(&s.display_name));
        Ok(out)
    }

    /// Lists unassigned folders (`subject_id IS NULL`) grouped by their parent folder path,
    /// including up to 4 sample photo IDs per folder for visual thumbnail matching.
    pub fn list_unassigned_folders(
        &self,
        parent_filter: Option<&str>,
    ) -> Result<UnassignedFoldersResponse> {
        let conn = self.read_conn()?;
        let mut stmt = conn.prepare(
            "SELECT folder_path, COALESCE(leaf_folder, folder_path), COUNT(*)
             FROM photos
             WHERE subject_id IS NULL AND folder_path != ''
             GROUP BY folder_path, leaf_folder",
        )?;
        let raw_folders: Vec<(String, String, i64)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<std::result::Result<_, _>>()?;

        let mut parent_map: BTreeMap<String, (usize, i64)> = BTreeMap::new();
        let mut total_unassigned_photos = 0i64;
        let mut filtered = Vec::new();

        let filter_norm = parent_filter.map(str::trim).filter(|s| !s.is_empty());

        for (folder_path, leaf_folder, count) in raw_folders {
            total_unassigned_photos += count;
            let parent_path = folder_path
                .rsplit_once('/')
                .map(|(p, _)| p.to_string())
                .unwrap_or_default();

            let entry = parent_map.entry(parent_path.clone()).or_insert((0, 0));
            entry.0 += 1;
            entry.1 += count;

            if filter_norm.map(|f| parent_path == f).unwrap_or(true) {
                filtered.push((folder_path, parent_path, leaf_folder, count));
            }
        }

        let mut parents: Vec<FolderParentSummary> = parent_map
            .into_iter()
            .map(|(parent_path, (folder_count, photo_count))| FolderParentSummary {
                parent_path,
                folder_count,
                photo_count,
            })
            .collect();
        parents.sort_by_key(|p| natural_sort_key(&p.parent_path));

        filtered.sort_by_key(|(fp, _, _, _)| natural_sort_key(fp));

        let mut sample_stmt = conn.prepare(
            "SELECT id FROM photos
             WHERE folder_path = ?1 AND subject_id IS NULL AND status = 'ready'
             ORDER BY filename ASC
             LIMIT 4",
        )?;

        let mut folders = Vec::with_capacity(filtered.len());
        for (folder_path, parent_path, leaf_folder, photo_count) in filtered {
            let sample_photo_ids: Vec<String> = sample_stmt
                .query_map([&folder_path], |r| r.get(0))?
                .filter_map(|r| r.ok())
                .collect();
            folders.push(UnassignedFolder {
                folder_path,
                parent_path,
                leaf_folder,
                photo_count,
                sample_photo_ids,
            });
        }

        Ok(UnassignedFoldersResponse {
            parents,
            folders,
            total_unassigned_photos,
        })
    }

    /// Assigns all unassigned photos in `folder_path` to `subject_id`.
    /// When `rename_on_disk` is true, physically renames (or safely merges into)
    /// the folder named after the subject's `display_name` and updates relative paths in SQLite.
    pub fn assign_folder_to_subject(
        &self,
        folder_path: &str,
        subject_id: &str,
        rename_on_disk: bool,
    ) -> Result<AssignFolderResult> {
        let mut conn = self.write_conn.lock().unwrap();
        let display_name: String = conn
            .query_row(
                "SELECT display_name FROM subjects WHERE id = ?1",
                [subject_id],
                |r| r.get(0),
            )
            .optional()?
            .ok_or_else(|| Error::NotFound(format!("Subject {} not found", subject_id)))?;

        let sanitized_name = sanitize_folder_component(&display_name);
        let (parent_path, current_leaf) = match folder_path.rsplit_once('/') {
            Some((p, l)) => (p, l),
            None => ("", folder_path),
        };

        let target_folder_path = if parent_path.is_empty() {
            sanitized_name.clone()
        } else {
            format!("{}/{}", parent_path, sanitized_name)
        };

        let mut did_rename = false;
        let old_abs = self.root.join(folder_path);
        let new_abs = self.root.join(&target_folder_path);

        if rename_on_disk && current_leaf != sanitized_name && old_abs.is_dir() {
            if !new_abs.exists() {
                fs::rename(&old_abs, &new_abs).map_err(|e| {
                    Error::Io(format!(
                        "Failed to rename {} -> {}: {}",
                        old_abs.display(),
                        new_abs.display(),
                        e
                    ))
                })?;
                did_rename = true;
            } else if new_abs.is_dir() {
                // Target subject folder already exists (e.g. photographer split one athlete across 2 folders):
                // safely move individual files into target folder and remove empty source folder.
                for entry in fs::read_dir(&old_abs)
                    .map_err(|e| Error::Io(format!("{}: {}", old_abs.display(), e)))?
                    .flatten()
                {
                    let src_file = entry.path();
                    if src_file.is_file() {
                        let dst_file = new_abs.join(entry.file_name());
                        if !dst_file.exists() {
                            fs::rename(&src_file, &dst_file).map_err(|e| {
                                Error::Io(format!(
                                    "Failed to move {} -> {}: {}",
                                    src_file.display(),
                                    dst_file.display(),
                                    e
                                ))
                            })?;
                        }
                    }
                }
                let _ = fs::remove_dir(&old_abs);
                did_rename = true;
            }
        }

        let final_folder_path = if did_rename {
            target_folder_path
        } else {
            folder_path.to_string()
        };
        let final_leaf = if did_rename {
            sanitized_name
        } else {
            current_leaf.to_string()
        };

        let now = Utc::now().to_rfc3339();
        let tx = conn.transaction()?;

        let photos_updated = if did_rename {
            let old_prefix = format!("{}/", folder_path);
            let new_prefix = format!("{}/", final_folder_path);
            let rows: Vec<(String, String)> = {
                let mut stmt = tx.prepare(
                    "SELECT id, relative_path FROM photos
                     WHERE folder_path = ?1 AND subject_id IS NULL",
                )?;
                let collected = stmt
                    .query_map([folder_path], |r| Ok((r.get(0)?, r.get(1)?)))?
                    .collect::<std::result::Result<_, _>>()?;
                collected
            };
            let mut upd = tx.prepare(
                "UPDATE photos
                 SET subject_id = ?2, relative_path = ?3, folder_path = ?4,
                     leaf_folder = ?5, updated_at = ?6
                 WHERE id = ?1",
            )?;
            let mut count = 0usize;
            for (pid, rel) in rows {
                let new_rel = if let Some(rest) = rel.strip_prefix(&old_prefix) {
                    format!("{}{}", new_prefix, rest)
                } else {
                    rel
                };
                count += upd.execute(rusqlite::params![
                    pid,
                    subject_id,
                    new_rel,
                    final_folder_path,
                    final_leaf,
                    now,
                ])?;
            }
            count
        } else {
            tx.execute(
                "UPDATE photos SET subject_id = ?2, updated_at = ?3
                 WHERE folder_path = ?1 AND subject_id IS NULL",
                rusqlite::params![folder_path, subject_id, now],
            )?
        };

        tx.commit()?;

        Ok(AssignFolderResult {
            photos_updated,
            new_folder_path: final_folder_path,
            renamed_on_disk: did_rename,
        })
    }

    /// Records a card-ingest batch and inserts all newly copied photos in one transaction.
    /// Returns the list of newly created photos needing thumbnail/preview generation.
    pub fn record_ingest_batch(
        &self,
        meta: IngestBatchMeta,
        files: &[IngestFileInput],
    ) -> Result<(String, Vec<PendingPhotoJob>)> {
        let mut conn = self.write_conn.lock().unwrap();
        let folder_levels = load_folder_levels(&conn)?;
        let subject_lookup = load_subject_number_map(&conn)?;

        let next_seq: i64 = conn.query_row(
            "SELECT COALESCE(MAX(sequence_num), 0) + 1 FROM ingest_batches",
            [],
            |r| r.get(0),
        )?;

        let batch_id = id::ingest_batch_id();
        let now = Utc::now().to_rfc3339();
        let batch_tags_json = serde_json::to_string(&meta.tags)?;

        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO ingest_batches (
                id, sequence_num, reader_id, reader_name, volume_name, photographer,
                source_path, destination_rel_path, tags_json, file_count, started_at, completed_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            rusqlite::params![
                batch_id,
                next_seq,
                meta.reader_id,
                meta.reader_name,
                meta.volume_name,
                meta.photographer,
                meta.source_path,
                meta.destination_rel_path,
                batch_tags_json,
                files.len() as i64,
                now,
                now,
            ],
        )?;

        let mut pending = Vec::new();
        {
            let mut stmt = tx.prepare(
                "INSERT OR IGNORE INTO photos (
                    id, subject_id, ingest_batch_id, relative_path, folder_path, leaf_folder,
                    filename, original_filename, file_size_bytes, tags_json, photographer,
                    status, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 'discovered', ?12, ?13)",
            )?;

            for f in files {
                let parsed = parse_relative_location(&f.relative_path, &folder_levels);
                let mut merged_tags = parsed.tags.clone();
                for (k, v) in &meta.tags {
                    merged_tags.insert(k.clone(), v.clone());
                }
                let subject_id = parsed
                    .leaf_folder
                    .as_deref()
                    .and_then(extract_subject_number)
                    .and_then(|num| subject_lookup.get(&num).cloned());

                let photo_id = id::photo_id();
                let tags_json = serde_json::to_string(&merged_tags)?;
                let changed = stmt.execute(rusqlite::params![
                    photo_id,
                    subject_id,
                    batch_id,
                    f.relative_path,
                    parsed.folder_path,
                    parsed.leaf_folder,
                    parsed.filename,
                    f.original_filename,
                    f.file_size_bytes,
                    tags_json,
                    meta.photographer,
                    now,
                    now,
                ])?;

                if changed > 0 {
                    pending.push(PendingPhotoJob {
                        photo_id,
                        relative_path: f.relative_path.clone(),
                    });
                }
            }
        }

        tx.commit()?;
        Ok((batch_id, pending))
    }

    /// Walks the event folder for any JPEGs not yet indexed (safety net for manual Finder drops).
    /// Always skips `_`-prefixed directories (including `_photofinish`) and hidden dotfiles.
    pub fn scan_directory(&self) -> Result<ScanResult> {
        let mut discovered = Vec::new();
        for entry in WalkDir::new(&self.root)
            .into_iter()
            .filter_entry(|e| {
                if e.depth() == 0 {
                    return true;
                }
                let name = e.file_name().to_string_lossy();
                if name.starts_with('.') || name.starts_with('_') {
                    return false;
                }
                true
            })
            .filter_map(|e| e.ok())
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let is_jpeg = entry
                .path()
                .extension()
                .map(|ext| {
                    let lower = ext.to_string_lossy().to_lowercase();
                    lower == "jpg" || lower == "jpeg"
                })
                .unwrap_or(false);
            if !is_jpeg {
                continue;
            }

            if let Ok(rel) = entry.path().strip_prefix(&self.root) {
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                let size = entry.metadata().map(|m| m.len() as i64).unwrap_or(0);
                let orig_name = entry.file_name().to_string_lossy().to_string();
                discovered.push(IngestFileInput {
                    relative_path: rel_str,
                    original_filename: orig_name,
                    file_size_bytes: size,
                });
            }
        }

        let photos_found = discovered.len();
        let mut conn = self.write_conn.lock().unwrap();
        let folder_levels = load_folder_levels(&conn)?;
        let subject_lookup = load_subject_number_map(&conn)?;

        let existing: HashSet<String> = {
            let mut stmt = conn.prepare("SELECT relative_path FROM photos")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
            rows.filter_map(|r| r.ok()).collect()
        };

        let now = Utc::now().to_rfc3339();
        let tx = conn.transaction()?;
        let mut pending = Vec::new();
        let mut photos_skipped = 0;

        {
            let mut stmt = tx.prepare(
                "INSERT INTO photos (
                    id, subject_id, relative_path, folder_path, leaf_folder,
                    filename, original_filename, file_size_bytes, tags_json,
                    status, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'discovered', ?10, ?11)",
            )?;

            for f in discovered {
                if existing.contains(&f.relative_path) {
                    photos_skipped += 1;
                    continue;
                }
                let parsed = parse_relative_location(&f.relative_path, &folder_levels);
                let subject_id = parsed
                    .leaf_folder
                    .as_deref()
                    .and_then(extract_subject_number)
                    .and_then(|num| subject_lookup.get(&num).cloned());
                let photo_id = id::photo_id();
                let tags_json = serde_json::to_string(&parsed.tags)?;
                stmt.execute(rusqlite::params![
                    photo_id,
                    subject_id,
                    f.relative_path,
                    parsed.folder_path,
                    parsed.leaf_folder,
                    parsed.filename,
                    f.original_filename,
                    f.file_size_bytes,
                    tags_json,
                    now,
                    now,
                ])?;
                pending.push(PendingPhotoJob {
                    photo_id,
                    relative_path: f.relative_path,
                });
            }
        }

        tx.commit()?;

        Ok(ScanResult {
            photos_found,
            photos_new: pending.len(),
            photos_skipped,
            pending_jobs: pending,
        })
    }

    pub fn list_unprocessed_photos(&self) -> Result<Vec<PendingPhotoJob>> {
        let conn = self.read_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, relative_path FROM photos WHERE status IN ('discovered', 'processing') ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(PendingPhotoJob {
                photo_id: r.get(0)?,
                relative_path: r.get(1)?,
            })
        })?;
        let mut jobs = Vec::new();
        for row in rows {
            jobs.push(row?);
        }
        Ok(jobs)
    }

    pub fn mark_photo_ready(
        &self,
        photo_id: &str,
        thumbnail_rel_path: &str,
        preview_rel_path: &str,
        width: u32,
        height: u32,
    ) -> Result<()> {
        let conn = self.write_conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE photos
             SET status = 'ready', thumbnail_rel_path = ?2, preview_rel_path = ?3,
                 width = ?4, height = ?5, error_message = NULL, updated_at = ?6
             WHERE id = ?1",
            rusqlite::params![
                photo_id,
                thumbnail_rel_path,
                preview_rel_path,
                width as i64,
                height as i64,
                now,
            ],
        )?;
        Ok(())
    }

    pub fn mark_photo_error(&self, photo_id: &str, message: &str) -> Result<()> {
        let conn = self.write_conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE photos SET status = 'error', error_message = ?2, updated_at = ?3 WHERE id = ?1",
            rusqlite::params![photo_id, message, now],
        )?;
        Ok(())
    }

    /// Commits a batch of completed or failed photo processing outcomes in a single
    /// SQLite transaction to eliminate per-photo write lock & WAL sync overhead.
    pub fn mark_photos_batch(&self, updates: &[PhotoProcessUpdate]) -> Result<()> {
        if updates.is_empty() {
            return Ok(());
        }
        let mut conn = self.write_conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        let tx = conn.transaction()?;
        {
            let mut ready_stmt = tx.prepare(
                "UPDATE photos
                 SET status = 'ready', thumbnail_rel_path = ?2, preview_rel_path = ?3,
                     width = ?4, height = ?5, error_message = NULL, updated_at = ?6
                 WHERE id = ?1",
            )?;
            let mut err_stmt = tx.prepare(
                "UPDATE photos SET status = 'error', error_message = ?2, updated_at = ?3 WHERE id = ?1",
            )?;

            for upd in updates {
                match upd {
                    PhotoProcessUpdate::Ready {
                        photo_id,
                        thumbnail_rel_path,
                        preview_rel_path,
                        width,
                        height,
                    } => {
                        ready_stmt.execute(rusqlite::params![
                            photo_id,
                            thumbnail_rel_path,
                            preview_rel_path,
                            *width as i64,
                            *height as i64,
                            now,
                        ])?;
                    }
                    PhotoProcessUpdate::Error { photo_id, message } => {
                        err_stmt.execute(rusqlite::params![photo_id, message, now])?;
                    }
                }
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn search_subjects(&self, query: &str, limit: i64) -> Result<Vec<SubjectRecord>> {
        let conn = self.read_conn()?;
        let trimmed = query.trim();
        let lower_q = trimmed.to_lowercase();
        let pattern = format!("%{}%", trimmed);
        let mut stmt = conn.prepare(
            "SELECT s.id, s.number, s.first_name, s.last_name, s.display_name,
                    s.group_label, s.team_name,
                    COUNT(CASE WHEN p.status = 'ready' THEN p.id END) AS photo_count
             FROM subjects s
             LEFT JOIN photos p ON p.subject_id = s.id
             WHERE s.is_active = 1
               AND (?1 = ''
                    OR s.display_name LIKE ?2 COLLATE NOCASE
                    OR COALESCE(s.number, '') LIKE ?2 COLLATE NOCASE
                    OR COALESCE(s.team_name, '') LIKE ?2 COLLATE NOCASE
                    OR COALESCE(s.group_label, '') LIKE ?2 COLLATE NOCASE)
             GROUP BY s.id",
        )?;
        let mut out: Vec<SubjectRecord> = stmt
            .query_map(rusqlite::params![trimmed, pattern], |r| {
                Ok(SubjectRecord {
                    id: r.get(0)?,
                    number: r.get(1)?,
                    first_name: r.get(2)?,
                    last_name: r.get(3)?,
                    display_name: r.get(4)?,
                    group_label: r.get(5)?,
                    team_name: r.get(6)?,
                    photo_count: r.get(7)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?;

        // Rank by relevance (exact number -> number prefix -> name prefix -> other)
        // and tie-break by natural numeric ordering (e.g. 105 before 1000).
        out.sort_by_key(|s| {
            let rank: u8 = if lower_q.is_empty() {
                0
            } else {
                let num_l = s.number.as_deref().unwrap_or("").to_lowercase();
                let first_l = s.first_name.to_lowercase();
                let last_l = s.last_name.as_deref().unwrap_or("").to_lowercase();
                if num_l == lower_q {
                    0
                } else if num_l.starts_with(&lower_q) {
                    1
                } else if first_l.starts_with(&lower_q) || last_l.starts_with(&lower_q) {
                    2
                } else {
                    3
                }
            };
            let numeric_bib = s
                .number
                .as_deref()
                .and_then(|n| n.parse::<u64>().ok())
                .unwrap_or(u64::MAX);
            (rank, numeric_bib, natural_sort_key(&s.display_name))
        });

        if limit > 0 && (out.len() as i64) > limit {
            out.truncate(limit as usize);
        }
        Ok(out)
    }

    pub fn get_subject(&self, subject_id: &str) -> Result<Option<SubjectRecord>> {
        let conn = self.read_conn()?;
        let row = conn
            .query_row(
                "SELECT s.id, s.number, s.first_name, s.last_name, s.display_name,
                        s.group_label, s.team_name,
                        COUNT(CASE WHEN p.status = 'ready' THEN p.id END) AS photo_count
                 FROM subjects s
                 LEFT JOIN photos p ON p.subject_id = s.id
                 WHERE s.id = ?1
                 GROUP BY s.id",
                [subject_id],
                |r| {
                    Ok(SubjectRecord {
                        id: r.get(0)?,
                        number: r.get(1)?,
                        first_name: r.get(2)?,
                        last_name: r.get(3)?,
                        display_name: r.get(4)?,
                        group_label: r.get(5)?,
                        team_name: r.get(6)?,
                        photo_count: r.get(7)?,
                    })
                },
            )
            .optional()?;
        Ok(row)
    }

    /// Returns a fast random sample of ready photo IDs for the kiosk home marquee.
    pub fn list_showcase_photo_ids(&self, count: i64) -> Result<Vec<String>> {
        let conn = self.read_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id FROM photos WHERE status = 'ready' ORDER BY RANDOM() LIMIT ?1",
        )?;
        let ids: Vec<String> = stmt
            .query_map([count.clamp(1, 60)], |r| r.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(ids)
    }

    pub fn list_products(&self, include_unavailable: bool) -> Result<Vec<ProductRecord>> {
        let conn = self.read_conn()?;
        orders::list_products(&conn, include_unavailable)
    }

    pub fn place_order(&self, req: CreateOrderRequest) -> Result<OrderRecord> {
        let mut conn = self.write_conn.lock().unwrap();
        orders::place_order(&mut conn, req)
    }

    pub fn get_order_by_number(&self, order_number: &str) -> Result<Option<OrderRecord>> {
        let conn = self.read_conn()?;
        orders::get_order_by_number(&conn, order_number)
    }

    pub fn list_subject_photos(&self, subject_id: &str) -> Result<Vec<PhotoRecord>> {
        let conn = self.read_conn()?;
        let folder_levels = load_folder_levels(&conn).unwrap_or_default();
        // Choose the level directly above "Subject" (e.g. "Apparatus" in gymnastics,
        // "Routine" in dance, "Group" in simple) for sport-agnostic section tabs.
        let section_level_key: Option<String> = if folder_levels.len() >= 2 {
            Some(folder_levels[folder_levels.len() - 2].clone())
        } else {
            folder_levels.first().cloned()
        };

        let mut stmt = conn.prepare(
            "SELECT id, subject_id, relative_path, folder_path, leaf_folder,
                    filename, status, thumbnail_rel_path, preview_rel_path,
                    width, height, tags_json
             FROM photos
             WHERE subject_id = ?1 AND status = 'ready'
             ORDER BY folder_path ASC, filename ASC",
        )?;
        let rows = stmt.query_map([subject_id], |r| {
            let folder_path: String = r.get(3)?;
            let tags_json: String = r.get(11)?;
            let tags: BTreeMap<String, String> =
                serde_json::from_str(&tags_json).unwrap_or_default();

            let section_label = section_level_key
                .as_ref()
                .and_then(|k| tags.get(k))
                .cloned()
                .or_else(|| {
                    // Fallback: parent folder immediately above the subject folder
                    folder_path
                        .rsplit_once('/')
                        .and_then(|(parent, _)| parent.rsplit('/').next())
                        .filter(|s| !s.is_empty())
                        .map(str::to_string)
                })
                .unwrap_or_else(|| "Photos".to_string());

            Ok(PhotoRecord {
                id: r.get(0)?,
                subject_id: r.get(1)?,
                relative_path: r.get(2)?,
                folder_path,
                leaf_folder: r.get(4)?,
                filename: r.get(5)?,
                status: r.get(6)?,
                thumbnail_rel_path: r.get(7)?,
                preview_rel_path: r.get(8)?,
                width: r.get(9)?,
                height: r.get(10)?,
                section_label,
                tags,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn get_photo_derivative_path(&self, photo_id: &str, preview: bool) -> Result<Option<PathBuf>> {
        let conn = self.read_conn()?;
        let col = if preview {
            "preview_rel_path"
        } else {
            "thumbnail_rel_path"
        };
        let sql = format!("SELECT {} FROM photos WHERE id = ?1", col);
        let rel: Option<Option<String>> = conn
            .query_row(&sql, [photo_id], |r| r.get(0))
            .optional()?;
        Ok(rel.flatten().map(|r| self.root.join(r)))
    }
}

fn query_photo_counts(conn: &Connection) -> Result<PhotoCounts> {
    let mut counts = PhotoCounts::default();
    let mut stmt = conn.prepare("SELECT status, COUNT(*) FROM photos GROUP BY status")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
    for row in rows {
        let (status, n) = row?;
        counts.total += n;
        match status.as_str() {
            "ready" => counts.ready += n,
            "error" => counts.error += n,
            _ => counts.pending += n,
        }
    }
    Ok(counts)
}

fn load_folder_levels(conn: &Connection) -> Result<Vec<String>> {
    let json: String =
        conn.query_row("SELECT folder_levels_json FROM event_config LIMIT 1", [], |r| {
            r.get(0)
        })?;
    Ok(serde_json::from_str(&json)?)
}

fn load_subject_number_map(conn: &Connection) -> Result<HashMap<String, String>> {
    let mut map = HashMap::new();
    let mut stmt = conn.prepare("SELECT id, number FROM subjects WHERE number IS NOT NULL")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    for row in rows {
        let (id, number) = row?;
        map.insert(number, id);
    }
    Ok(map)
}

/// Seeds the v3 ShootProof pricing ladder defaults (all customizable per event).
fn seed_default_products(conn: &Connection, now: &str) -> Result<()> {
    let defaults: &[(&str, &str, Option<&str>, i64, i64)] = &[
        ("single_digital", "Single Digital Photo", None, 2500, 0),
        ("full_digital", "All Photos Digital Package", None, 8500, 1),
        ("usb_addon", "Instant Dual USB-C/A Add-On", None, 1000, 2),
        ("print", "5x7 Lustre Print", Some("5x7"), 1800, 3),
        ("print", "8x10 Lustre Print", Some("8x10"), 2800, 4),
        ("print", "11x14 Print", Some("11x14"), 4000, 5),
        ("print", "16x20 Poster Print", Some("16x20"), 6500, 6),
    ];

    for (ptype, name, size, price, order) in defaults {
        conn.execute(
            "INSERT INTO products (
                id, product_type, name, size, price_cents, is_available,
                display_order, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7, ?8)",
            rusqlite::params![id::product_id(), ptype, name, size, price, order, now, now],
        )?;
    }
    Ok(())
}

fn build_display_name(number: Option<&str>, first_name: &str, last_name: Option<&str>) -> String {
    let name_part = match last_name.map(str::trim).filter(|s| !s.is_empty()) {
        Some(last) => format!("{} {}", first_name.trim(), last),
        None => first_name.trim().to_string(),
    };
    match number.map(str::trim).filter(|s| !s.is_empty()) {
        Some(num) => format!("{} {}", num, name_part),
        None => name_part,
    }
}

fn sanitize_folder_component(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

fn auto_match_unassigned_in_tx(tx: &rusqlite::Transaction<'_>, now: &str) -> Result<usize> {
    let subject_lookup = load_subject_number_map(tx)?;
    if subject_lookup.is_empty() {
        return Ok(0);
    }

    let unassigned_folders: Vec<(String, String)> = {
        let mut stmt = tx.prepare(
            "SELECT DISTINCT folder_path, leaf_folder FROM photos
             WHERE subject_id IS NULL AND leaf_folder IS NOT NULL",
        )?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<std::result::Result<_, _>>()?;
        rows
    };

    let mut matched_total = 0usize;
    let mut upd = tx.prepare(
        "UPDATE photos SET subject_id = ?2, updated_at = ?3
         WHERE folder_path = ?1 AND subject_id IS NULL",
    )?;

    for (folder_path, leaf) in unassigned_folders {
        if let Some(num) = extract_subject_number(&leaf) {
            if let Some(subject_id) = subject_lookup.get(&num) {
                matched_total += upd.execute(rusqlite::params![folder_path, subject_id, now])?;
            }
        }
    }

    Ok(matched_total)
}

fn slugify(input: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    out.trim_end_matches('-').to_string()
}

fn default_order_code(name: &str) -> String {
    let letters: String = name
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .take(3)
        .map(|c| c.to_ascii_uppercase())
        .collect();
    if letters.is_empty() {
        "EVT".to_string()
    } else {
        letters
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn creates_reopens_and_gathers_multiple_event_bundles() {
        let season_dir = tempdir().unwrap();

        let meet_a_dir = season_dir.path().join("2026-03-13_IL-Level-3");
        let meet_b_dir = season_dir.path().join("2026-03-13_IN-Level-4");

        let bundle_a = EventBundle::create(CreateEventParams {
            root_path: meet_a_dir.clone(),
            name: "IL Level 3 State".into(),
            description: None,
            preset_id: Some("gymnastics".into()),
            subject_label: None,
            folder_levels: None,
            order_code: Some("IL3".into()),
            station_code: Some("A".into()),
            tax_rate_basis_points: Some(850),
            scaffold: Some(ScaffoldConfig {
                num_areas: Some(1),
                area_prefix: Some("Gym".into()),
                sessions_per_area: Some(2),
                session_prefix: Some("Session".into()),
            }),
        })
        .unwrap();

        assert!(meet_a_dir.join("_photofinish/photofinish.db").is_file());
        assert!(meet_a_dir.join("Gym A/Session 1A").is_dir());

        let sub = bundle_a
            .add_subject(Some("1059"), "Iza", Some("Z"), Some("1A"), Some("Flippers"))
            .unwrap();
        assert_eq!(sub.display_name, "1059 Iza Z");

        // Record an ingest batch and confirm auto-match to subject 1059
        let (_batch_id, jobs) = bundle_a
            .record_ingest_batch(
                IngestBatchMeta {
                    reader_id: Some("USB-SERIAL-1".into()),
                    reader_name: Some("External USB Reader".into()),
                    volume_name: Some("EOS_DIGITAL".into()),
                    photographer: Some("KDS".into()),
                    source_path: Some("/Volumes/EOS_DIGITAL/DCIM".into()),
                    destination_rel_path: "Gym A/Session 1A/Group 2B/Beam".into(),
                    tags: BTreeMap::new(),
                },
                &[IngestFileInput {
                    relative_path: "Gym A/Session 1A/Group 2B/Beam/1059 Iza Z/IMG_001.jpg".into(),
                    original_filename: "IMG_001.jpg".into(),
                    file_size_bytes: 4096,
                }],
            )
            .unwrap();
        assert_eq!(jobs.len(), 1);
        bundle_a
            .mark_photo_ready(
                &jobs[0].photo_id,
                "_photofinish/thumbnails/test.jpg",
                "_photofinish/previews/test.jpg",
                1200,
                800,
            )
            .unwrap();

        let matches = bundle_a.search_subjects("1059", 10).unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].photo_count, 1);

        // Create second independent meet bundle (simulating Server B coming home)
        EventBundle::create(CreateEventParams {
            root_path: meet_b_dir,
            name: "IN Level 4 State".into(),
            description: None,
            preset_id: Some("dance".into()),
            subject_label: None,
            folder_levels: None,
            order_code: Some("IN4".into()),
            station_code: Some("B".into()),
            tax_rate_basis_points: None,
            scaffold: None,
        })
        .unwrap();

        // Gather both event bundles from the shared parent folder
        let gathered = EventBundle::discover_in_directory(season_dir.path()).unwrap();
        assert_eq!(gathered.len(), 2);
        assert_eq!(gathered[0].name, "IL Level 3 State");
        assert_eq!(gathered[0].photo_counts.ready, 1);
        assert_eq!(gathered[1].name, "IN Level 4 State");
        assert_eq!(gathered[1].subject_label, "Dancer");
    }

    #[test]
    fn imports_roster_auto_matches_and_assigns_folders_with_safe_merge() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("xcel-test");
        let bundle = EventBundle::create(CreateEventParams {
            root_path: root.clone(),
            name: "Xcel Test".into(),
            description: None,
            preset_id: Some("gymnastics".into()),
            subject_label: None,
            folder_levels: None,
            order_code: Some("XCL".into()),
            station_code: Some("A".into()),
            tax_rate_basis_points: None,
            scaffold: None,
        })
        .unwrap();

        // Create photos on disk BEFORE roster exists (one numbered, two camera-renamed)
        let p1 = root.join("Gym A/Session 1A/Group 1A/Beam/105 Julia B/IMG_1.jpg");
        let p2 = root.join("Gym A/Session 1A/Group 1A/Beam/Gymnast 05/IMG_2.jpg");
        let p3 = root.join("Gym A/Session 1A/Group 1A/Beam/Gymnast 06/IMG_3.jpg");
        for p in [&p1, &p2, &p3] {
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, b"fakejpg").unwrap();
        }
        let scan = bundle.scan_directory().unwrap();
        assert_eq!(scan.photos_new, 3);

        // Import roster AFTER scan: 105 Julia B should auto-match immediately!
        let entries = vec![
            RosterEntryInput {
                number: Some("105".into()),
                first_name: "Julia".into(),
                last_name: Some("B".into()),
                group_label: Some("Session 1A".into()),
                team_name: None,
                external_id: None,
            },
            RosterEntryInput {
                number: Some("143".into()),
                first_name: "Avery".into(),
                last_name: Some("W".into()),
                group_label: Some("Session 1A".into()),
                team_name: None,
                external_id: None,
            },
        ];
        let res = bundle.import_subjects(&entries).unwrap();
        assert_eq!(res.imported_new, 2);
        assert_eq!(res.updated, 0);
        assert_eq!(res.auto_matched_photos, 1);

        // Re-importing should upsert without duplicates
        let res2 = bundle.import_subjects(&entries).unwrap();
        assert_eq!(res2.imported_new, 0);
        assert_eq!(res2.updated, 2);

        // Two folders remain unassigned (Gymnast 05 and Gymnast 06)
        let unassigned = bundle.list_unassigned_folders(None).unwrap();
        assert_eq!(unassigned.folders.len(), 2);
        assert_eq!(unassigned.total_unassigned_photos, 2);

        let avery = bundle
            .list_subjects(Some("143"), None, 10)
            .unwrap()
            .pop()
            .unwrap();

        // Assign Gymnast 05 -> 143 Avery W (physical rename)
        let a1 = bundle
            .assign_folder_to_subject("Gym A/Session 1A/Group 1A/Beam/Gymnast 05", &avery.id, true)
            .unwrap();
        assert_eq!(a1.photos_updated, 1);
        assert!(a1.renamed_on_disk);
        assert!(root.join("Gym A/Session 1A/Group 1A/Beam/143 Avery W/IMG_2.jpg").is_file());

        // Assign Gymnast 06 -> SAME 143 Avery W (safe merge into existing dir!)
        let a2 = bundle
            .assign_folder_to_subject("Gym A/Session 1A/Group 1A/Beam/Gymnast 06", &avery.id, true)
            .unwrap();
        assert_eq!(a2.photos_updated, 1);
        assert!(a2.renamed_on_disk);
        assert!(root.join("Gym A/Session 1A/Group 1A/Beam/143 Avery W/IMG_3.jpg").is_file());

        let after = bundle.list_unassigned_folders(None).unwrap();
        assert_eq!(after.folders.len(), 0);
    }
}
