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
use crate::hardware::UsbCopyItem;
use crate::id;
use crate::orders::{
    self, CreateOrderRequest, EventOrderSettingsInput, OrderListResponse, OrderRecord,
    ProductRecord, ProductUpsertInput, UpdatePaymentRequest, UsbOrderPlan, UsbSubjectBreakdown,
};
use crate::preset::{
    extract_subject_number, find_preset, parse_relative_location, scaffold_folders, ScaffoldConfig,
};
use crate::roster::{
    natural_sort_key, AssignFolderResult, BatchReassignResult, FolderChangeResultItem, FolderItem,
    FolderLocationSummary, FolderParentSummary, FolderReassignmentInput, ImportRosterResult,
    IngestBatchSummary, ListFoldersResponse, RemoveFolderMode, RemoveFolderResult,
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

/// Lightweight read-only snapshot of an Event Bundle for multi-station library overview
/// and consolidated reporting (never mutates or runs migrations on unopened bundles).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleOverview {
    pub event_id: String,
    pub root_path: String,
    pub name: String,
    pub order_code: String,
    pub station_code: String,
    pub photo_counts: PhotoCounts,
    pub subject_count: i64,
    pub order_count: i64,
    pub paid_order_count: i64,
    pub gross_paid_cents: i64,
    pub pending_fulfillment_count: i64,
    pub order_numbers: Vec<String>,
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
    pub photos_removed: usize,
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
    /// Recursively checks `parent` and nested child directories (up to 6 levels deep) while
    /// skipping hidden/`_`-prefixed folders and stopping descent immediately whenever an
    /// Event Bundle is found (never walks photo directories inside bundles).
    pub fn discover_in_directory(parent: impl AsRef<Path>) -> Result<Vec<EventSummary>> {
        let parent = parent.as_ref();
        if !parent.is_dir() {
            return Err(Error::InvalidInput(format!(
                "Not a directory: {}",
                parent.display()
            )));
        }

        const MAX_DISCOVER_DEPTH: usize = 6;
        let mut bundle_roots = Vec::new();
        let mut stack: Vec<(PathBuf, usize)> = vec![(parent.to_path_buf(), 0)];

        while let Some((dir, depth)) = stack.pop() {
            if is_event_bundle(&dir) {
                bundle_roots.push(dir);
                // Do not descend into photo directories inside a discovered Event Bundle
                continue;
            }
            if depth >= MAX_DISCOVER_DEPTH {
                continue;
            }
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with('.') || name.starts_with('_') {
                        continue;
                    }
                }
                stack.push((path, depth + 1));
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

    /// Reads high-level event, photo, and order metrics without opening RW or applying migrations.
    /// Safe for scanning gathered multi-station bundles on NAS shares or inspecting bundles
    /// created by newer app versions (`SchemaTooNew`).
    pub fn inspect_readonly(root_path: impl AsRef<Path>) -> Result<BundleOverview> {
        let root = root_path.as_ref();
        let db_path = event_db_path(root);
        if !db_path.is_file() {
            return Err(Error::NotFound(format!(
                "Not a PhotoFinish Event Bundle: {}",
                root.display()
            )));
        }

        let conn = open_ro_connection(&db_path)?;
        let latest_version = EVENT_MIGRATIONS.last().map(|m| m.version).unwrap_or(0);
        let current_version: i64 =
            conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current_version > latest_version {
            return Err(Error::SchemaTooNew {
                db_version: current_version,
                app_version: latest_version,
            });
        }

        let (event_id, name, order_code, station_code): (String, String, String, String) = conn
            .query_row(
                "SELECT id, name, order_code, station_code FROM event_config LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )?;

        let photo_counts = query_photo_counts(&conn)?;
        let subject_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM subjects WHERE is_active = 1", [], |r| {
                r.get(0)
            })?;

        let (order_count, paid_order_count, gross_paid_cents): (i64, i64, i64) = conn.query_row(
            "SELECT COUNT(*),
                    COALESCE(SUM(CASE WHEN payment_status = 'paid' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN payment_status = 'paid' THEN total_cents ELSE 0 END), 0)
             FROM orders",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;

        let pending_fulfillment_count: i64 = conn.query_row(
            "SELECT COUNT(DISTINCT o.id)
             FROM orders o
             JOIN order_items oi ON oi.order_id = o.id
             WHERE o.payment_status != 'refunded'
               AND oi.fulfillment_status != 'fulfilled'",
            [],
            |r| r.get(0),
        )?;

        let mut num_stmt = conn.prepare("SELECT order_number FROM orders ORDER BY order_number")?;
        let order_numbers: Vec<String> = num_stmt
            .query_map([], |r| r.get(0))?
            .collect::<std::result::Result<_, _>>()?;

        Ok(BundleOverview {
            event_id,
            root_path: root.to_string_lossy().to_string(),
            name,
            order_code,
            station_code,
            photo_counts,
            subject_count,
            order_count,
            paid_order_count,
            gross_paid_cents,
            pending_fulfillment_count,
            order_numbers,
        })
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
             GROUP BY s.id",
        )?;
        let rows = stmt.query_map(rusqlite::params![group, trimmed, pattern], |r| {
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
        out.sort_by_key(|s| {
            let numeric_bib = s
                .number
                .as_deref()
                .and_then(|n| n.parse::<u64>().ok())
                .unwrap_or(u64::MAX);
            (numeric_bib, natural_sort_key(&s.display_name))
        });
        if limit > 0 && (out.len() as i64) > limit {
            out.truncate(limit as usize);
        }
        Ok(out)
    }

    /// Lists recent ingested card batches with live photo, thumbnail, and unnamed-folder counts
    /// for the Card Naming Queue bridging Card Ingest and Roster & Folders.
    pub fn list_ingest_batches(&self, limit: i64) -> Result<Vec<IngestBatchSummary>> {
        let conn = self.read_conn()?;
        let mut stmt = conn.prepare(
            "SELECT b.id, b.sequence_num, b.reader_id, b.reader_name, b.volume_name,
                    b.photographer, b.destination_rel_path, COALESCE(b.completed_at, b.started_at),
                    COUNT(p.id) AS total_photos,
                    COALESCE(SUM(CASE WHEN p.status = 'ready' THEN 1 ELSE 0 END), 0) AS ready_photos,
                    COALESCE(SUM(CASE WHEN p.status IN ('discovered', 'processing') THEN 1 ELSE 0 END), 0) AS pending_photos,
                    COUNT(DISTINCT CASE WHEN p.folder_path != '' THEN p.folder_path END) AS total_folders,
                    COUNT(DISTINCT CASE WHEN p.folder_path != '' AND p.subject_id IS NULL THEN p.folder_path END) AS unassigned_folders,
                    COALESCE(SUM(CASE WHEN p.subject_id IS NULL THEN 1 ELSE 0 END), 0) AS unassigned_photos
             FROM ingest_batches b
             LEFT JOIN photos p ON p.ingest_batch_id = b.id
             GROUP BY b.id
             HAVING total_photos > 0
             ORDER BY b.sequence_num DESC
             LIMIT ?1",
        )?;
        let rows = stmt
            .query_map([limit], |r| {
                Ok(IngestBatchSummary {
                    id: r.get(0)?,
                    sequence_num: r.get(1)?,
                    reader_id: r.get(2)?,
                    reader_name: r.get(3)?,
                    volume_name: r.get(4)?,
                    photographer: r.get(5)?,
                    destination_rel_path: r.get(6)?,
                    created_at: r.get(7)?,
                    total_photos: r.get(8)?,
                    ready_photos: r.get(9)?,
                    pending_photos: r.get(10)?,
                    total_folders: r.get(11)?,
                    unassigned_folders: r.get(12)?,
                    unassigned_photos: r.get(13)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Lists photo folders (either unassigned-only, all folders within a location, or strictly
    /// the folders belonging to a single ingested card via `batch_id`), along with location
    /// summaries, inferred session & group roster context, cross-apparatus reference thumbnails,
    /// and subjects already matched on the same apparatus/section in this session.
    pub fn list_folders(
        &self,
        parent_filter: Option<&str>,
        only_unassigned: bool,
        context_parent: Option<&str>,
        batch_id: Option<&str>,
    ) -> Result<ListFoldersResponse> {
        let conn = self.read_conn()?;

        // Preload lightweight subject lookup map (< 1ms for 2,000 subjects) so the
        // folder GROUP BY stays purely on `idx_photos_folder` with zero JOIN / temp B-tree.
        let subject_map: HashMap<String, (String, Option<String>)> = {
            let mut s_stmt = conn.prepare("SELECT id, display_name, number FROM subjects")?;
            let rows = s_stmt.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    (r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?),
                ))
            })?;
            rows.filter_map(|r| r.ok()).collect()
        };

        let mut stmt = conn.prepare(
            "SELECT folder_path,
                    COUNT(*) AS photo_count,
                    SUM(CASE WHEN subject_id IS NULL THEN 1 ELSE 0 END) AS unassigned_count,
                    MIN(subject_id) AS min_subject_id,
                    MAX(subject_id) AS max_subject_id
             FROM photos
             WHERE folder_path != ''
             GROUP BY folder_path",
        )?;

        struct RawFolderRow {
            folder_path: String,
            parent_path: String,
            leaf_folder: String,
            photo_count: i64,
            unassigned_count: i64,
            min_subject_id: Option<String>,
            max_subject_id: Option<String>,
        }

        let raw_rows: Vec<RawFolderRow> = stmt
            .query_map([], |r| {
                let folder_path: String = r.get(0)?;
                let (parent_path, leaf_folder) = match folder_path.rsplit_once('/') {
                    Some((p, l)) => (p.to_string(), l.to_string()),
                    None => (String::new(), folder_path.clone()),
                };
                Ok(RawFolderRow {
                    folder_path,
                    parent_path,
                    leaf_folder,
                    photo_count: r.get(1)?,
                    unassigned_count: r.get(2)?,
                    min_subject_id: r.get(3)?,
                    max_subject_id: r.get(4)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?;

        let mut loc_map: BTreeMap<String, FolderLocationSummary> = BTreeMap::new();
        let mut total_unassigned_photos = 0i64;
        let mut total_unassigned_folders = 0usize;

        let filter_norm = parent_filter.map(str::trim).filter(|s| !s.is_empty());
        let batch_norm = batch_id.map(str::trim).filter(|s| !s.is_empty());
        let mut selected_rows: Vec<RawFolderRow> = Vec::new();

        // Keep lightweight (parent_path, subject_id) tuples for instant in-memory section matching
        let mut assigned_folder_index: Vec<(String, String)> = Vec::new();

        for row in raw_rows {
            let has_unassigned = row.unassigned_count > 0;
            if has_unassigned {
                total_unassigned_photos += row.unassigned_count;
                total_unassigned_folders += 1;
            }

            if row.min_subject_id.is_some()
                && row.min_subject_id == row.max_subject_id
                && row.unassigned_count == 0
            {
                if let Some(ref sid) = row.min_subject_id {
                    assigned_folder_index.push((row.parent_path.clone(), sid.clone()));
                }
            }

            let loc = loc_map
                .entry(row.parent_path.clone())
                .or_insert_with(|| FolderLocationSummary {
                    parent_path: row.parent_path.clone(),
                    folder_count: 0,
                    photo_count: 0,
                    unassigned_folder_count: 0,
                    unassigned_photo_count: 0,
                });
            loc.folder_count += 1;
            loc.photo_count += row.photo_count;
            if has_unassigned {
                loc.unassigned_folder_count += 1;
                loc.unassigned_photo_count += row.unassigned_count;
            }

            if batch_norm.is_none() {
                if only_unassigned && !has_unassigned {
                    continue;
                }
                if let Some(f) = filter_norm {
                    if row.parent_path != f {
                        continue;
                    }
                }
                selected_rows.push(row);
            }
        }

        // If scoped to a specific ingested card (`batch_id`), load strictly that card's folders
        // using `idx_photos_batch` so the operator only sees folders from this physical card.
        let mut batch_dest_path: Option<String> = None;
        if let Some(bid) = batch_norm {
            batch_dest_path = conn
                .query_row(
                    "SELECT destination_rel_path FROM ingest_batches WHERE id = ?1",
                    [bid],
                    |r| r.get::<_, String>(0),
                )
                .optional()?
                .map(|s| s.trim_matches('/').to_string())
                .filter(|s| !s.is_empty());

            let mut b_stmt = conn.prepare(
                "SELECT folder_path,
                        COUNT(*) AS photo_count,
                        SUM(CASE WHEN subject_id IS NULL THEN 1 ELSE 0 END) AS unassigned_count,
                        MIN(subject_id) AS min_subject_id,
                        MAX(subject_id) AS max_subject_id
                 FROM photos
                 WHERE ingest_batch_id = ?1 AND folder_path != ''
                 GROUP BY folder_path",
            )?;
            selected_rows = b_stmt
                .query_map([bid], |r| {
                    let folder_path: String = r.get(0)?;
                    let (parent_path, leaf_folder) = match folder_path.rsplit_once('/') {
                        Some((p, l)) => (p.to_string(), l.to_string()),
                        None => (String::new(), folder_path.clone()),
                    };
                    Ok(RawFolderRow {
                        folder_path,
                        parent_path,
                        leaf_folder,
                        photo_count: r.get(1)?,
                        unassigned_count: r.get(2)?,
                        min_subject_id: r.get(3)?,
                        max_subject_id: r.get(4)?,
                    })
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            if only_unassigned {
                selected_rows.retain(|r| r.unassigned_count > 0);
            }
        }

        let mut locations: Vec<FolderLocationSummary> = loc_map.into_values().collect();
        locations.sort_by_key(|l| natural_sort_key(&l.parent_path));

        selected_rows.sort_by_key(|r| natural_sort_key(&r.folder_path));

        // Guard against rendering thousands of folder thumbnail cards at once if viewing
        // all folders without a location filter.
        if batch_norm.is_none()
            && !only_unassigned
            && filter_norm.is_none()
            && selected_rows.len() > 200
        {
            selected_rows.truncate(200);
        }

        // `+status = 'ready'` forces SQLite to use `idx_photos_folder` instead of
        // scanning all 150k ready rows via `idx_photos_status`.
        let mut sample_stmt = conn.prepare(
            "SELECT id FROM photos
             WHERE folder_path = ?1 AND +status = 'ready'
             ORDER BY filename ASC
             LIMIT 4",
        )?;
        let mut batch_sample_stmt = conn.prepare(
            "SELECT id FROM photos
             WHERE folder_path = ?1 AND ingest_batch_id = ?2 AND +status = 'ready'
             ORDER BY filename ASC
             LIMIT 4",
        )?;

        let mut folders = Vec::with_capacity(selected_rows.len());
        for row in selected_rows {
            let sample_photo_ids: Vec<String> = if let Some(bid) = batch_norm {
                batch_sample_stmt
                    .query_map(rusqlite::params![&row.folder_path, bid], |r| r.get(0))?
                    .filter_map(|r| r.ok())
                    .collect()
            } else {
                sample_stmt
                    .query_map([&row.folder_path], |r| r.get(0))?
                    .filter_map(|r| r.ok())
                    .collect()
            };
            let has_multiple_subjects = row.min_subject_id != row.max_subject_id;
            let has_any_subject = row.min_subject_id.is_some();
            let is_mixed = has_multiple_subjects || (has_any_subject && row.unassigned_count > 0);
            let (assigned_subject_id, assigned_subject_name, assigned_subject_number) =
                if has_any_subject && !is_mixed {
                    let sid = row.min_subject_id;
                    let (name, num) = sid
                        .as_ref()
                        .and_then(|id| subject_map.get(id))
                        .map(|(n, num)| (Some(n.clone()), num.clone()))
                        .unwrap_or((None, None));
                    (sid, name, num)
                } else {
                    (None, None, None)
                };
            folders.push(FolderItem {
                folder_path: row.folder_path,
                parent_path: row.parent_path,
                leaf_folder: row.leaf_folder,
                photo_count: row.photo_count,
                unassigned_count: row.unassigned_count,
                assigned_subject_id,
                assigned_subject_name,
                assigned_subject_number,
                is_mixed,
                sample_photo_ids,
            });
        }

        // Determine active context location for smart "Same Group" / "Same Session" scoping.
        let effective_context_parent: Option<String> = context_parent
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .or_else(|| filter_norm.map(str::to_string))
            .or_else(|| folders.first().map(|f| f.parent_path.clone()).filter(|s| !s.is_empty()))
            .or(batch_dest_path);

        let mut detected_session_group: Option<String> = None;
        let mut detected_group_prefix: Option<String> = None;
        let mut same_group_subject_ids: Vec<String> = Vec::new();
        let mut subject_sample_photos: BTreeMap<String, String> = BTreeMap::new();
        let mut section_label: Option<String> = None;
        let mut section_matched_subjects: BTreeMap<String, String> = BTreeMap::new();

        if let Some(ref ctx_parent) = effective_context_parent {
            // 1. Match roster group_label (e.g. "Session 10A") by exact path segment first,
            //    so "Session 1" never falsely matches "Session 10A".
            let all_groups = self.list_subject_groups()?;
            let segments: Vec<&str> = ctx_parent.split('/').collect();
            detected_session_group = all_groups
                .iter()
                .find(|g| segments.iter().any(|seg| seg.eq_ignore_ascii_case(g)))
                .cloned();

            // 2. Section / apparatus name (last segment of ctx_parent, e.g. "Beam" or "Floor")
            //    and subjects already matched on that SAME apparatus in earlier rotations/groups
            //    of this session so operators see fewer remaining athletes as the session progresses.
            let sec_opt = segments
                .last()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            if let Some(ref sec) = sec_opt {
                section_label = Some(sec.clone());
                for (other_parent, sid) in &assigned_folder_index {
                    let other_segs: Vec<&str> = other_parent.split('/').collect();
                    let same_section = other_segs
                        .last()
                        .map(|s| s.eq_ignore_ascii_case(sec))
                        .unwrap_or(false);
                    if !same_section {
                        continue;
                    }
                    if let Some(ref sess) = detected_session_group {
                        let same_session =
                            other_segs.iter().any(|seg| seg.eq_ignore_ascii_case(sess));
                        if !same_session {
                            continue;
                        }
                    }
                    // Short distinguishing label right above the apparatus (e.g. "Group 1A")
                    let short_loc = if other_segs.len() >= 2 {
                        other_segs[other_segs.len() - 2].to_string()
                    } else {
                        other_parent.clone()
                    };
                    section_matched_subjects
                        .entry(sid.clone())
                        .or_insert(short_loc);
                }
            }

            // 3. Group ancestor prefix (one level above apparatus/section, e.g.
            //    "Gym A/Session 10A/Group 1A/Beam" -> "Gym A/Session 10A/Group 1A").
            if let Some((ancestor, _)) = ctx_parent.rsplit_once('/') {
                if !ancestor.is_empty() {
                    detected_group_prefix = Some(ancestor.to_string());
                    let range_start = format!("{}/", ancestor);
                    let range_end = format!("{}0", ancestor); // ASCII '0' = '/' + 1 for B-tree range scan
                    let mut grp_stmt = conn.prepare(
                        "SELECT DISTINCT subject_id FROM photos
                         WHERE folder_path >= ?1 AND folder_path < ?2
                           AND subject_id IS NOT NULL",
                    )?;
                    same_group_subject_ids = grp_stmt
                        .query_map(rusqlite::params![range_start, range_end], |r| {
                            r.get::<_, String>(0)
                        })?
                        .filter_map(|r| r.ok())
                        .collect();
                }
            }

            // 4. Collect 1 reference thumbnail per subject in Same Group and Same Session,
            //    preferring a ready photo OUTSIDE `ctx_parent` (e.g. Floor when fixing Beam!).
            //    `+status = 'ready'` forces `idx_photos_subject` (~1ms total for 100 subjects!).
            let mut candidate_subject_ids: HashSet<String> =
                same_group_subject_ids.iter().cloned().collect();
            if let Some(ref sess) = detected_session_group {
                let mut sess_stmt = conn.prepare(
                    "SELECT id FROM subjects WHERE is_active = 1 AND group_label = ?1 LIMIT 250",
                )?;
                for id_res in sess_stmt.query_map([sess], |r| r.get::<_, String>(0))? {
                    if let Ok(sid) = id_res {
                        candidate_subject_ids.insert(sid);
                    }
                }
            }

            let ctx_prefix_start = format!("{}/", ctx_parent);
            let ctx_prefix_end = format!("{}0", ctx_parent);
            let mut other_photo_stmt = conn.prepare(
                "SELECT id FROM photos
                 WHERE subject_id = ?1 AND +status = 'ready'
                   AND NOT (folder_path >= ?2 AND folder_path < ?3)
                 LIMIT 1",
            )?;
            let mut any_photo_stmt = conn.prepare(
                "SELECT id FROM photos
                 WHERE subject_id = ?1 AND +status = 'ready'
                 LIMIT 1",
            )?;

            for sid in candidate_subject_ids {
                let pic = other_photo_stmt
                    .query_row(
                        rusqlite::params![&sid, &ctx_prefix_start, &ctx_prefix_end],
                        |r| r.get::<_, String>(0),
                    )
                    .optional()?
                    .or_else(|| {
                        any_photo_stmt
                            .query_row([&sid], |r| r.get::<_, String>(0))
                            .optional()
                            .ok()
                            .flatten()
                    });
                if let Some(pid) = pic {
                    subject_sample_photos.insert(sid, pid);
                }
            }
        }

        Ok(ListFoldersResponse {
            locations,
            folders,
            total_unassigned_photos,
            total_unassigned_folders,
            detected_session_group,
            detected_group_prefix,
            same_group_subject_ids,
            subject_sample_photos,
            section_label,
            section_matched_subjects,
        })
    }

    /// Atomically applies one or more folder reassignments/renames/swaps/merges.
    ///
    /// Uses a two-phase swap-safe staging rename on disk (`.pf_swap_*`) and a two-phase
    /// `relative_path` update inside a single SQLite transaction so cyclic rotation shifts
    /// (e.g. A -> B, B -> C, C -> A) never collide, never mix athletes' photos, and never
    /// violate `UNIQUE(relative_path)`.
    pub fn apply_folder_reassignments(
        &self,
        inputs: &[FolderReassignmentInput],
        rename_on_disk: bool,
    ) -> Result<BatchReassignResult> {
        if inputs.is_empty() {
            return Ok(BatchReassignResult {
                folders_updated: 0,
                photos_updated: 0,
                renamed_on_disk: false,
                changes: Vec::new(),
            });
        }

        let mut conn = self.write_conn.lock().unwrap();

        struct PlannedMove {
            old_folder_path: String,
            parent_path: String,
            target_subject_id: Option<String>,
            target_subject_name: Option<String>,
            target_folder_path: String,
            target_leaf: String,
            will_rename_disk: bool,
            action: String,
        }

        let mut seen_old = HashSet::new();
        let mut plans = Vec::with_capacity(inputs.len());

        for input in inputs {
            let old_fp = validate_event_rel_folder(&input.folder_path)?;
            if !seen_old.insert(old_fp.clone()) {
                return Err(Error::InvalidInput(format!(
                    "Duplicate source folder '{}' in batch",
                    old_fp
                )));
            }
            let (parent_path, old_leaf) = match old_fp.rsplit_once('/') {
                Some((p, l)) => (p.to_string(), l.to_string()),
                None => (String::new(), old_fp.clone()),
            };

            let (target_subject_name, target_leaf, target_folder_path, will_rename_disk) =
                match input.target_subject_id.as_deref() {
                    Some(sid) => {
                        let display_name: String = conn
                            .query_row(
                                "SELECT display_name FROM subjects WHERE id = ?1",
                                [sid],
                                |r| r.get(0),
                            )
                            .optional()?
                            .ok_or_else(|| Error::NotFound(format!("Subject {} not found", sid)))?;
                        let sanitized = sanitize_folder_component(&display_name);
                        if sanitized.is_empty() {
                            return Err(Error::InvalidInput(format!(
                                "Subject '{}' produces an empty folder name",
                                display_name
                            )));
                        }
                        let target_fp = if parent_path.is_empty() {
                            sanitized.clone()
                        } else {
                            format!("{}/{}", parent_path, sanitized)
                        };
                        let old_abs = self.root.join(&old_fp);
                        let will_rename = rename_on_disk && target_fp != old_fp && old_abs.is_dir();
                        (
                            Some(display_name),
                            if will_rename { sanitized } else { old_leaf.clone() },
                            if will_rename { target_fp } else { old_fp.clone() },
                            will_rename,
                        )
                    }
                    None => (None, old_leaf.clone(), old_fp.clone(), false),
                };

            plans.push(PlannedMove {
                old_folder_path: old_fp,
                parent_path,
                target_subject_id: input.target_subject_id.clone(),
                target_subject_name,
                target_folder_path,
                target_leaf,
                will_rename_disk,
                action: String::new(),
            });
        }

        // Classify each action accurately (swapped vs renamed vs merged vs reassigned vs unassigned)
        let moving_away: HashSet<String> = plans
            .iter()
            .filter(|p| p.will_rename_disk)
            .map(|p| p.old_folder_path.clone())
            .collect();
        let mut target_counts: HashMap<String, usize> = HashMap::new();
        for p in &plans {
            if p.will_rename_disk {
                *target_counts.entry(p.target_folder_path.clone()).or_insert(0) += 1;
            }
        }
        for p in &mut plans {
            p.action = if p.target_subject_id.is_none() {
                "unassigned".to_string()
            } else if !p.will_rename_disk {
                "reassigned".to_string()
            } else if target_counts.get(&p.target_folder_path).copied().unwrap_or(0) > 1 {
                "merged".to_string()
            } else if moving_away.contains(&p.target_folder_path) {
                "swapped".to_string()
            } else if self.root.join(&p.target_folder_path).exists() {
                "merged".to_string()
            } else {
                "renamed".to_string()
            };
        }

        // Snapshot all affected photo rows BEFORE modifying SQLite or disk so swapped folder
        // paths never cross-contaminate.
        struct PhotoSnap {
            id: String,
            filename: String,
            tags_json: String,
        }
        let mut photos_by_plan: Vec<Vec<PhotoSnap>> = Vec::with_capacity(plans.len());
        {
            let mut stmt = conn.prepare(
                "SELECT id, filename, tags_json FROM photos WHERE folder_path = ?1",
            )?;
            for p in &plans {
                let rows = stmt
                    .query_map([&p.old_folder_path], |r| {
                        Ok(PhotoSnap {
                            id: r.get(0)?,
                            filename: r.get(1)?,
                            tags_json: r.get(2)?,
                        })
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                photos_by_plan.push(rows);
            }
        }

        let batch_nonce = id::generate("swp_");
        let mut any_disk_rename = false;
        let mut file_renames_by_plan: Vec<HashMap<String, String>> =
            vec![HashMap::new(); plans.len()];

        // Phase 1 on disk: move every renaming folder to a unique hidden staging folder in its parent.
        let mut staged: Vec<(usize, PathBuf, PathBuf)> = Vec::new();
        for (idx, p) in plans.iter().enumerate() {
            if !p.will_rename_disk {
                continue;
            }
            let old_abs = self.root.join(&p.old_folder_path);
            let parent_abs = if p.parent_path.is_empty() {
                self.root.clone()
            } else {
                self.root.join(&p.parent_path)
            };
            let staging_abs = parent_abs.join(format!(".pf_swap_{}_{}", batch_nonce, idx));
            if let Err(e) = fs::rename(&old_abs, &staging_abs) {
                // Roll back anything already moved in Phase 1
                for (_, st, orig) in staged.into_iter().rev() {
                    let _ = fs::rename(&st, &orig);
                }
                return Err(Error::Io(format!(
                    "Failed to stage rename {} -> {}: {}",
                    old_abs.display(),
                    staging_abs.display(),
                    e
                )));
            }
            staged.push((idx, staging_abs, old_abs));
        }

        // Phase 2 on disk: move each staged folder into its final destination (or safely merge files).
        for (idx, staging_abs, _old_abs) in staged {
            let p = &plans[idx];
            let target_abs = self.root.join(&p.target_folder_path);
            if !target_abs.exists() {
                fs::rename(&staging_abs, &target_abs).map_err(|e| {
                    Error::Io(format!(
                        "Failed to finalize rename {} -> {}: {}",
                        staging_abs.display(),
                        target_abs.display(),
                        e
                    ))
                })?;
            } else if target_abs.is_dir() {
                for entry in fs::read_dir(&staging_abs)
                    .map_err(|e| Error::Io(format!("{}: {}", staging_abs.display(), e)))?
                    .flatten()
                {
                    let src_file = entry.path();
                    if src_file.is_file() {
                        let orig_name = entry.file_name().to_string_lossy().to_string();
                        let final_name = unique_filename_in_dir(&target_abs, &orig_name);
                        let dst_file = target_abs.join(&final_name);
                        fs::rename(&src_file, &dst_file).map_err(|e| {
                            Error::Io(format!(
                                "Failed to merge {} -> {}: {}",
                                src_file.display(),
                                dst_file.display(),
                                e
                            ))
                        })?;
                        if final_name != orig_name {
                            file_renames_by_plan[idx].insert(orig_name, final_name);
                        }
                    }
                }
                let _ = fs::remove_dir(&staging_abs);
            } else {
                return Err(Error::Io(format!(
                    "Target path {} already exists as a file",
                    target_abs.display()
                )));
            }
            any_disk_rename = true;
        }

        // Phase 3 in SQLite: two-pass atomic transaction so UNIQUE(relative_path) never conflicts.
        let folder_levels = load_folder_levels(&conn)?;
        let now = Utc::now().to_rfc3339();
        let tx = conn.transaction()?;

        {
            let mut temp_stmt =
                tx.prepare("UPDATE photos SET relative_path = ?2 WHERE id = ?1")?;
            for (idx, p) in plans.iter().enumerate() {
                if !p.will_rename_disk {
                    continue;
                }
                for photo in &photos_by_plan[idx] {
                    let tmp_rel = format!("__pf_swap__/{}/{}", batch_nonce, photo.id);
                    temp_stmt.execute(rusqlite::params![photo.id, tmp_rel])?;
                }
            }
        }

        let mut total_photos_updated = 0usize;
        let mut change_results = Vec::with_capacity(plans.len());

        {
            let mut full_upd = tx.prepare(
                "UPDATE photos
                 SET subject_id = ?2, relative_path = ?3, folder_path = ?4,
                     leaf_folder = ?5, filename = ?6, tags_json = ?7, updated_at = ?8
                 WHERE id = ?1",
            )?;
            let mut subj_upd = tx.prepare(
                "UPDATE photos SET subject_id = ?2, updated_at = ?3 WHERE id = ?1",
            )?;

            for (idx, p) in plans.iter().enumerate() {
                let mut count = 0usize;
                for photo in &photos_by_plan[idx] {
                    if p.will_rename_disk {
                        let new_filename = file_renames_by_plan[idx]
                            .get(&photo.filename)
                            .cloned()
                            .unwrap_or_else(|| photo.filename.clone());
                        let new_rel = if p.target_folder_path.is_empty() {
                            new_filename.clone()
                        } else {
                            format!("{}/{}", p.target_folder_path, new_filename)
                        };
                        let mut merged_tags: BTreeMap<String, String> =
                            serde_json::from_str(&photo.tags_json).unwrap_or_default();
                        let parsed = parse_relative_location(&new_rel, &folder_levels);
                        for (k, v) in parsed.tags {
                            merged_tags.insert(k, v);
                        }
                        let new_tags_json = serde_json::to_string(&merged_tags)?;
                        count += full_upd.execute(rusqlite::params![
                            photo.id,
                            p.target_subject_id,
                            new_rel,
                            p.target_folder_path,
                            p.target_leaf,
                            new_filename,
                            new_tags_json,
                            now,
                        ])?;
                    } else {
                        count += subj_upd.execute(rusqlite::params![
                            photo.id,
                            p.target_subject_id,
                            now,
                        ])?;
                    }
                }
                total_photos_updated += count;
                change_results.push(FolderChangeResultItem {
                    old_folder_path: p.old_folder_path.clone(),
                    new_folder_path: p.target_folder_path.clone(),
                    target_subject_id: p.target_subject_id.clone(),
                    target_subject_name: p.target_subject_name.clone(),
                    photos_updated: count,
                    action: p.action.clone(),
                });
            }
        }

        tx.commit()?;

        Ok(BatchReassignResult {
            folders_updated: plans.len(),
            photos_updated: total_photos_updated,
            renamed_on_disk: any_disk_rename,
            changes: change_results,
        })
    }

    /// Removes a photo folder from the event database (and cached thumbnails/previews),
    /// and either moves the original folder to `_photofinish/trash/` (reversible and never
    /// re-scanned), permanently deletes it from disk, or leaves originals untouched on disk.
    pub fn remove_folder(
        &self,
        folder_path: &str,
        mode: RemoveFolderMode,
    ) -> Result<RemoveFolderResult> {
        let rel_folder = validate_event_rel_folder(folder_path)?;
        let mut conn = self.write_conn.lock().unwrap();

        struct DelPhoto {
            relative_path: String,
            thumb: Option<String>,
            preview: Option<String>,
        }

        let photos: Vec<DelPhoto> = {
            let mut stmt = conn.prepare(
                "SELECT relative_path, thumbnail_rel_path, preview_rel_path
                 FROM photos WHERE folder_path = ?1",
            )?;
            let rows = stmt
                .query_map([&rel_folder], |r| {
                    Ok(DelPhoto {
                        relative_path: r.get(0)?,
                        thumb: r.get(1)?,
                        preview: r.get(2)?,
                    })
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            rows
        };

        let tx = conn.transaction()?;
        let photos_removed = tx.execute("DELETE FROM photos WHERE folder_path = ?1", [&rel_folder])?;
        tx.commit()?;

        // Clean up cached thumbnails & previews inside `_photofinish/`
        for p in &photos {
            if let Some(ref t) = p.thumb {
                let _ = fs::remove_file(self.root.join(t));
            }
            if let Some(ref pr) = p.preview {
                let _ = fs::remove_file(self.root.join(pr));
            }
        }

        let old_abs = self.root.join(&rel_folder);
        let mut trashed_to: Option<String> = None;

        match mode {
            RemoveFolderMode::DbOnly => {}
            RemoveFolderMode::Trash => {
                if old_abs.exists() {
                    let mut trash_rel = format!("_photofinish/trash/{}", rel_folder);
                    let mut trash_abs = self.root.join(&trash_rel);
                    if trash_abs.exists() {
                        let ts = Utc::now().format("%Y%m%d_%H%M%S");
                        trash_rel = format!("_photofinish/trash/{}_{}", rel_folder, ts);
                        trash_abs = self.root.join(&trash_rel);
                    }
                    if old_abs.is_dir() && !dir_has_subdirectories(&old_abs) {
                        if let Some(parent) = trash_abs.parent() {
                            fs::create_dir_all(parent).map_err(|e| {
                                Error::Io(format!("Failed to create trash dir {}: {}", parent.display(), e))
                            })?;
                        }
                        fs::rename(&old_abs, &trash_abs).map_err(|e| {
                            Error::Io(format!(
                                "Failed to move {} to trash {}: {}",
                                old_abs.display(),
                                trash_abs.display(),
                                e
                            ))
                        })?;
                    } else {
                        fs::create_dir_all(&trash_abs).map_err(|e| {
                            Error::Io(format!("Failed to create trash dir {}: {}", trash_abs.display(), e))
                        })?;
                        for p in &photos {
                            let src = self.root.join(&p.relative_path);
                            if src.is_file() {
                                if let Some(fname) = src.file_name() {
                                    let dst = trash_abs.join(fname);
                                    let _ = fs::rename(&src, &dst);
                                }
                            }
                        }
                        let _ = fs::remove_dir(&old_abs);
                    }
                    trashed_to = Some(trash_rel);
                }
            }
            RemoveFolderMode::DeleteDisk => {
                if old_abs.is_dir() && !dir_has_subdirectories(&old_abs) {
                    fs::remove_dir_all(&old_abs).map_err(|e| {
                        Error::Io(format!("Failed to delete {}: {}", old_abs.display(), e))
                    })?;
                } else {
                    for p in &photos {
                        let src = self.root.join(&p.relative_path);
                        if src.is_file() {
                            let _ = fs::remove_file(&src);
                        }
                    }
                    let _ = fs::remove_dir(&old_abs);
                }
            }
        }

        Ok(RemoveFolderResult {
            folder_path: rel_folder,
            photos_removed,
            mode,
            trashed_to,
        })
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
             WHERE folder_path = ?1 AND subject_id IS NULL AND +status = 'ready'
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

    /// Two-way full-event reconcile: discovers any new JPEGs on disk AND prunes orphaned DB rows
    /// (plus their cached thumbnails/previews) whose original files/folders were deleted in Finder.
    /// Always skips `_`-prefixed directories (including `_photofinish`) and hidden dotfiles.
    pub fn scan_directory(&self) -> Result<ScanResult> {
        self.reconcile_subpaths(&[String::new()])
    }

    /// Reconciles one or more relative directory subpaths inside the event bundle against SQLite:
    /// - Inserts any newly dropped JPEGs (skipping 0-byte files still being written), auto-matching
    ///   them to the roster by leading bib number and returning `pending_jobs` for thumbnailing.
    /// - Prunes any DB rows under those subpaths whose files no longer exist on disk (e.g. a
    ///   deleted folder like `Session 18A` or Finder rename), along with their cached thumbnails
    ///   and previews inside `_photofinish/`.
    ///
    /// Passing `[""]` performs a full-event reconcile. Passing specific subpaths (from the
    /// FSEvents auto-watcher) scopes both disk walking and SQLite queries (`idx_photos_folder`)
    /// so changes reconcile in milliseconds even on 150,000+ photo meets.
    pub fn reconcile_subpaths(&self, rel_prefixes: &[String]) -> Result<ScanResult> {
        // Safety guard: never prune if the event volume/bundle is unmounted or unreadable.
        if !self.root.is_dir() || !self.db_path.is_file() {
            return Err(Error::Io(format!(
                "Event bundle root is not accessible: {}",
                self.root.display()
            )));
        }

        let prefixes = collapse_reconcile_prefixes(rel_prefixes);
        if prefixes.is_empty() {
            return Ok(ScanResult {
                photos_found: 0,
                photos_new: 0,
                photos_skipped: 0,
                photos_removed: 0,
                pending_jobs: Vec::new(),
            });
        }

        let mut total_found = 0usize;
        let mut total_skipped = 0usize;
        let mut all_pending: Vec<PendingPhotoJob> = Vec::new();
        let mut files_to_cleanup: Vec<String> = Vec::new();
        let mut total_removed = 0usize;

        for prefix in &prefixes {
            // 1. Walk disk strictly within `prefix` (if it still exists on disk).
            let mut discovered: HashMap<String, IngestFileInput> = HashMap::new();
            let walk_target = if prefix.is_empty() {
                self.root.clone()
            } else {
                self.root.join(prefix)
            };

            if walk_target.is_dir() {
                for entry in WalkDir::new(&walk_target)
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
                    if !is_jpeg_path(entry.path()) {
                        continue;
                    }
                    let size = entry.metadata().map(|m| m.len() as i64).unwrap_or(0);
                    // Skip 0-byte placeholder files that Finder/copy tools may still be creating.
                    if size == 0 {
                        continue;
                    }
                    if let Ok(rel) = entry.path().strip_prefix(&self.root) {
                        let rel_str = rel.to_string_lossy().replace('\\', "/");
                        if has_hidden_or_internal_segment(&rel_str) {
                            continue;
                        }
                        let orig_name = entry.file_name().to_string_lossy().to_string();
                        discovered.insert(
                            rel_str.clone(),
                            IngestFileInput {
                                relative_path: rel_str,
                                original_filename: orig_name,
                                file_size_bytes: size,
                            },
                        );
                    }
                }
            } else if walk_target.is_file() && is_jpeg_path(&walk_target) {
                let size = walk_target.metadata().map(|m| m.len() as i64).unwrap_or(0);
                if size > 0 {
                    if let Ok(rel) = walk_target.strip_prefix(&self.root) {
                        let rel_str = rel.to_string_lossy().replace('\\', "/");
                        if !has_hidden_or_internal_segment(&rel_str) {
                            let orig_name = walk_target
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default();
                            discovered.insert(
                                rel_str.clone(),
                                IngestFileInput {
                                    relative_path: rel_str,
                                    original_filename: orig_name,
                                    file_size_bytes: size,
                                },
                            );
                        }
                    }
                }
            }

            total_found += discovered.len();

            // 2. Diff against DB rows scoped strictly to `prefix` inside a single transaction.
            let mut conn = self.write_conn.lock().unwrap();
            let folder_levels = load_folder_levels(&conn)?;
            let subject_lookup = load_subject_number_map(&conn)?;

            struct ExistingRow {
                id: String,
                relative_path: String,
                thumb: Option<String>,
                preview: Option<String>,
            }

            let existing_rows: Vec<ExistingRow> = if prefix.is_empty() {
                let mut stmt = conn.prepare(
                    "SELECT id, relative_path, thumbnail_rel_path, preview_rel_path FROM photos",
                )?;
                let rows = stmt
                    .query_map([], |r| {
                        Ok(ExistingRow {
                            id: r.get(0)?,
                            relative_path: r.get(1)?,
                            thumb: r.get(2)?,
                            preview: r.get(3)?,
                        })
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                rows
            } else {
                let range_start = format!("{}/", prefix);
                let range_end = format!("{}0", prefix); // ASCII '0' = '/' + 1
                let mut stmt = conn.prepare(
                    "SELECT id, relative_path, thumbnail_rel_path, preview_rel_path
                     FROM photos
                     WHERE folder_path = ?1
                        OR (folder_path >= ?2 AND folder_path < ?3)",
                )?;
                let rows = stmt
                    .query_map(rusqlite::params![prefix, range_start, range_end], |r| {
                        Ok(ExistingRow {
                            id: r.get(0)?,
                            relative_path: r.get(1)?,
                            thumb: r.get(2)?,
                            preview: r.get(3)?,
                        })
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                rows
            };

            let mut existing_paths: HashSet<String> = HashSet::with_capacity(existing_rows.len());
            let mut orphans: Vec<ExistingRow> = Vec::new();
            for row in existing_rows {
                if discovered.contains_key(&row.relative_path) {
                    existing_paths.insert(row.relative_path);
                } else if !self.root.join(&row.relative_path).is_file() {
                    // Belt-and-suspenders check: only prune rows genuinely absent from disk.
                    orphans.push(row);
                } else {
                    existing_paths.insert(row.relative_path);
                }
            }

            let now = Utc::now().to_rfc3339();
            let tx = conn.transaction()?;

            if !orphans.is_empty() {
                let mut del_stmt = tx.prepare("DELETE FROM photos WHERE id = ?1")?;
                for orphan in &orphans {
                    del_stmt.execute([&orphan.id])?;
                }
            }

            let mut new_entries: Vec<IngestFileInput> = discovered
                .into_values()
                .filter(|f| {
                    if existing_paths.contains(&f.relative_path) {
                        total_skipped += 1;
                        false
                    } else {
                        true
                    }
                })
                .collect();
            new_entries.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

            if !new_entries.is_empty() {
                let mut ins_stmt = tx.prepare(
                    "INSERT INTO photos (
                        id, subject_id, relative_path, folder_path, leaf_folder,
                        filename, original_filename, file_size_bytes, tags_json,
                        status, created_at, updated_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'discovered', ?10, ?11)",
                )?;

                for f in new_entries {
                    let parsed = parse_relative_location(&f.relative_path, &folder_levels);
                    let subject_id = parsed
                        .leaf_folder
                        .as_deref()
                        .and_then(extract_subject_number)
                        .and_then(|num| subject_lookup.get(&num).cloned());
                    let photo_id = id::photo_id();
                    let tags_json = serde_json::to_string(&parsed.tags)?;
                    ins_stmt.execute(rusqlite::params![
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
                    all_pending.push(PendingPhotoJob {
                        photo_id,
                        relative_path: f.relative_path,
                    });
                }
            }

            tx.commit()?;
            drop(conn);

            total_removed += orphans.len();
            for orphan in orphans {
                if let Some(t) = orphan.thumb {
                    files_to_cleanup.push(t);
                }
                if let Some(pr) = orphan.preview {
                    files_to_cleanup.push(pr);
                }
            }
        }

        // 3. Remove cached thumbnails/previews for pruned photos outside DB lock.
        for rel in files_to_cleanup {
            let _ = fs::remove_file(self.root.join(rel));
        }

        Ok(ScanResult {
            photos_found: total_found,
            photos_new: all_pending.len(),
            photos_skipped: total_skipped,
            photos_removed: total_removed,
            pending_jobs: all_pending,
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

    /// Returns a fast random sample of ready photo IDs for the kiosk home marquee
    /// using O(k log N) rowid probes instead of sorting 150K+ rows with `ORDER BY RANDOM()`.
    pub fn list_showcase_photo_ids(&self, count: i64) -> Result<Vec<String>> {
        use rand::Rng;

        let conn = self.read_conn()?;
        let target = count.clamp(1, 60) as usize;
        let max_rowid: i64 = conn
            .query_row("SELECT COALESCE(MAX(rowid), 0) FROM photos", [], |r| r.get(0))
            .unwrap_or(0);
        if max_rowid <= 0 {
            return Ok(Vec::new());
        }

        // Small table: direct query is instant
        if max_rowid <= 500 {
            let mut stmt = conn.prepare(
                "SELECT id FROM photos WHERE status = 'ready' ORDER BY RANDOM() LIMIT ?1",
            )?;
            let ids: Vec<String> = stmt
                .query_map([target as i64], |r| r.get(0))?
                .filter_map(|r| r.ok())
                .collect();
            return Ok(ids);
        }

        // Large table (10K–200K+ photos): probe random rowids via primary B-tree (<1ms!)
        let mut rng = rand::thread_rng();
        let mut stmt = conn.prepare(
            "SELECT id FROM photos WHERE rowid >= ?1 AND status = 'ready' LIMIT 1",
        )?;
        let mut seen = HashSet::with_capacity(target);
        let mut ids = Vec::with_capacity(target);
        let max_attempts = target * 4;
        for _ in 0..max_attempts {
            if ids.len() >= target {
                break;
            }
            let probe = rng.gen_range(1..=max_rowid);
            if let Ok(Some(id)) = stmt.query_row([probe], |r| r.get::<_, String>(0)).optional() {
                if seen.insert(id.clone()) {
                    ids.push(id);
                }
            }
        }
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

    pub fn list_orders(&self) -> Result<OrderListResponse> {
        let conn = self.read_conn()?;
        orders::list_orders(&conn)
    }

    pub fn update_order_payment(&self, req: &UpdatePaymentRequest) -> Result<OrderRecord> {
        let mut conn = self.write_conn.lock().unwrap();
        orders::update_order_payment(&mut conn, req)
    }

    pub fn update_order_notes(&self, order_id: &str, notes: Option<&str>) -> Result<OrderRecord> {
        let mut conn = self.write_conn.lock().unwrap();
        orders::update_order_notes(&mut conn, order_id, notes)
    }

    pub fn update_order_item_fulfillment(
        &self,
        order_id: &str,
        item_id: &str,
        fulfillment_status: &str,
    ) -> Result<OrderRecord> {
        let mut conn = self.write_conn.lock().unwrap();
        orders::update_order_item_fulfillment(&mut conn, order_id, item_id, fulfillment_status)
    }

    pub fn mark_usb_items_fulfilled(&self, order_id: &str) -> Result<usize> {
        let mut conn = self.write_conn.lock().unwrap();
        orders::mark_usb_items_fulfilled(&mut conn, order_id)
    }

    pub fn upsert_product(&self, input: &ProductUpsertInput) -> Result<ProductRecord> {
        let mut conn = self.write_conn.lock().unwrap();
        orders::upsert_product(&mut conn, input)
    }

    pub fn update_event_order_settings(&self, input: &EventOrderSettingsInput) -> Result<EventSummary> {
        {
            let mut conn = self.write_conn.lock().unwrap();
            orders::update_event_order_settings(&mut conn, input)?;
        }
        self.summary()
    }

    /// Resolves all full-resolution original JPEG files that belong on a USB drive for `order_id`,
    /// organized cleanly as `<Subject Display Name>/<Section (e.g. Beam, Floor)>/<filename>`.
    pub fn resolve_order_usb_copy_plan(
        &self,
        order_id: &str,
    ) -> Result<(UsbOrderPlan, Vec<UsbCopyItem>)> {
        let conn = self.read_conn()?;
        let order = orders::get_order_by_id(&conn, order_id)?
            .ok_or_else(|| Error::NotFound(format!("Order '{}' not found", order_id)))?;

        let folder_levels = load_folder_levels(&conn).unwrap_or_default();
        let section_level_key: Option<String> = if folder_levels.len() >= 2 {
            Some(folder_levels[folder_levels.len() - 2].clone())
        } else {
            folder_levels.first().cloned()
        };

        // Determine which subjects get ALL photos vs individual selected photos
        let mut full_subject_ids: Vec<String> = Vec::new();
        let mut single_photo_ids: Vec<(String, Option<String>)> = Vec::new();
        let mut usb_fallback_subject_ids: Vec<String> = Vec::new();

        for item in &order.items {
            match item.product_type.as_str() {
                "full_digital" => {
                    if let Some(ref sid) = item.subject_id {
                        if !full_subject_ids.contains(sid) {
                            full_subject_ids.push(sid.clone());
                        }
                    }
                }
                "single_digital" => {
                    if let Some(ref pid) = item.photo_id {
                        single_photo_ids.push((pid.clone(), item.subject_display_name.clone()));
                    }
                }
                "usb_addon" => {
                    if let Some(ref sid) = item.subject_id {
                        if !usb_fallback_subject_ids.contains(sid) {
                            usb_fallback_subject_ids.push(sid.clone());
                        }
                    }
                }
                _ => {}
            }
        }

        // If an order has a usb_addon for a subject without an explicit full_digital or single_digital
        // item for that subject, include all photos for that subject as well.
        let single_subject_set: HashSet<String> = order
            .items
            .iter()
            .filter(|i| i.product_type == "single_digital")
            .filter_map(|i| i.subject_id.clone())
            .collect();
        for sid in usb_fallback_subject_ids {
            if !full_subject_ids.contains(&sid) && !single_subject_set.contains(&sid) {
                full_subject_ids.push(sid);
            }
        }

        let mut seen_photo_ids: HashSet<String> = HashSet::new();
        let mut copy_items: Vec<UsbCopyItem> = Vec::new();
        let mut subject_groups: BTreeMap<String, (usize, u64, BTreeMap<String, usize>)> =
            BTreeMap::new();
        let mut missing_files = 0usize;

        let extract_section = |folder_path: &str, tags_json: &str| -> String {
            let tags: BTreeMap<String, String> =
                serde_json::from_str(tags_json).unwrap_or_default();
            section_level_key
                .as_ref()
                .and_then(|k| tags.get(k))
                .cloned()
                .or_else(|| {
                    folder_path
                        .rsplit_once('/')
                        .and_then(|(parent, _)| parent.rsplit('/').next())
                        .filter(|s| !s.is_empty())
                        .map(str::to_string)
                })
                .unwrap_or_else(|| "Photos".to_string())
        };

        let mut photo_by_subject_stmt = conn.prepare(
            "SELECT p.id, p.relative_path, p.folder_path, p.filename, p.tags_json,
                    COALESCE(p.file_size_bytes, 0), COALESCE(s.display_name, 'Photos')
             FROM photos p
             LEFT JOIN subjects s ON s.id = p.subject_id
             WHERE p.subject_id = ?1 AND +p.status = 'ready'
             ORDER BY p.folder_path ASC, p.filename ASC",
        )?;

        for sid in &full_subject_ids {
            let rows = photo_by_subject_stmt.query_map([sid], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, i64>(5)?,
                    r.get::<_, String>(6)?,
                ))
            })?;
            for row in rows {
                let (pid, rel_path, folder_path, filename, tags_json, db_size, subject_name) = row?;
                if !seen_photo_ids.insert(pid) {
                    continue;
                }
                let src_abs = self.root.join(&rel_path);
                let size_bytes = match fs::metadata(&src_abs) {
                    Ok(m) if m.is_file() => m.len(),
                    _ => {
                        missing_files += 1;
                        continue;
                    }
                };
                let effective_size = if size_bytes > 0 {
                    size_bytes
                } else {
                    db_size.max(0) as u64
                };
                let section = extract_section(&folder_path, &tags_json);
                let dest_rel = format!(
                    "{}/{}/{}",
                    sanitize_folder_component(&subject_name),
                    sanitize_folder_component(&section),
                    sanitize_folder_component(&filename)
                );
                copy_items.push(UsbCopyItem {
                    source_abs_path: src_abs,
                    dest_rel_path: dest_rel,
                    file_size_bytes: effective_size,
                });
                let entry = subject_groups
                    .entry(subject_name)
                    .or_insert_with(|| (0, 0, BTreeMap::new()));
                entry.0 += 1;
                entry.1 += effective_size;
                *entry.2.entry(section).or_insert(0) += 1;
            }
        }

        let mut single_stmt = conn.prepare(
            "SELECT p.id, p.relative_path, p.folder_path, p.filename, p.tags_json,
                    COALESCE(p.file_size_bytes, 0), s.display_name
             FROM photos p
             LEFT JOIN subjects s ON s.id = p.subject_id
             WHERE p.id = ?1",
        )?;

        for (pid, fallback_subject) in single_photo_ids {
            if !seen_photo_ids.insert(pid.clone()) {
                continue;
            }
            let row = single_stmt
                .query_row([&pid], |r| {
                    Ok((
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, i64>(5)?,
                        r.get::<_, Option<String>>(6)?,
                    ))
                })
                .optional()?;
            let Some((rel_path, folder_path, filename, tags_json, db_size, db_subject)) = row else {
                missing_files += 1;
                continue;
            };
            let src_abs = self.root.join(&rel_path);
            let size_bytes = match fs::metadata(&src_abs) {
                Ok(m) if m.is_file() => m.len(),
                _ => {
                    missing_files += 1;
                    continue;
                }
            };
            let effective_size = if size_bytes > 0 {
                size_bytes
            } else {
                db_size.max(0) as u64
            };
            let subject_name = db_subject
                .or(fallback_subject)
                .unwrap_or_else(|| "Selected Photos".to_string());
            let section = extract_section(&folder_path, &tags_json);
            let dest_rel = format!(
                "{}/{}/{}",
                sanitize_folder_component(&subject_name),
                sanitize_folder_component(&section),
                sanitize_folder_component(&filename)
            );
            copy_items.push(UsbCopyItem {
                source_abs_path: src_abs,
                dest_rel_path: dest_rel,
                file_size_bytes: effective_size,
            });
            let entry = subject_groups
                .entry(subject_name)
                .or_insert_with(|| (0, 0, BTreeMap::new()));
            entry.0 += 1;
            entry.1 += effective_size;
            *entry.2.entry(section).or_insert(0) += 1;
        }

        let total_photos = copy_items.len();
        let total_bytes: u64 = copy_items.iter().map(|i| i.file_size_bytes).sum();
        let subjects: Vec<UsbSubjectBreakdown> = subject_groups
            .into_iter()
            .map(|(subject_label, (photo_count, total_bytes, sec_map))| UsbSubjectBreakdown {
                subject_label,
                photo_count,
                total_bytes,
                sections: sec_map.into_iter().collect(),
            })
            .collect();

        Ok((
            UsbOrderPlan {
                order_id: order.id,
                order_number: order.order_number,
                total_photos,
                total_bytes,
                subjects,
                missing_files,
            },
            copy_items,
        ))
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
             WHERE subject_id = ?1 AND +status = 'ready'
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

    /// Samples up to `max_samples` already-generated `1280px` preview paths for a subject,
    /// spread across start/middle/finish frames so Vision sees standing salute & clear poses.
    pub fn sample_subject_preview_paths(
        &self,
        subject_id: &str,
        max_samples: usize,
    ) -> Result<Vec<PathBuf>> {
        if max_samples == 0 {
            return Ok(Vec::new());
        }
        let conn = self.read_conn()?;
        let mut stmt = conn.prepare(
            "SELECT preview_rel_path FROM photos
             WHERE subject_id = ?1 AND +status = 'ready' AND preview_rel_path IS NOT NULL
             ORDER BY filename ASC",
        )?;
        let all_paths: Vec<PathBuf> = stmt
            .query_map([subject_id], |r| r.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .map(|rel| self.root.join(rel))
            .filter(|p| p.is_file())
            .collect();

        let n = all_paths.len();
        if n <= max_samples {
            return Ok(all_paths);
        }

        // Spread indices across start, middle, and finish frames (e.g. 0, 1, n/6, n/3, n/2, 2n/3, n-2, n-1)
        let mut sampled = Vec::with_capacity(max_samples);
        let mut seen = HashSet::new();
        for i in 0..max_samples {
            let idx = if i == 0 {
                0
            } else if i == 1 {
                1.min(n - 1)
            } else if i == max_samples - 1 {
                n - 1
            } else if i == max_samples - 2 {
                n.saturating_sub(2)
            } else {
                ((i - 1) * (n - 1)) / (max_samples - 2)
            };
            if seen.insert(idx) {
                sampled.push(all_paths[idx].clone());
            }
        }
        Ok(sampled)
    }

    /// Lists all active subject IDs that currently have at least one `ready` preview photo.
    pub fn list_subject_ids_with_ready_photos(&self) -> Result<Vec<String>> {
        let conn = self.read_conn()?;
        let mut stmt = conn.prepare(
            "SELECT DISTINCT subject_id FROM photos
             WHERE subject_id IS NOT NULL AND +status = 'ready' AND preview_rel_path IS NOT NULL",
        )?;
        let ids: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(ids)
    }

    /// Fast fallback thumbnail path for a subject when a Vision face avatar isn't cached yet.
    pub fn get_subject_fallback_thumbnail_path(&self, subject_id: &str) -> Result<Option<PathBuf>> {
        let conn = self.read_conn()?;
        let rel: Option<String> = conn
            .query_row(
                "SELECT thumbnail_rel_path FROM photos
                 WHERE subject_id = ?1 AND +status = 'ready' AND thumbnail_rel_path IS NOT NULL
                 ORDER BY filename ASC
                 LIMIT 1",
                [subject_id],
                |r| r.get(0),
            )
            .optional()?;
        Ok(rel.map(|r| self.root.join(r)))
    }
}

fn is_jpeg_path(path: &Path) -> bool {
    path.extension()
        .map(|ext| {
            let lower = ext.to_string_lossy().to_lowercase();
            lower == "jpg" || lower == "jpeg"
        })
        .unwrap_or(false)
}

fn has_hidden_or_internal_segment(rel: &str) -> bool {
    rel.split('/')
        .any(|seg| seg.starts_with('.') || seg.starts_with('_'))
}

/// Normalizes and deduplicates relative folder prefixes so ancestor prefixes subsume
/// any nested descendants (and `""` subsumes everything for a full-event reconcile).
fn collapse_reconcile_prefixes(raw: &[String]) -> Vec<String> {
    let mut cleaned: Vec<String> = Vec::with_capacity(raw.len());
    for item in raw {
        let norm = item.replace('\\', "/").trim_matches('/').to_string();
        if !norm.is_empty() && has_hidden_or_internal_segment(&norm) {
            continue;
        }
        cleaned.push(norm);
    }
    cleaned.sort_by(|a, b| a.len().cmp(&b.len()).then_with(|| a.cmp(b)));
    cleaned.dedup();

    let mut collapsed: Vec<String> = Vec::new();
    for p in cleaned {
        let covered = collapsed.iter().any(|anc| {
            anc.is_empty()
                || p == *anc
                || (p.len() > anc.len()
                    && p.starts_with(anc.as_str())
                    && p.as_bytes()[anc.len()] == b'/')
        });
        if !covered {
            collapsed.push(p);
        }
    }
    collapsed
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

fn validate_event_rel_folder(folder_path: &str) -> Result<String> {
    let norm = folder_path.trim().replace('\\', "/");
    let norm = norm.trim_matches('/').to_string();
    if norm.is_empty() {
        return Err(Error::InvalidInput("Folder path cannot be empty".into()));
    }
    for seg in norm.split('/') {
        if seg.is_empty() || seg == "." || seg == ".." {
            return Err(Error::InvalidInput(format!(
                "Invalid folder path '{}'",
                folder_path
            )));
        }
    }
    if norm == "_photofinish" || norm.starts_with("_photofinish/") {
        return Err(Error::InvalidInput(
            "Cannot modify internal _photofinish directory".into(),
        ));
    }
    Ok(norm)
}

fn dir_has_subdirectories(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    entries
        .flatten()
        .any(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
}

fn unique_filename_in_dir(dir: &Path, filename: &str) -> String {
    if !dir.join(filename).exists() {
        return filename.to_string();
    }
    let path = Path::new(filename);
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| filename.to_string());
    let ext = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    for n in 2..10_000 {
        let candidate = format!("{}_{}{}", stem, n, ext);
        if !dir.join(&candidate).exists() {
            return candidate;
        }
    }
    format!("{}_{}{}", stem, id::generate("dup_"), ext)
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

    #[test]
    fn batch_reassign_handles_cyclic_rotation_swap_and_safe_merge_with_collision() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("rotation-swap");
        let bundle = EventBundle::create(CreateEventParams {
            root_path: root.clone(),
            name: "Rotation Swap Test".into(),
            description: None,
            preset_id: Some("gymnastics".into()),
            subject_label: None,
            folder_levels: None,
            order_code: Some("ROT".into()),
            station_code: Some("A".into()),
            tax_rate_basis_points: None,
            scaffold: None,
        })
        .unwrap();

        let sub_a = bundle
            .add_subject(Some("105"), "Julia", Some("B"), Some("Session 10A"), None)
            .unwrap();
        let sub_b = bundle
            .add_subject(Some("107"), "Reece", Some("C"), Some("Session 10A"), None)
            .unwrap();
        let sub_c = bundle
            .add_subject(Some("117"), "Faith", Some("M"), Some("Session 10A"), None)
            .unwrap();

        // Create 3 Beam folders (out of order!) + 1 Floor folder for Julia (to verify group detection & cross-apparatus sample photo)
        let files = [
            ("Gym A/Session 10A/Group 1A/Beam/105 Julia B/IMG_001.jpg", b"actually-reece".as_slice()),
            ("Gym A/Session 10A/Group 1A/Beam/107 Reece C/IMG_001.jpg", b"actually-faith".as_slice()),
            ("Gym A/Session 10A/Group 1A/Beam/117 Faith M/IMG_001.jpg", b"actually-julia".as_slice()),
            ("Gym A/Session 10A/Group 1A/Floor/105 Julia B/FLR_001.jpg", b"julia-floor".as_slice()),
        ];
        for (rel, bytes) in files {
            let p = root.join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(&p, bytes).unwrap();
        }
        let scan = bundle.scan_directory().unwrap();
        assert_eq!(scan.photos_new, 4);
        for job in &scan.pending_jobs {
            bundle
                .mark_photo_ready(
                    &job.photo_id,
                    &format!("_photofinish/thumbnails/{}.jpg", job.photo_id),
                    &format!("_photofinish/previews/{}.jpg", job.photo_id),
                    800,
                    600,
                )
                .unwrap();
        }

        // Verify list_folders detects Session 10A, Group 1A, and cross-apparatus sample photo for Julia
        let listed = bundle
            .list_folders(Some("Gym A/Session 10A/Group 1A/Beam"), false, None, None)
            .unwrap();
        assert_eq!(listed.folders.len(), 3);
        assert_eq!(listed.detected_session_group.as_deref(), Some("Session 10A"));
        assert_eq!(
            listed.detected_group_prefix.as_deref(),
            Some("Gym A/Session 10A/Group 1A")
        );
        assert_eq!(listed.same_group_subject_ids.len(), 3);
        // Julia's reference sample photo should come from Floor (outside Beam!)
        let julia_sample_id = listed.subject_sample_photos.get(&sub_a.id).unwrap();
        let julia_floor_job = scan
            .pending_jobs
            .iter()
            .find(|j| j.relative_path.contains("FLR_001.jpg"))
            .unwrap();
        assert_eq!(julia_sample_id, &julia_floor_job.photo_id);

        // Cyclic 3-way swap on Beam: 105 -> 107, 107 -> 117, 117 -> 105
        let res = bundle
            .apply_folder_reassignments(
                &[
                    FolderReassignmentInput {
                        folder_path: "Gym A/Session 10A/Group 1A/Beam/105 Julia B".into(),
                        target_subject_id: Some(sub_b.id.clone()),
                    },
                    FolderReassignmentInput {
                        folder_path: "Gym A/Session 10A/Group 1A/Beam/107 Reece C".into(),
                        target_subject_id: Some(sub_c.id.clone()),
                    },
                    FolderReassignmentInput {
                        folder_path: "Gym A/Session 10A/Group 1A/Beam/117 Faith M".into(),
                        target_subject_id: Some(sub_a.id.clone()),
                    },
                ],
                true,
            )
            .unwrap();
        assert_eq!(res.folders_updated, 3);
        assert_eq!(res.photos_updated, 3);
        assert!(res.renamed_on_disk);
        assert!(res.changes.iter().all(|c| c.action == "swapped"));

        // Verify exact file contents landed in the right folders on disk with ZERO mixing!
        assert_eq!(
            fs::read(root.join("Gym A/Session 10A/Group 1A/Beam/107 Reece C/IMG_001.jpg")).unwrap(),
            b"actually-reece"
        );
        assert_eq!(
            fs::read(root.join("Gym A/Session 10A/Group 1A/Beam/117 Faith M/IMG_001.jpg")).unwrap(),
            b"actually-faith"
        );
        assert_eq!(
            fs::read(root.join("Gym A/Session 10A/Group 1A/Beam/105 Julia B/IMG_001.jpg")).unwrap(),
            b"actually-julia"
        );

        // Now add an unassigned extra folder with colliding filename IMG_001.jpg and merge into 105 Julia B
        let extra = root.join("Gym A/Session 10A/Group 1A/Beam/Extra/IMG_001.jpg");
        fs::create_dir_all(extra.parent().unwrap()).unwrap();
        fs::write(&extra, b"julia-extra").unwrap();
        bundle.scan_directory().unwrap();

        let merge_res = bundle
            .apply_folder_reassignments(
                &[FolderReassignmentInput {
                    folder_path: "Gym A/Session 10A/Group 1A/Beam/Extra".into(),
                    target_subject_id: Some(sub_a.id.clone()),
                }],
                true,
            )
            .unwrap();
        assert_eq!(merge_res.changes[0].action, "merged");
        assert_eq!(
            fs::read(root.join("Gym A/Session 10A/Group 1A/Beam/105 Julia B/IMG_001.jpg")).unwrap(),
            b"actually-julia"
        );
        assert_eq!(
            fs::read(root.join("Gym A/Session 10A/Group 1A/Beam/105 Julia B/IMG_001_2.jpg"))
                .unwrap(),
            b"julia-extra"
        );
    }

    #[test]
    fn remove_folder_supports_trash_permanent_delete_and_db_only() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("remove-folder-test");
        let bundle = EventBundle::create(CreateEventParams {
            root_path: root.clone(),
            name: "Remove Folder Test".into(),
            description: None,
            preset_id: Some("gymnastics".into()),
            subject_label: None,
            folder_levels: None,
            order_code: Some("REM".into()),
            station_code: Some("A".into()),
            tax_rate_basis_points: None,
            scaffold: None,
        })
        .unwrap();

        let f_trash = root.join("Gym B/Session 2B/Group 1B/Beam/Delete/IMG_1.jpg");
        let f_perm = root.join("Gym B/Session 2B/Group 1B/Beam/Junk/IMG_2.jpg");
        let f_db = root.join("Gym B/Session 2B/Group 1B/Beam/KeepDisk/IMG_3.jpg");
        for p in [&f_trash, &f_perm, &f_db] {
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, b"jpgdata").unwrap();
        }
        let scan = bundle.scan_directory().unwrap();
        assert_eq!(scan.photos_new, 3);

        // 1. Trash mode: moves out of event tree into _photofinish/trash, never re-scanned!
        let r1 = bundle
            .remove_folder("Gym B/Session 2B/Group 1B/Beam/Delete", RemoveFolderMode::Trash)
            .unwrap();
        assert_eq!(r1.photos_removed, 1);
        assert!(!f_trash.exists());
        let trashed_path = root.join(r1.trashed_to.unwrap());
        assert!(trashed_path.join("IMG_1.jpg").is_file());

        // 2. Permanent delete mode: removed from disk & DB
        let r2 = bundle
            .remove_folder("Gym B/Session 2B/Group 1B/Beam/Junk", RemoveFolderMode::DeleteDisk)
            .unwrap();
        assert_eq!(r2.photos_removed, 1);
        assert!(!f_perm.parent().unwrap().exists());

        // 3. DB-only mode: removed from DB, remains on disk
        let r3 = bundle
            .remove_folder("Gym B/Session 2B/Group 1B/Beam/KeepDisk", RemoveFolderMode::DbOnly)
            .unwrap();
        assert_eq!(r3.photos_removed, 1);
        assert!(f_db.is_file());

        // Re-scanning only re-discovers KeepDisk (1 photo), NEVER Delete (trashed) or Junk (deleted)!
        let rescan = bundle.scan_directory().unwrap();
        assert_eq!(rescan.photos_new, 1);
        assert_eq!(rescan.photos_found, 1);
    }

    #[test]
    fn order_desk_payment_usb_fulfillment_pricing_and_csv_export() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("orders-meet");
        let usb_dir = dir.path().join("mock-usb-drive");

        let bundle = EventBundle::create(CreateEventParams {
            root_path: root.clone(),
            name: "State Finals".into(),
            description: None,
            preset_id: Some("gymnastics".into()),
            subject_label: None,
            folder_levels: None,
            order_code: Some("STF".into()),
            station_code: Some("A".into()),
            tax_rate_basis_points: Some(850),
            scaffold: None,
        })
        .unwrap();

        let sub = bundle
            .add_subject(Some("105"), "Julia", Some("B"), Some("Session 1A"), None)
            .unwrap();

        let p_beam = root.join("Gym A/Session 1A/Group 1A/Beam/105 Julia B/IMG_1001.jpg");
        let p_floor = root.join("Gym A/Session 1A/Group 1A/Floor/105 Julia B/IMG_2002.jpg");
        for (p, data) in [(&p_beam, b"beam-orig".as_slice()), (&p_floor, b"floor-orig".as_slice())] {
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, data).unwrap();
        }
        let scan = bundle.scan_directory().unwrap();
        for job in &scan.pending_jobs {
            bundle
                .mark_photo_ready(
                    &job.photo_id,
                    &format!("_photofinish/thumbnails/{}.jpg", job.photo_id),
                    &format!("_photofinish/previews/{}.jpg", job.photo_id),
                    1200,
                    800,
                )
                .unwrap();
        }

        // Update pricing + settings
        let products = bundle.list_products(true).unwrap();
        let full_prod = products.iter().find(|p| p.product_type == "full_digital").unwrap();
        let usb_prod = products.iter().find(|p| p.product_type == "usb_addon").unwrap();

        bundle
            .upsert_product(&ProductUpsertInput {
                id: Some(full_prod.id.clone()),
                product_type: full_prod.product_type.clone(),
                name: full_prod.name.clone(),
                size: None,
                price_cents: 9000,
                is_available: true,
                display_order: full_prod.display_order,
            })
            .unwrap();
        let updated_summary = bundle
            .update_event_order_settings(&EventOrderSettingsInput {
                order_code: "FIN".into(),
                station_code: "B".into(),
                tax_rate_basis_points: 700,
            })
            .unwrap();
        assert_eq!(updated_summary.order_code, "FIN");
        assert_eq!(updated_summary.tax_rate_basis_points, 700);

        // Place order with Full Digital ($90) + USB Add-On ($10) = $100 subtotal + $7 tax = $107
        let order = bundle
            .place_order(CreateOrderRequest {
                customer_name: "Joe Franklin".into(),
                customer_email: Some("joe@example.com".into()),
                customer_phone: Some("555-1234".into()),
                shipping_address: None,
                notes: Some("Pick up at desk".into()),
                items: vec![
                    crate::orders::CreateOrderItemInput {
                        product_id: full_prod.id.clone(),
                        subject_id: Some(sub.id.clone()),
                        photo_id: None,
                        quantity: 1,
                    },
                    crate::orders::CreateOrderItemInput {
                        product_id: usb_prod.id.clone(),
                        subject_id: Some(sub.id.clone()),
                        photo_id: None,
                        quantity: 1,
                    },
                ],
            })
            .unwrap();
        assert_eq!(order.order_number, "FIN-B0001");
        assert_eq!(order.total_cents, 10700);

        // Stats before payment
        let before = bundle.list_orders().unwrap();
        assert_eq!(before.stats.pending_payment_count, 1);
        assert_eq!(before.stats.pending_usb_count, 1);
        assert_eq!(before.stats.gross_paid_cents, 0);

        // Mark Paid
        let paid = bundle
            .update_order_payment(&UpdatePaymentRequest {
                order_id: order.id.clone(),
                payment_status: "paid".into(),
                payment_method: Some("square_qr".into()),
                payment_reference: Some("SQ-9876".into()),
                notes: None,
            })
            .unwrap();
        assert_eq!(paid.payment_status, "paid");
        assert!(paid.paid_at.is_some());

        // Resolve USB plan & copy files to mock USB drive
        let (plan, copy_items) = bundle.resolve_order_usb_copy_plan(&order.id).unwrap();
        assert_eq!(plan.total_photos, 2);
        assert_eq!(plan.missing_files, 0);
        assert_eq!(plan.subjects.len(), 1);
        assert_eq!(plan.subjects[0].subject_label, "105 Julia B");

        let (copied_count, _, _) =
            crate::hardware::copy_order_files_to_usb(&usb_dir, &copy_items, |_| {}).unwrap();
        assert_eq!(copied_count, 2);
        assert_eq!(
            fs::read(usb_dir.join("105 Julia B/Beam/IMG_1001.jpg")).unwrap(),
            b"beam-orig"
        );
        assert_eq!(
            fs::read(usb_dir.join("105 Julia B/Floor/IMG_2002.jpg")).unwrap(),
            b"floor-orig"
        );

        let marked = bundle.mark_usb_items_fulfilled(&order.id).unwrap();
        assert_eq!(marked, 1);

        let after = bundle.list_orders().unwrap();
        assert_eq!(after.stats.paid_count, 1);
        assert_eq!(after.stats.pending_usb_count, 0);
        assert_eq!(after.stats.gross_paid_cents, 10700);

        // CSV Export verification
        let csv = orders::format_orders_csv(&[("State Finals".into(), after.orders)]);
        assert!(csv.contains("FIN-B0001"));
        assert!(csv.contains("Joe Franklin"));
        assert!(csv.contains("105 Julia B"));
        assert!(csv.contains("107.00"));
    }

    #[test]
    fn multi_station_gather_and_consolidated_csv_merge() {
        let nas_dir = tempdir().unwrap();

        // Simulate a nested Sunday-night NAS gather layout across 2 stations at the same meet
        // plus a second meet at another venue
        let station_a_root = nas_dir.path().join("Weekend-03/Station-A/2026 IL State");
        let station_b_root = nas_dir.path().join("Weekend-03/Station-B/2026 IL State");
        let station_c_root = nas_dir.path().join("Weekend-03/Station-C/2026 WI State");

        let bundle_a = EventBundle::create(CreateEventParams {
            root_path: station_a_root.clone(),
            name: "2026 IL State (Gym A)".into(),
            description: None,
            preset_id: Some("gymnastics".into()),
            subject_label: None,
            folder_levels: None,
            order_code: Some("ILS".into()),
            station_code: Some("A".into()),
            tax_rate_basis_points: Some(800),
            scaffold: None,
        })
        .unwrap();

        let bundle_b = EventBundle::create(CreateEventParams {
            root_path: station_b_root.clone(),
            name: "2026 IL State (Gym B)".into(),
            description: None,
            preset_id: Some("gymnastics".into()),
            subject_label: None,
            folder_levels: None,
            order_code: Some("ILS".into()),
            station_code: Some("B".into()),
            tax_rate_basis_points: Some(800),
            scaffold: None,
        })
        .unwrap();

        let _bundle_c = EventBundle::create(CreateEventParams {
            root_path: station_c_root.clone(),
            name: "2026 WI State".into(),
            description: None,
            preset_id: Some("gymnastics".into()),
            subject_label: None,
            folder_levels: None,
            order_code: Some("WIS".into()),
            station_code: Some("C".into()),
            tax_rate_basis_points: Some(550),
            scaffold: None,
        })
        .unwrap();

        // Place paid orders on Station A and Station B for the same meet
        let prod_a = bundle_a.list_products(false).unwrap()[0].clone();
        let sub_a = bundle_a.add_subject(Some("101"), "Ava", Some("M"), None, None).unwrap();
        let ord_a = bundle_a
            .place_order(CreateOrderRequest {
                customer_name: "Parent A".into(),
                customer_email: Some("a@example.com".into()),
                customer_phone: None,
                shipping_address: None,
                notes: None,
                items: vec![crate::orders::CreateOrderItemInput {
                    product_id: prod_a.id,
                    subject_id: Some(sub_a.id),
                    photo_id: None,
                    quantity: 1,
                }],
            })
            .unwrap();
        bundle_a
            .update_order_payment(&UpdatePaymentRequest {
                order_id: ord_a.id,
                payment_status: "paid".into(),
                payment_method: Some("square_qr".into()),
                payment_reference: Some("SQ-A1".into()),
                notes: None,
            })
            .unwrap();

        let prod_b = bundle_b.list_products(false).unwrap()[0].clone();
        let sub_b = bundle_b.add_subject(Some("202"), "Chloe", Some("R"), None, None).unwrap();
        let ord_b = bundle_b
            .place_order(CreateOrderRequest {
                customer_name: "Parent B".into(),
                customer_email: Some("b@example.com".into()),
                customer_phone: None,
                shipping_address: None,
                notes: None,
                items: vec![crate::orders::CreateOrderItemInput {
                    product_id: prod_b.id,
                    subject_id: Some(sub_b.id),
                    photo_id: None,
                    quantity: 1,
                }],
            })
            .unwrap();
        bundle_b
            .update_order_payment(&UpdatePaymentRequest {
                order_id: ord_b.id,
                payment_status: "paid".into(),
                payment_method: Some("square_qr".into()),
                payment_reference: Some("SQ-B1".into()),
                notes: None,
            })
            .unwrap();

        // Recursive discovery from top-level NAS folder must find all 3 nested bundles
        let discovered = EventBundle::discover_in_directory(nas_dir.path()).unwrap();
        assert_eq!(discovered.len(), 3);

        // Read-only inspection should report exact station codes, order numbers, and revenue
        let ov_a = EventBundle::inspect_readonly(&station_a_root).unwrap();
        let ov_b = EventBundle::inspect_readonly(&station_b_root).unwrap();
        let ov_c = EventBundle::inspect_readonly(&station_c_root).unwrap();

        assert_eq!(ov_a.station_code, "A");
        assert_eq!(ov_b.station_code, "B");
        assert_eq!(ov_c.station_code, "C");
        assert_eq!(ov_a.order_numbers, vec!["ILS-A0001"]);
        assert_eq!(ov_b.order_numbers, vec!["ILS-B0001"]);
        assert_eq!(ov_a.paid_order_count, 1);
        assert_eq!(ov_b.paid_order_count, 1);
        assert!(ov_a.gross_paid_cents > 0);
        assert_ne!(ov_a.order_numbers[0], ov_b.order_numbers[0]);

        // Consolidated CSV merges all stations cleanly
        let csv = orders::format_orders_csv(&[
            (ov_a.name.clone(), bundle_a.list_orders().unwrap().orders),
            (ov_b.name.clone(), bundle_b.list_orders().unwrap().orders),
        ]);
        assert!(csv.contains("ILS-A0001"));
        assert!(csv.contains("ILS-B0001"));
        assert!(csv.contains("Parent A"));
        assert!(csv.contains("Parent B"));
    }

    #[test]
    fn scan_and_reconcile_prunes_deleted_folders_and_cached_derivatives() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("orphan-meet");
        let bundle = EventBundle::create(CreateEventParams {
            root_path: root.clone(),
            name: "Orphan Cleanup Meet".into(),
            description: None,
            preset_id: Some("gymnastics".into()),
            subject_label: None,
            folder_levels: None,
            order_code: Some("OCM".into()),
            station_code: Some("A".into()),
            tax_rate_basis_points: None,
            scaffold: None,
        })
        .unwrap();

        let sub_keep = bundle
            .add_subject(Some("101"), "Maya", Some("K"), Some("Session 10A"), None)
            .unwrap();
        let sub_del = bundle
            .add_subject(Some("180"), "Zoe", Some("D"), Some("Session 18A"), None)
            .unwrap();

        let keep_file = root.join("Gym A/Session 10A/Group 1A/Beam/101 Maya K/IMG_001.jpg");
        let del_file1 = root.join("Gym A/Session 18A/Group 1A/Beam/180 Zoe D/IMG_002.jpg");
        let del_file2 = root.join("Gym A/Session 18A/Group 1A/Floor/180 Zoe D/IMG_003.jpg");
        for f in [&keep_file, &del_file1, &del_file2] {
            fs::create_dir_all(f.parent().unwrap()).unwrap();
            fs::write(f, b"fake-jpeg-bytes").unwrap();
        }

        let scan1 = bundle.scan_directory().unwrap();
        assert_eq!(scan1.photos_new, 3);
        assert_eq!(scan1.photos_removed, 0);

        // Mark all photos ready with mock thumbnails & previews
        let mut del_thumbs = Vec::new();
        for job in &scan1.pending_jobs {
            let t_rel = format!("_photofinish/thumbnails/{}.jpg", job.photo_id);
            let p_rel = format!("_photofinish/previews/{}.jpg", job.photo_id);
            fs::write(root.join(&t_rel), b"thumb").unwrap();
            fs::write(root.join(&p_rel), b"prev").unwrap();
            bundle
                .mark_photo_ready(&job.photo_id, &t_rel, &p_rel, 1280, 853)
                .unwrap();
            if job.relative_path.contains("Session 18A") {
                del_thumbs.push((t_rel, p_rel));
            }
        }

        // Attach a single-photo order item to one of the Session 18A photos to verify
        // pruning never violates foreign keys (ON DELETE SET NULL keeps order history intact)
        let del_photo_id = bundle.list_subject_photos(&sub_del.id).unwrap()[0].id.clone();
        let single_prod = bundle
            .list_products(false)
            .unwrap()
            .into_iter()
            .find(|p| p.product_type == "single_digital")
            .unwrap();
        bundle
            .place_order(CreateOrderRequest {
                customer_name: "Order Parent".into(),
                customer_email: None,
                customer_phone: None,
                shipping_address: None,
                notes: None,
                items: vec![crate::orders::CreateOrderItemInput {
                    product_id: single_prod.id,
                    subject_id: Some(sub_del.id.clone()),
                    photo_id: Some(del_photo_id),
                    quantity: 1,
                }],
            })
            .unwrap();

        // Delete Session 18A in Finder
        fs::remove_dir_all(root.join("Gym A/Session 18A")).unwrap();

        // Full scan_directory prunes ONLY Session 18A + its thumbnails/previews, leaving Session 10A ready
        let scan2 = bundle.scan_directory().unwrap();
        assert_eq!(scan2.photos_found, 1);
        assert_eq!(scan2.photos_new, 0);
        assert_eq!(scan2.photos_skipped, 1);
        assert_eq!(scan2.photos_removed, 2);

        let counts = bundle.photo_counts().unwrap();
        assert_eq!(counts.total, 1);
        assert_eq!(counts.ready, 1);
        assert_eq!(bundle.list_subject_photos(&sub_keep.id).unwrap().len(), 1);
        assert_eq!(bundle.list_subject_photos(&sub_del.id).unwrap().len(), 0);
        for (t_rel, p_rel) in &del_thumbs {
            assert!(!root.join(t_rel).exists());
            assert!(!root.join(p_rel).exists());
        }
        // Order history preserved cleanly
        assert_eq!(bundle.list_orders().unwrap().orders.len(), 1);
    }

    #[test]
    fn targeted_reconcile_subpaths_handles_add_delete_and_rename_without_touching_siblings() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("targeted-meet");
        let bundle = EventBundle::create(CreateEventParams {
            root_path: root.clone(),
            name: "Targeted Reconcile Meet".into(),
            description: None,
            preset_id: Some("gymnastics".into()),
            subject_label: None,
            folder_levels: None,
            order_code: Some("TRM".into()),
            station_code: Some("A".into()),
            tax_rate_basis_points: None,
            scaffold: None,
        })
        .unwrap();

        let sub105 = bundle
            .add_subject(Some("105"), "Julia", Some("B"), Some("Session 10A"), None)
            .unwrap();

        let keep = root.join("Gym A/Session 10A/Group 1A/Beam/101 Keep/IMG_001.jpg");
        let old_dir = root.join("Gym A/Session 10A/Group 1A/Beam/TempFolder");
        fs::create_dir_all(keep.parent().unwrap()).unwrap();
        fs::create_dir_all(&old_dir).unwrap();
        fs::write(&keep, b"keep-jpg").unwrap();
        fs::write(old_dir.join("IMG_002.jpg"), b"rename-jpg").unwrap();

        assert_eq!(bundle.scan_directory().unwrap().photos_new, 2);

        // Simulate Finder renaming TempFolder -> "105 Julia B" + dropping a 0-byte in-progress file
        let new_dir = root.join("Gym A/Session 10A/Group 1A/Beam/105 Julia B");
        fs::rename(&old_dir, &new_dir).unwrap();
        fs::write(new_dir.join("IMG_IN_PROGRESS.jpg"), b"").unwrap();

        let res = bundle
            .reconcile_subpaths(&[
                "Gym A/Session 10A/Group 1A/Beam/TempFolder".into(),
                "Gym A/Session 10A/Group 1A/Beam/105 Julia B".into(),
            ])
            .unwrap();
        assert_eq!(res.photos_removed, 1);
        assert_eq!(res.photos_new, 1); // 0-byte file skipped!
        assert_eq!(res.pending_jobs.len(), 1);
        assert_eq!(bundle.photo_counts().unwrap().total, 2);

        // Once the imaging pipeline marks the newly indexed photo ready, it appears in 105 Julia B's gallery
        bundle
            .mark_photo_ready(
                &res.pending_jobs[0].photo_id,
                "_photofinish/thumbnails/t.jpg",
                "_photofinish/previews/p.jpg",
                1280,
                853,
            )
            .unwrap();
        assert_eq!(bundle.list_subject_photos(&sub105.id).unwrap().len(), 1);
    }

    #[test]
    fn ingest_batch_queue_scoping_and_section_matched_narrowing() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("queue-meet");
        let bundle = EventBundle::create(CreateEventParams {
            root_path: root.clone(),
            name: "Queue Narrowing Meet".into(),
            description: None,
            preset_id: Some("gymnastics".into()),
            subject_label: None,
            folder_levels: None,
            order_code: Some("QNM".into()),
            station_code: Some("A".into()),
            tax_rate_basis_points: None,
            scaffold: None,
        })
        .unwrap();

        let sub101 = bundle
            .add_subject(Some("101"), "Ava", Some("M"), Some("Session 10A"), None)
            .unwrap();
        let sub102 = bundle
            .add_subject(Some("102"), "Bella", Some("S"), Some("Session 10A"), None)
            .unwrap();

        // Rotation 1 on Beam (Group 1A): Ava (101) already matched
        let f1 = "Gym A/Session 10A/Group 1A/Beam/101 Ava M/IMG_001.jpg";
        fs::create_dir_all(root.join(f1).parent().unwrap()).unwrap();
        fs::write(root.join(f1), b"jpg1").unwrap();
        let (batch1, _) = bundle
            .record_ingest_batch(
                IngestBatchMeta {
                    reader_id: Some("USB-BEAM".into()),
                    reader_name: Some("Gym A · Beam".into()),
                    volume_name: Some("CARD1".into()),
                    photographer: Some("KDS".into()),
                    source_path: None,
                    destination_rel_path: "Gym A/Session 10A/Group 1A/Beam".into(),
                    tags: BTreeMap::new(),
                },
                &[IngestFileInput {
                    relative_path: f1.into(),
                    original_filename: "IMG_001.jpg".into(),
                    file_size_bytes: 4,
                }],
            )
            .unwrap();

        // Rotation 2 on Beam (Group 2A): unnamed folder "Gymnast 01"
        let f2 = "Gym A/Session 10A/Group 2A/Beam/Gymnast 01/IMG_002.jpg";
        fs::create_dir_all(root.join(f2).parent().unwrap()).unwrap();
        fs::write(root.join(f2), b"jpg2").unwrap();
        let (batch2, _) = bundle
            .record_ingest_batch(
                IngestBatchMeta {
                    reader_id: Some("USB-BEAM".into()),
                    reader_name: Some("Gym A · Beam".into()),
                    volume_name: Some("CARD2".into()),
                    photographer: Some("KDS".into()),
                    source_path: None,
                    destination_rel_path: "Gym A/Session 10A/Group 2A/Beam".into(),
                    tags: BTreeMap::new(),
                },
                &[IngestFileInput {
                    relative_path: f2.into(),
                    original_filename: "IMG_002.jpg".into(),
                    file_size_bytes: 4,
                }],
            )
            .unwrap();

        // Queue should show batch2 with 1 unassigned folder and batch1 with 0 unassigned
        let batches = bundle.list_ingest_batches(20).unwrap();
        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].id, batch2);
        assert_eq!(batches[0].unassigned_folders, 1);
        assert_eq!(batches[1].id, batch1);
        assert_eq!(batches[1].unassigned_folders, 0);

        // Scoping list_folders to batch2 shows ONLY batch2's folder AND flags sub101 as
        // already matched on Beam in Group 1A (so the picker can grey out Ava!)
        let view = bundle
            .list_folders(None, false, None, Some(&batch2))
            .unwrap();
        assert_eq!(view.folders.len(), 1);
        assert_eq!(view.folders[0].leaf_folder, "Gymnast 01");
        assert_eq!(view.section_label.as_deref(), Some("Beam"));
        assert_eq!(
            view.section_matched_subjects.get(&sub101.id).map(String::as_str),
            Some("Group 1A")
        );
        assert!(!view.section_matched_subjects.contains_key(&sub102.id));

        // After assigning batch2's folder to Bella (102), batch2's unassigned_folders drops to 0
        bundle
            .apply_folder_reassignments(
                &[FolderReassignmentInput {
                    folder_path: "Gym A/Session 10A/Group 2A/Beam/Gymnast 01".into(),
                    target_subject_id: Some(sub102.id.clone()),
                }],
                true,
            )
            .unwrap();
        let after_batches = bundle.list_ingest_batches(20).unwrap();
        assert_eq!(after_batches[0].unassigned_folders, 0);

        // In Card View with `only_unassigned = true`, completed card cleanly returns 0 folders,
        // while `only_unassigned = false` still allows reviewing all named folders on that card.
        let card_remaining = bundle
            .list_folders(None, true, None, Some(&batch2))
            .unwrap();
        assert_eq!(card_remaining.folders.len(), 0);
        let card_all = bundle
            .list_folders(None, false, None, Some(&batch2))
            .unwrap();
        assert_eq!(card_all.folders.len(), 1);
        assert_eq!(card_all.folders[0].assigned_subject_id.as_deref(), Some(sub102.id.as_str()));
    }
}
