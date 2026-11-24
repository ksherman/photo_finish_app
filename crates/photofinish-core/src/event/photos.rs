use rusqlite::OptionalExtension;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use super::{
    extract_section_label, load_folder_levels, load_subject_number_map, section_level_key,
    EventBundle, IngestBatchMeta, IngestFileInput, PendingPhotoJob, PhotoProcessUpdate,
    PhotoRecord, ScanResult,
};
use crate::id;
use crate::preset::{extract_subject_number, parse_relative_location};
use crate::util::{folder_prefix_range, is_jpeg_path, now_iso};
use crate::{Error, Result};

const SQL_MARK_PHOTO_READY: &str = "UPDATE photos
    SET status = 'ready', thumbnail_rel_path = ?2, preview_rel_path = ?3,
        width = ?4, height = ?5, error_message = NULL, updated_at = ?6
    WHERE id = ?1";

const SQL_MARK_PHOTO_ERROR: &str =
    "UPDATE photos SET status = 'error', error_message = ?2, updated_at = ?3 WHERE id = ?1";

impl EventBundle {
    /// Records a card-ingest batch and inserts all newly copied photos in one transaction.
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
        let now = now_iso();
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
                let mut merged_tags = parsed.tags;
                merged_tags.extend(meta.tags.iter().map(|(k, v)| (k.clone(), v.clone())));
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

    /// Two-way full-event reconcile: discovers new JPEGs AND prunes orphaned DB rows.
    pub fn scan_directory(&self) -> Result<ScanResult> {
        self.reconcile_subpaths(&[String::new()])
    }

    /// Reconciles one or more relative directory subpaths inside the event bundle against SQLite.
    pub fn reconcile_subpaths(&self, rel_prefixes: &[String]) -> Result<ScanResult> {
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
            let mut discovered: HashMap<String, IngestFileInput> = HashMap::new();
            let walk_target = if prefix.is_empty() {
                self.root.clone()
            } else {
                self.root.join(prefix)
            };

            let record_file = |path: &Path, discovered: &mut HashMap<String, IngestFileInput>| {
                if !is_jpeg_path(path) {
                    return;
                }
                let size = path.metadata().map(|m| m.len() as i64).unwrap_or(0);
                if size == 0 {
                    return;
                }
                if let Ok(rel) = path.strip_prefix(&self.root) {
                    let rel_str = rel.to_string_lossy().replace('\\', "/");
                    if !has_hidden_or_internal_segment(&rel_str) {
                        let original_filename = path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();
                        discovered.insert(
                            rel_str.clone(),
                            IngestFileInput {
                                relative_path: rel_str,
                                original_filename,
                                file_size_bytes: size,
                            },
                        );
                    }
                }
            };

            if walk_target.is_dir() {
                for entry in WalkDir::new(&walk_target)
                    .into_iter()
                    .filter_entry(|e| {
                        e.depth() == 0 || {
                            let name = e.file_name().to_string_lossy();
                            !name.starts_with('.') && !name.starts_with('_')
                        }
                    })
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_type().is_file())
                {
                    record_file(entry.path(), &mut discovered);
                }
            } else if walk_target.is_file() {
                record_file(&walk_target, &mut discovered);
            }

            total_found += discovered.len();

            let mut conn = self.write_conn.lock().unwrap();
            let folder_levels = load_folder_levels(&conn)?;
            let subject_lookup = load_subject_number_map(&conn)?;

            struct ExistingRow {
                id: String,
                relative_path: String,
                thumb: Option<String>,
                preview: Option<String>,
            }
            let map_existing = |r: &rusqlite::Row<'_>| {
                Ok(ExistingRow {
                    id: r.get(0)?,
                    relative_path: r.get(1)?,
                    thumb: r.get(2)?,
                    preview: r.get(3)?,
                })
            };

            let existing_rows: Vec<ExistingRow> = if prefix.is_empty() {
                let mut stmt = conn.prepare(
                    "SELECT id, relative_path, thumbnail_rel_path, preview_rel_path FROM photos",
                )?;
                let rows = stmt
                    .query_map([], map_existing)?
                    .collect::<std::result::Result<_, _>>()?;
                rows
            } else {
                let (range_start, range_end) = folder_prefix_range(prefix);
                let mut stmt = conn.prepare(
                    "SELECT id, relative_path, thumbnail_rel_path, preview_rel_path
                     FROM photos
                     WHERE folder_path = ?1
                        OR (folder_path >= ?2 AND folder_path < ?3)",
                )?;
                let rows = stmt
                    .query_map(rusqlite::params![prefix, range_start, range_end], map_existing)?
                    .collect::<std::result::Result<_, _>>()?;
                rows
            };

            let mut existing_paths: HashSet<String> = HashSet::with_capacity(existing_rows.len());
            let mut orphans: Vec<ExistingRow> = Vec::new();
            for row in existing_rows {
                if discovered.contains_key(&row.relative_path)
                    || self.root.join(&row.relative_path).is_file()
                {
                    existing_paths.insert(row.relative_path);
                } else {
                    orphans.push(row);
                }
            }

            let now = now_iso();
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
        let jobs = stmt
            .query_map([], |r| {
                Ok(PendingPhotoJob {
                    photo_id: r.get(0)?,
                    relative_path: r.get(1)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
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
        conn.execute(
            SQL_MARK_PHOTO_READY,
            rusqlite::params![
                photo_id,
                thumbnail_rel_path,
                preview_rel_path,
                width as i64,
                height as i64,
                now_iso(),
            ],
        )?;
        Ok(())
    }

    pub fn mark_photo_error(&self, photo_id: &str, message: &str) -> Result<()> {
        let conn = self.write_conn.lock().unwrap();
        conn.execute(
            SQL_MARK_PHOTO_ERROR,
            rusqlite::params![photo_id, message, now_iso()],
        )?;
        Ok(())
    }

    pub fn mark_photos_batch(&self, updates: &[PhotoProcessUpdate]) -> Result<()> {
        if updates.is_empty() {
            return Ok(());
        }
        let mut conn = self.write_conn.lock().unwrap();
        let now = now_iso();
        let tx = conn.transaction()?;
        {
            let mut ready_stmt = tx.prepare(SQL_MARK_PHOTO_READY)?;
            let mut err_stmt = tx.prepare(SQL_MARK_PHOTO_ERROR)?;

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

    /// Returns a fast random sample of ready photo IDs for the kiosk home marquee
    /// using O(k log N) rowid probes instead of sorting 150K+ rows with `ORDER BY RANDOM()`.
    pub fn list_showcase_photo_ids(&self, count: i64) -> Result<Vec<String>> {
        use rand::RngExt;

        let conn = self.read_conn()?;
        let target = count.clamp(1, 60) as usize;
        let max_rowid: i64 = conn
            .query_row("SELECT COALESCE(MAX(rowid), 0) FROM photos", [], |r| r.get(0))
            .unwrap_or(0);
        if max_rowid <= 0 {
            return Ok(Vec::new());
        }

        if max_rowid <= 500 {
            let mut stmt = conn.prepare(
                "SELECT id FROM photos WHERE status = 'ready' ORDER BY RANDOM() LIMIT ?1",
            )?;
            return Ok(stmt
                .query_map([target as i64], |r| r.get(0))?
                .filter_map(|r| r.ok())
                .collect());
        }

        let mut rng = rand::rng();
        let mut stmt = conn.prepare(
            "SELECT id FROM photos WHERE rowid >= ?1 AND status = 'ready' LIMIT 1",
        )?;
        let mut seen = HashSet::with_capacity(target);
        let mut ids = Vec::with_capacity(target);
        for _ in 0..(target * 4) {
            if ids.len() >= target {
                break;
            }
            let probe = rng.random_range(1..=max_rowid);
            if let Ok(Some(id)) = stmt.query_row([probe], |r| r.get::<_, String>(0)).optional() {
                if seen.insert(id.clone()) {
                    ids.push(id);
                }
            }
        }
        Ok(ids)
    }

    pub fn list_subject_photos(&self, subject_id: &str) -> Result<Vec<PhotoRecord>> {
        let conn = self.read_conn()?;
        let folder_levels = load_folder_levels(&conn).unwrap_or_default();
        let level_key = section_level_key(&folder_levels);

        let mut stmt = conn.prepare(
            "SELECT id, subject_id, relative_path, folder_path, leaf_folder,
                    filename, status, thumbnail_rel_path, preview_rel_path,
                    width, height, tags_json
             FROM photos
             WHERE subject_id = ?1 AND +status = 'ready'
             ORDER BY folder_path ASC, filename ASC",
        )?;
        let out = stmt
            .query_map([subject_id], |r| {
                let folder_path: String = r.get(3)?;
                let tags_json: String = r.get(11)?;
                let tags: BTreeMap<String, String> =
                    serde_json::from_str(&tags_json).unwrap_or_default();
                let section_label = extract_section_label(&folder_path, &tags, level_key.as_deref());

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
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(out)
    }

    pub fn get_photo_derivative_path(
        &self,
        photo_id: &str,
        preview: bool,
    ) -> Result<Option<PathBuf>> {
        let conn = self.read_conn()?;
        let col = if preview {
            "preview_rel_path"
        } else {
            "thumbnail_rel_path"
        };
        let rel: Option<Option<String>> = conn
            .query_row(
                &format!("SELECT {} FROM photos WHERE id = ?1", col),
                [photo_id],
                |r| r.get(0),
            )
            .optional()?;
        Ok(rel.flatten().map(|r| self.root.join(r)))
    }

    /// Samples up to `max_samples` already-generated `1280px` preview paths spread across
    /// start/middle/finish frames so Apple Vision sees standing salute & clear poses.
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

    pub fn list_subject_ids_with_ready_photos(&self) -> Result<Vec<String>> {
        let conn = self.read_conn()?;
        let mut stmt = conn.prepare(
            "SELECT DISTINCT subject_id FROM photos
             WHERE subject_id IS NOT NULL AND +status = 'ready' AND preview_rel_path IS NOT NULL",
        )?;
        let ids = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(ids)
    }

    pub fn get_subject_fallback_thumbnail_path(&self, subject_id: &str) -> Result<Option<PathBuf>> {
        let conn = self.read_conn()?;
        let rel: Option<String> = conn
            .query_row(
                "SELECT thumbnail_rel_path FROM photos
                 WHERE subject_id = ?1 AND +status = 'ready' AND thumbnail_rel_path IS NOT NULL
                 ORDER BY filename ASC LIMIT 1",
                [subject_id],
                |r| r.get(0),
            )
            .optional()?;
        Ok(rel.map(|r| self.root.join(r)))
    }
}

fn has_hidden_or_internal_segment(rel: &str) -> bool {
    rel.split('/')
        .any(|seg| seg.starts_with('.') || seg.starts_with('_'))
}

fn collapse_reconcile_prefixes(raw: &[String]) -> Vec<String> {
    let mut cleaned: Vec<String> = raw
        .iter()
        .map(|item| item.replace('\\', "/").trim_matches('/').to_string())
        .filter(|norm| norm.is_empty() || !has_hidden_or_internal_segment(norm))
        .collect();
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
