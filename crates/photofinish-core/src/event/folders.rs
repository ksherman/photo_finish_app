use chrono::Utc;
use rusqlite::{Connection, OptionalExtension};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use super::{load_folder_levels, EventBundle};
use crate::id;
use crate::preset::parse_relative_location;
use crate::roster::{
    natural_sort_key, BatchReassignResult, FolderChangeResultItem, FolderItem,
    FolderLocationSummary, FolderReassignmentInput, IngestBatchSummary, ListFoldersResponse,
    RemoveFolderMode, RemoveFolderResult,
};
use crate::util::{
    folder_prefix_range, now_iso, sanitize_folder_component, split_parent_and_leaf, trim_non_empty,
    unique_filename_in_dir,
};
use crate::{Error, Result};

struct RawFolderRow {
    folder_path: String,
    parent_path: String,
    leaf_folder: String,
    photo_count: i64,
    unassigned_count: i64,
    min_subject_id: Option<String>,
    max_subject_id: Option<String>,
}

fn query_raw_folders(conn: &Connection, batch_id: Option<&str>) -> Result<Vec<RawFolderRow>> {
    let map_row = |r: &rusqlite::Row<'_>| {
        let folder_path: String = r.get(0)?;
        let (parent, leaf) = split_parent_and_leaf(&folder_path);
        Ok(RawFolderRow {
            parent_path: parent.to_string(),
            leaf_folder: leaf.to_string(),
            folder_path,
            photo_count: r.get(1)?,
            unassigned_count: r.get(2)?,
            min_subject_id: r.get(3)?,
            max_subject_id: r.get(4)?,
        })
    };

    if let Some(bid) = batch_id {
        let mut stmt = conn.prepare(
            "SELECT folder_path, COUNT(*),
                    SUM(CASE WHEN subject_id IS NULL THEN 1 ELSE 0 END),
                    MIN(subject_id), MAX(subject_id)
             FROM photos
             WHERE ingest_batch_id = ?1 AND folder_path != ''
             GROUP BY folder_path",
        )?;
        let rows = stmt.query_map([bid], map_row)?.collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    } else {
        let mut stmt = conn.prepare(
            "SELECT folder_path, COUNT(*),
                    SUM(CASE WHEN subject_id IS NULL THEN 1 ELSE 0 END),
                    MIN(subject_id), MAX(subject_id)
             FROM photos
             WHERE folder_path != ''
             GROUP BY folder_path",
        )?;
        let rows = stmt.query_map([], map_row)?.collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }
}

impl EventBundle {
    /// Lists recent ingested card batches with live photo, thumbnail, and unnamed-folder counts.
    pub fn list_ingest_batches(&self, limit: i64) -> Result<Vec<IngestBatchSummary>> {
        let conn = self.read_conn()?;
        let mut stmt = conn.prepare(
            "SELECT b.id, b.sequence_num, b.reader_id, b.reader_name, b.volume_name,
                    b.photographer, b.destination_rel_path, COALESCE(b.completed_at, b.started_at),
                    COUNT(p.id) AS total_photos,
                    COALESCE(SUM(CASE WHEN p.status = 'ready' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN p.status IN ('discovered', 'processing') THEN 1 ELSE 0 END), 0),
                    COUNT(DISTINCT CASE WHEN p.folder_path != '' THEN p.folder_path END),
                    COUNT(DISTINCT CASE WHEN p.folder_path != '' AND p.subject_id IS NULL THEN p.folder_path END),
                    COALESCE(SUM(CASE WHEN p.subject_id IS NULL THEN 1 ELSE 0 END), 0)
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

    /// Lists photo folders (unassigned-only, all folders within a location, or strictly a
    /// single ingested card via `batch_id`), along with location summaries, inferred session &
    /// group roster context, cross-apparatus reference thumbnails, and already-matched subjects.
    pub fn list_folders(
        &self,
        parent_filter: Option<&str>,
        only_unassigned: bool,
        context_parent: Option<&str>,
        batch_id: Option<&str>,
    ) -> Result<ListFoldersResponse> {
        let conn = self.read_conn()?;

        let subject_map: HashMap<String, (String, Option<String>)> = {
            let mut s_stmt = conn.prepare("SELECT id, display_name, number FROM subjects")?;
            let map = s_stmt
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        (r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?),
                    ))
                })?
                .filter_map(|r| r.ok())
                .collect();
            map
        };

        let raw_rows = query_raw_folders(&conn, None)?;
        let mut loc_map: BTreeMap<String, FolderLocationSummary> = BTreeMap::new();
        let mut total_unassigned_photos = 0i64;
        let mut total_unassigned_folders = 0usize;

        let filter_norm = trim_non_empty(parent_filter);
        let batch_norm = trim_non_empty(batch_id);
        let mut selected_rows: Vec<RawFolderRow> = Vec::new();
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

            if batch_norm.is_none()
                && (!only_unassigned || has_unassigned)
                && filter_norm.is_none_or(|f| row.parent_path == f)
            {
                selected_rows.push(row);
            }
        }

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

            selected_rows = query_raw_folders(&conn, Some(bid))?;
            if only_unassigned {
                selected_rows.retain(|r| r.unassigned_count > 0);
            }
        }

        let mut locations: Vec<FolderLocationSummary> = loc_map.into_values().collect();
        locations.sort_by_key(|l| natural_sort_key(&l.parent_path));
        selected_rows.sort_by_key(|r| natural_sort_key(&r.folder_path));

        if batch_norm.is_none()
            && !only_unassigned
            && filter_norm.is_none()
            && selected_rows.len() > 200
        {
            selected_rows.truncate(200);
        }

        // `+status = 'ready'` forces SQLite to use `idx_photos_folder` instead of `idx_photos_status`.
        let mut sample_stmt = conn.prepare(
            "SELECT id FROM photos
             WHERE folder_path = ?1 AND +status = 'ready'
             ORDER BY filename ASC LIMIT 4",
        )?;
        let mut batch_sample_stmt = conn.prepare(
            "SELECT id FROM photos
             WHERE folder_path = ?1 AND ingest_batch_id = ?2 AND +status = 'ready'
             ORDER BY filename ASC LIMIT 4",
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
            let has_any = row.min_subject_id.is_some();
            let is_mixed =
                row.min_subject_id != row.max_subject_id || (has_any && row.unassigned_count > 0);
            let (assigned_subject_id, assigned_subject_name, assigned_subject_number) =
                if has_any && !is_mixed {
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

        let effective_context_parent: Option<String> = trim_non_empty(context_parent)
            .or(filter_norm)
            .map(str::to_string)
            .or_else(|| folders.first().map(|f| f.parent_path.clone()).filter(|s| !s.is_empty()))
            .or(batch_dest_path);

        let mut detected_session_group: Option<String> = None;
        let mut detected_group_prefix: Option<String> = None;
        let mut same_group_subject_ids: Vec<String> = Vec::new();
        let mut subject_sample_photos: BTreeMap<String, String> = BTreeMap::new();
        let mut section_label: Option<String> = None;
        let mut section_matched_subjects: BTreeMap<String, String> = BTreeMap::new();

        if let Some(ref ctx_parent) = effective_context_parent {
            let all_groups = self.list_subject_groups()?;
            let segments: Vec<&str> = ctx_parent.split('/').collect();
            detected_session_group = all_groups
                .iter()
                .find(|g| segments.iter().any(|seg| seg.eq_ignore_ascii_case(g)))
                .cloned();

            if let Some(sec) = trim_non_empty(segments.last().copied()) {
                section_label = Some(sec.to_string());
                for (other_parent, sid) in &assigned_folder_index {
                    let other_segs: Vec<&str> = other_parent.split('/').collect();
                    if !other_segs.last().is_some_and(|s| s.eq_ignore_ascii_case(sec)) {
                        continue;
                    }
                    if let Some(ref sess) = detected_session_group {
                        if !other_segs.iter().any(|seg| seg.eq_ignore_ascii_case(sess)) {
                            continue;
                        }
                    }
                    let short_loc = other_segs
                        .len()
                        .checked_sub(2)
                        .map(|i| other_segs[i].to_string())
                        .unwrap_or_else(|| other_parent.clone());
                    section_matched_subjects.entry(sid.clone()).or_insert(short_loc);
                }
            }

            if let Some((ancestor, _)) = ctx_parent.rsplit_once('/') {
                if !ancestor.is_empty() {
                    detected_group_prefix = Some(ancestor.to_string());
                    let (range_start, range_end) = folder_prefix_range(ancestor);
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

            let mut candidate_ids: HashSet<String> =
                same_group_subject_ids.iter().cloned().collect();
            if let Some(ref sess) = detected_session_group {
                let mut sess_stmt = conn.prepare(
                    "SELECT id FROM subjects WHERE is_active = 1 AND group_label = ?1 LIMIT 250",
                )?;
                for sid in sess_stmt.query_map([sess], |r| r.get::<_, String>(0))?.flatten() {
                    candidate_ids.insert(sid);
                }
            }

            let (ctx_start, ctx_end) = folder_prefix_range(ctx_parent);
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

            for sid in candidate_ids {
                let pic = other_photo_stmt
                    .query_row(rusqlite::params![&sid, &ctx_start, &ctx_end], |r| {
                        r.get::<_, String>(0)
                    })
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

    /// Atomically applies one or more folder reassignments/renames/swaps/merges using
    /// two-phase staging on disk (`.pf_swap_*`) and two-pass SQLite updates so cyclic
    /// rotation shifts never collide or violate `UNIQUE(relative_path)`.
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
            let (parent, old_leaf) = split_parent_and_leaf(&old_fp);
            let parent_path = parent.to_string();
            let old_leaf = old_leaf.to_string();

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
                        let will_rename =
                            rename_on_disk && target_fp != old_fp && self.root.join(&old_fp).is_dir();
                        (
                            Some(display_name),
                            if will_rename { sanitized } else { old_leaf },
                            if will_rename { target_fp } else { old_fp.clone() },
                            will_rename,
                        )
                    }
                    None => (None, old_leaf, old_fp.clone(), false),
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

        let moving_away: HashSet<&str> = plans
            .iter()
            .filter(|p| p.will_rename_disk)
            .map(|p| p.old_folder_path.as_str())
            .collect();
        let mut target_counts: HashMap<String, usize> = HashMap::new();
        for p in &plans {
            if p.will_rename_disk {
                *target_counts.entry(p.target_folder_path.clone()).or_insert(0) += 1;
            }
        }
        let moving_away_owned: HashSet<String> =
            moving_away.into_iter().map(str::to_string).collect();
        for p in &mut plans {
            p.action = if p.target_subject_id.is_none() {
                "unassigned"
            } else if !p.will_rename_disk {
                "reassigned"
            } else if target_counts.get(&p.target_folder_path).copied().unwrap_or(0) > 1 {
                "merged"
            } else if moving_away_owned.contains(&p.target_folder_path) {
                "swapped"
            } else if self.root.join(&p.target_folder_path).exists() {
                "merged"
            } else {
                "renamed"
            }
            .to_string();
        }

        struct PhotoSnap {
            id: String,
            filename: String,
            tags_json: String,
        }
        let mut photos_by_plan: Vec<Vec<PhotoSnap>> = Vec::with_capacity(plans.len());
        {
            let mut stmt = conn
                .prepare("SELECT id, filename, tags_json FROM photos WHERE folder_path = ?1")?;
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

        // Phase 1 on disk: stage every renaming folder to a unique hidden sibling
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

        // Phase 2 on disk: move staged folders into final targets (or safely merge files)
        for (idx, staging_abs, _) in staged {
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

        // Phase 3 in SQLite: two-pass atomic transaction so UNIQUE(relative_path) never conflicts
        let folder_levels = load_folder_levels(&conn)?;
        let now = now_iso();
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
                        merged_tags.extend(parsed.tags);
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

    /// Removes a photo folder from the DB (and cached derivatives), and optionally moves
    /// originals to `_photofinish/trash/` or permanently deletes them from disk.
    pub fn remove_folder(
        &self,
        folder_path: &str,
        mode: RemoveFolderMode,
    ) -> Result<RemoveFolderResult> {
        let rel_folder = validate_event_rel_folder(folder_path)?;
        let mut conn = self.write_conn.lock().unwrap();

        let photos: Vec<(String, Option<String>, Option<String>)> = {
            let mut stmt = conn.prepare(
                "SELECT relative_path, thumbnail_rel_path, preview_rel_path
                 FROM photos WHERE folder_path = ?1",
            )?;
            let rows = stmt
                .query_map([&rel_folder], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
                .collect::<std::result::Result<_, _>>()?;
            rows
        };

        let tx = conn.transaction()?;
        let photos_removed = tx.execute("DELETE FROM photos WHERE folder_path = ?1", [&rel_folder])?;
        tx.commit()?;

        for (_, thumb, preview) in &photos {
            if let Some(t) = thumb {
                let _ = fs::remove_file(self.root.join(t));
            }
            if let Some(pr) = preview {
                let _ = fs::remove_file(self.root.join(pr));
            }
        }

        let old_abs = self.root.join(&rel_folder);
        let mut trashed_to: Option<String> = None;

        match mode {
            RemoveFolderMode::DbOnly => {}
            RemoveFolderMode::Trash if old_abs.exists() => {
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
                    for (rel, _, _) in &photos {
                        let src = self.root.join(rel);
                        if src.is_file() {
                            if let Some(fname) = src.file_name() {
                                let _ = fs::rename(&src, trash_abs.join(fname));
                            }
                        }
                    }
                    let _ = fs::remove_dir(&old_abs);
                }
                trashed_to = Some(trash_rel);
            }
            RemoveFolderMode::Trash => {}
            RemoveFolderMode::DeleteDisk => {
                if old_abs.is_dir() && !dir_has_subdirectories(&old_abs) {
                    fs::remove_dir_all(&old_abs).map_err(|e| {
                        Error::Io(format!("Failed to delete {}: {}", old_abs.display(), e))
                    })?;
                } else {
                    for (rel, _, _) in &photos {
                        let src = self.root.join(rel);
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
    fs::read_dir(dir)
        .ok()
        .is_some_and(|entries| {
            entries
                .flatten()
                .any(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        })
}
