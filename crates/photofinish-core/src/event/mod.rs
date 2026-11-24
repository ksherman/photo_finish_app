mod folders;
mod photos;
mod subjects;
mod usb;

#[cfg(test)]
mod tests;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::db::{
    backups_dir, bundle_dir, event_db_path, is_event_bundle, open_ro_connection,
    open_rw_connection, previews_dir, run_migrations, thumbnails_dir, EVENT_MIGRATIONS,
};
use crate::id;
use crate::orders::{
    self, CreateOrderRequest, EventOrderSettingsInput, OrderListResponse, OrderRecord,
    ProductRecord, ProductUpsertInput, UpdatePaymentRequest,
};
use crate::preset::{find_preset, scaffold_folders, ScaffoldConfig};
use crate::util::now_iso;
use crate::{Error, Result};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
/// Writes serialize through `write_conn`; concurrent LAN reads open short-lived WAL connections.
pub struct EventBundle {
    pub(crate) root: PathBuf,
    pub(crate) db_path: PathBuf,
    pub(crate) write_conn: Mutex<Connection>,
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

        for dir in [
            params.root_path.clone(),
            bundle_dir(&params.root_path),
            thumbnails_dir(&params.root_path),
            previews_dir(&params.root_path),
        ] {
            fs::create_dir_all(&dir)
                .map_err(|e| Error::Io(format!("{}: {}", dir.display(), e)))?;
        }

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
        let now = now_iso();

        conn.execute(
            "INSERT INTO event_config (
                id, name, slug, description, status, preset_id, subject_label,
                folder_levels_json, preset_config_json, order_code, station_code,
                next_order_number, tax_rate_basis_points, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, 'active', ?5, ?6, ?7, ?8, ?9, ?10, 0, ?11, ?12, ?13)",
            rusqlite::params![
                id::event_id(),
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

        for dir in [thumbnails_dir(&root), previews_dir(&root)] {
            fs::create_dir_all(&dir)
                .map_err(|e| Error::Io(format!("{}: {}", dir.display(), e)))?;
        }

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

    /// Discovers all PhotoFinish Event Bundles inside a parent folder (up to 6 levels deep,
    /// stopping descent immediately whenever a bundle is found).
    pub fn discover_in_directory(parent: impl AsRef<Path>) -> Result<Vec<EventSummary>> {
        let parent = parent.as_ref();
        if !parent.is_dir() {
            return Err(Error::InvalidInput(format!("Not a directory: {}", parent.display())));
        }

        const MAX_DISCOVER_DEPTH: usize = 6;
        let mut bundle_roots = Vec::new();
        let mut stack: Vec<(PathBuf, usize)> = vec![(parent.to_path_buf(), 0)];

        while let Some((dir, depth)) = stack.pop() {
            if is_event_bundle(&dir) {
                bundle_roots.push(dir);
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
                if path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|name| name.starts_with('.') || name.starts_with('_'))
                {
                    continue;
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
        let mut summary = conn.query_row(
            "SELECT id, name, slug, description, status, preset_id, subject_label,
                    folder_levels_json, order_code, station_code, tax_rate_basis_points, created_at
             FROM event_config LIMIT 1",
            [],
            |row| {
                let folder_levels_json: String = row.get(7)?;
                Ok((
                    EventSummary {
                        id: row.get(0)?,
                        root_path: self.root.to_string_lossy().to_string(),
                        name: row.get(1)?,
                        slug: row.get(2)?,
                        description: row.get(3)?,
                        status: row.get(4)?,
                        preset_id: row.get(5)?,
                        subject_label: row.get(6)?,
                        folder_levels: Vec::new(),
                        order_code: row.get(8)?,
                        station_code: row.get(9)?,
                        tax_rate_basis_points: row.get(10)?,
                        photo_counts: PhotoCounts::default(),
                        subject_count: 0,
                        order_count: 0,
                        created_at: row.get(11)?,
                    },
                    folder_levels_json,
                ))
            },
        )?;
        summary.0.folder_levels = serde_json::from_str(&summary.1)?;
        summary.0.photo_counts = query_photo_counts(&conn)?;
        summary.0.subject_count =
            conn.query_row("SELECT COUNT(*) FROM subjects WHERE is_active = 1", [], |r| {
                r.get(0)
            })?;
        summary.0.order_count = conn.query_row("SELECT COUNT(*) FROM orders", [], |r| r.get(0))?;
        Ok(summary.0)
    }

    pub fn photo_counts(&self) -> Result<PhotoCounts> {
        query_photo_counts(&self.read_conn()?)
    }

    // ---- Order & Product Catalog Delegation ----

    pub fn list_products(&self, include_unavailable: bool) -> Result<Vec<ProductRecord>> {
        orders::list_products(&self.read_conn()?, include_unavailable)
    }

    pub fn place_order(&self, req: CreateOrderRequest) -> Result<OrderRecord> {
        orders::place_order(&mut self.write_conn.lock().unwrap(), req)
    }

    pub fn get_order_by_number(&self, order_number: &str) -> Result<Option<OrderRecord>> {
        orders::get_order_by_number(&self.read_conn()?, order_number)
    }

    pub fn list_orders(&self) -> Result<OrderListResponse> {
        orders::list_orders(&self.read_conn()?)
    }

    pub fn update_order_payment(&self, req: &UpdatePaymentRequest) -> Result<OrderRecord> {
        orders::update_order_payment(&mut self.write_conn.lock().unwrap(), req)
    }

    pub fn update_order_notes(&self, order_id: &str, notes: Option<&str>) -> Result<OrderRecord> {
        orders::update_order_notes(&mut self.write_conn.lock().unwrap(), order_id, notes)
    }

    pub fn update_order_item_fulfillment(
        &self,
        order_id: &str,
        item_id: &str,
        fulfillment_status: &str,
    ) -> Result<OrderRecord> {
        orders::update_order_item_fulfillment(
            &mut self.write_conn.lock().unwrap(),
            order_id,
            item_id,
            fulfillment_status,
        )
    }

    pub fn mark_usb_items_fulfilled(&self, order_id: &str) -> Result<usize> {
        orders::mark_usb_items_fulfilled(&mut self.write_conn.lock().unwrap(), order_id)
    }

    pub fn upsert_product(&self, input: &ProductUpsertInput) -> Result<ProductRecord> {
        orders::upsert_product(&mut self.write_conn.lock().unwrap(), input)
    }

    pub fn update_event_order_settings(
        &self,
        input: &EventOrderSettingsInput,
    ) -> Result<EventSummary> {
        orders::update_event_order_settings(&mut self.write_conn.lock().unwrap(), input)?;
        self.summary()
    }
}

pub(crate) fn query_photo_counts(conn: &Connection) -> Result<PhotoCounts> {
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

pub(crate) fn load_folder_levels(conn: &Connection) -> Result<Vec<String>> {
    let json: String =
        conn.query_row("SELECT folder_levels_json FROM event_config LIMIT 1", [], |r| {
            r.get(0)
        })?;
    Ok(serde_json::from_str(&json)?)
}

/// Picks the folder level directly above "Subject" (e.g. "Apparatus" in gymnastics,
/// "Routine" in dance, "Group" in simple) for sport-agnostic section grouping.
pub(crate) fn section_level_key(folder_levels: &[String]) -> Option<String> {
    if folder_levels.len() >= 2 {
        Some(folder_levels[folder_levels.len() - 2].clone())
    } else {
        folder_levels.first().cloned()
    }
}

pub(crate) fn extract_section_label(
    folder_path: &str,
    tags: &BTreeMap<String, String>,
    level_key: Option<&str>,
) -> String {
    level_key
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
}

pub(crate) fn load_subject_number_map(conn: &Connection) -> Result<HashMap<String, String>> {
    let mut stmt = conn.prepare("SELECT id, number FROM subjects WHERE number IS NOT NULL")?;
    let map = stmt
        .query_map([], |r| Ok((r.get::<_, String>(1)?, r.get::<_, String>(0)?)))?
        .collect::<std::result::Result<HashMap<_, _>, _>>()?;
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
