use rusqlite::OptionalExtension;
use std::collections::{BTreeMap, HashSet};
use std::fs;

use super::{extract_section_label, load_folder_levels, section_level_key, EventBundle};
use crate::hardware::UsbCopyItem;
use crate::orders::{self, UsbOrderPlan, UsbSubjectBreakdown};
use crate::util::sanitize_folder_component;
use crate::{Error, Result};

impl EventBundle {
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
        let level_key = section_level_key(&folder_levels);

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

        let single_subject_set: HashSet<&str> = order
            .items
            .iter()
            .filter(|i| i.product_type == "single_digital")
            .filter_map(|i| i.subject_id.as_deref())
            .collect();
        for sid in usb_fallback_subject_ids {
            if !full_subject_ids.contains(&sid) && !single_subject_set.contains(sid.as_str()) {
                full_subject_ids.push(sid);
            }
        }

        struct UsbPlanAcc<'a> {
            root: &'a std::path::Path,
            level_key: Option<String>,
            seen_photo_ids: HashSet<String>,
            copy_items: Vec<UsbCopyItem>,
            subject_groups: BTreeMap<String, (usize, u64, BTreeMap<String, usize>)>,
            missing_files: usize,
        }

        impl UsbPlanAcc<'_> {
            #[allow(clippy::too_many_arguments)]
            fn push_photo(
                &mut self,
                pid: String,
                rel_path: String,
                folder_path: String,
                filename: String,
                tags_json: String,
                db_size: i64,
                subject_name: String,
            ) {
                if !self.seen_photo_ids.insert(pid) {
                    return;
                }
                let src_abs = self.root.join(&rel_path);
                let size_bytes = match fs::metadata(&src_abs) {
                    Ok(m) if m.is_file() => m.len(),
                    _ => {
                        self.missing_files += 1;
                        return;
                    }
                };
                let effective_size = if size_bytes > 0 {
                    size_bytes
                } else {
                    db_size.max(0) as u64
                };
                let tags: BTreeMap<String, String> =
                    serde_json::from_str(&tags_json).unwrap_or_default();
                let section =
                    extract_section_label(&folder_path, &tags, self.level_key.as_deref());
                let dest_rel = format!(
                    "{}/{}/{}",
                    sanitize_folder_component(&subject_name),
                    sanitize_folder_component(&section),
                    sanitize_folder_component(&filename)
                );
                self.copy_items.push(UsbCopyItem {
                    source_abs_path: src_abs,
                    dest_rel_path: dest_rel,
                    file_size_bytes: effective_size,
                });
                let entry = self
                    .subject_groups
                    .entry(subject_name)
                    .or_insert_with(|| (0, 0, BTreeMap::new()));
                entry.0 += 1;
                entry.1 += effective_size;
                *entry.2.entry(section).or_insert(0) += 1;
            }
        }

        let mut acc = UsbPlanAcc {
            root: &self.root,
            level_key,
            seen_photo_ids: HashSet::new(),
            copy_items: Vec::new(),
            subject_groups: BTreeMap::new(),
            missing_files: 0,
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
                let (pid, rel, fp, fname, tags, sz, subj) = row?;
                acc.push_photo(pid, rel, fp, fname, tags, sz, subj);
            }
        }

        let mut single_stmt = conn.prepare(
            "SELECT p.relative_path, p.folder_path, p.filename, p.tags_json,
                    COALESCE(p.file_size_bytes, 0), s.display_name
             FROM photos p
             LEFT JOIN subjects s ON s.id = p.subject_id
             WHERE p.id = ?1",
        )?;

        for (pid, fallback_subject) in single_photo_ids {
            if acc.seen_photo_ids.contains(&pid) {
                continue;
            }
            let row = single_stmt
                .query_row([&pid], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, i64>(4)?,
                        r.get::<_, Option<String>>(5)?,
                    ))
                })
                .optional()?;
            let Some((rel, fp, fname, tags, sz, db_subject)) = row else {
                acc.missing_files += 1;
                continue;
            };
            let subject_name = db_subject
                .or(fallback_subject)
                .unwrap_or_else(|| "Selected Photos".to_string());
            acc.push_photo(pid, rel, fp, fname, tags, sz, subject_name);
        }

        let total_photos = acc.copy_items.len();
        let total_bytes: u64 = acc.copy_items.iter().map(|i| i.file_size_bytes).sum();
        let subjects: Vec<UsbSubjectBreakdown> = acc
            .subject_groups
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
                missing_files: acc.missing_files,
            },
            acc.copy_items,
        ))
    }
}
