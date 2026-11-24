use rusqlite::OptionalExtension;

use super::{load_subject_number_map, EventBundle, SubjectRecord};
use crate::id;
use crate::preset::extract_subject_number;
use crate::roster::{natural_sort_key, ImportRosterResult, RosterEntryInput};
use crate::util::{now_iso, trim_non_empty};
use crate::Result;

fn map_subject_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<SubjectRecord> {
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
}

fn subject_numeric_bib(s: &SubjectRecord) -> u64 {
    s.number
        .as_deref()
        .and_then(|n| n.parse::<u64>().ok())
        .unwrap_or(u64::MAX)
}

impl EventBundle {
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
        let now = now_iso();
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

    /// Bulk-imports subjects (upserting by `number` when present) and auto-matches
    /// any unassigned folders whose leading number matches the roster.
    pub fn import_subjects(&self, entries: &[RosterEntryInput]) -> Result<ImportRosterResult> {
        let mut conn = self.write_conn.lock().unwrap();
        let now = now_iso();
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
                let num = trim_non_empty(entry.number.as_deref());
                let last = trim_non_empty(entry.last_name.as_deref());
                let group = trim_non_empty(entry.group_label.as_deref());
                let team = trim_non_empty(entry.team_name.as_deref());
                let ext = trim_non_empty(entry.external_id.as_deref());
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

    pub fn auto_match_unassigned_photos(&self) -> Result<usize> {
        let mut conn = self.write_conn.lock().unwrap();
        let tx = conn.transaction()?;
        let count = auto_match_unassigned_in_tx(&tx, &now_iso())?;
        tx.commit()?;
        Ok(count)
    }

    pub fn list_subject_groups(&self) -> Result<Vec<String>> {
        let conn = self.read_conn()?;
        let mut stmt = conn.prepare(
            "SELECT DISTINCT group_label FROM subjects
             WHERE is_active = 1 AND group_label IS NOT NULL AND TRIM(group_label) != ''",
        )?;
        let mut groups: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .collect();
        groups.sort_by_key(|g| natural_sort_key(g));
        Ok(groups)
    }

    pub fn list_subjects(
        &self,
        query: Option<&str>,
        group_label: Option<&str>,
        limit: i64,
    ) -> Result<Vec<SubjectRecord>> {
        self.query_subjects_inner(query.unwrap_or(""), group_label, limit, false)
    }

    pub fn search_subjects(&self, query: &str, limit: i64) -> Result<Vec<SubjectRecord>> {
        self.query_subjects_inner(query, None, limit, true)
    }

    fn query_subjects_inner(
        &self,
        query: &str,
        group_label: Option<&str>,
        limit: i64,
        rank_by_relevance: bool,
    ) -> Result<Vec<SubjectRecord>> {
        let conn = self.read_conn()?;
        let trimmed = query.trim();
        let lower_q = trimmed.to_lowercase();
        let pattern = format!("%{}%", trimmed);
        let group = trim_non_empty(group_label);

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
                    OR COALESCE(s.team_name, '') LIKE ?3 COLLATE NOCASE
                    OR COALESCE(s.group_label, '') LIKE ?3 COLLATE NOCASE)
             GROUP BY s.id",
        )?;
        let mut out: Vec<SubjectRecord> = stmt
            .query_map(rusqlite::params![group, trimmed, pattern], map_subject_row)?
            .collect::<std::result::Result<_, _>>()?;

        out.sort_by_key(|s| {
            let rank: u8 = if !rank_by_relevance || lower_q.is_empty() {
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
            (rank, subject_numeric_bib(s), natural_sort_key(&s.display_name))
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
                map_subject_row,
            )
            .optional()?;
        Ok(row)
    }
}

fn build_display_name(number: Option<&str>, first_name: &str, last_name: Option<&str>) -> String {
    let name_part = match trim_non_empty(last_name) {
        Some(last) => format!("{} {}", first_name.trim(), last),
        None => first_name.trim().to_string(),
    };
    match trim_non_empty(number) {
        Some(num) => format!("{} {}", num, name_part),
        None => name_part,
    }
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
