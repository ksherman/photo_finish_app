use chrono::Utc;
use rusqlite::{backup::Backup, Connection, OpenFlags, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use crate::{Error, Result};

/// Visible, self-identifying metadata folder inside every portable Event Bundle.
pub const BUNDLE_DIR_NAME: &str = "_photofinish";
/// SQLite database filename inside `_photofinish/`.
pub const EVENT_DB_FILENAME: &str = "photofinish.db";
pub const BACKUPS_DIR_NAME: &str = "backups";
pub const THUMBNAILS_DIR_NAME: &str = "thumbnails";
pub const PREVIEWS_DIR_NAME: &str = "previews";

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Copy)]
pub struct Migration {
    pub version: i64,
    pub name: &'static str,
    pub sql: &'static str,
}

pub const EVENT_MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "0001_event_initial",
    sql: include_str!("migrations/0001_event_initial.sql"),
}];

pub const APP_MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "0001_app_initial",
    sql: include_str!("migrations/0001_app_initial.sql"),
}];

pub fn bundle_dir(event_root: &Path) -> PathBuf {
    event_root.join(BUNDLE_DIR_NAME)
}

pub fn event_db_path(event_root: &Path) -> PathBuf {
    bundle_dir(event_root).join(EVENT_DB_FILENAME)
}

pub fn backups_dir(event_root: &Path) -> PathBuf {
    bundle_dir(event_root).join(BACKUPS_DIR_NAME)
}

pub fn thumbnails_dir(event_root: &Path) -> PathBuf {
    bundle_dir(event_root).join(THUMBNAILS_DIR_NAME)
}

pub fn previews_dir(event_root: &Path) -> PathBuf {
    bundle_dir(event_root).join(PREVIEWS_DIR_NAME)
}

pub fn is_event_bundle(path: &Path) -> bool {
    event_db_path(path).is_file()
}

/// Configures standard SQLite pragmas for local concurrency and durability.
pub fn configure_connection(conn: &Connection, read_only: bool) -> Result<()> {
    conn.busy_timeout(Duration::from_millis(5000))?;
    if !read_only {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
    }
    conn.pragma_update(None, "foreign_keys", "ON")?;
    Ok(())
}

pub fn open_rw_connection(db_path: &Path) -> Result<Connection> {
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent).map_err(|e| Error::Io(format!("{}: {}", parent.display(), e)))?;
    }
    let conn = Connection::open(db_path)?;
    configure_connection(&conn, false)?;
    Ok(conn)
}

pub fn open_ro_connection(db_path: &Path) -> Result<Connection> {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let conn = Connection::open_with_flags(db_path, flags)?;
    configure_connection(&conn, true)?;
    Ok(conn)
}

/// Creates a consistent online SQLite snapshot (safe while WAL mode is active).
pub fn backup_sqlite_db(src_conn: &Connection, dst_path: &Path) -> Result<()> {
    if let Some(parent) = dst_path.parent() {
        fs::create_dir_all(parent).map_err(|e| Error::Io(format!("{}: {}", parent.display(), e)))?;
    }
    let mut dst_conn = Connection::open(dst_path)?;
    let backup = Backup::new(src_conn, &mut dst_conn)?;
    backup.run_to_completion(100, Duration::from_millis(5), None)?;
    Ok(())
}

/// Runs embedded forward-only migrations inside immediate transactions.
///
/// Safety guarantees for portable event bundles:
/// 1. Refuses to open a DB created by a newer PhotoFinish schema version (`SchemaTooNew`).
/// 2. When upgrading an existing DB (`current_version > 0 && current_version < latest`),
///    automatically writes an online SQLite backup into `backup_dir` BEFORE applying DDL.
/// 3. Each migration runs atomically and updates both `_schema_migrations` and `PRAGMA user_version`.
pub fn run_migrations(
    conn: &mut Connection,
    migrations: &[Migration],
    backup_dir: Option<&Path>,
    db_stem: &str,
) -> Result<i64> {
    let latest_version = migrations.last().map(|m| m.version).unwrap_or(0);
    let current_version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;

    if current_version > latest_version {
        return Err(Error::SchemaTooNew {
            db_version: current_version,
            app_version: latest_version,
        });
    }

    if current_version == latest_version {
        return Ok(current_version);
    }

    // Automatically snapshot existing databases before migrating
    if current_version > 0 {
        if let Some(dir) = backup_dir {
            let stamp = Utc::now().format("%Y-%m-%dT%H-%M-%SZ");
            let backup_file = dir.join(format!("{}_v{}_{}.db", db_stem, current_version, stamp));
            backup_sqlite_db(conn, &backup_file)?;
        }
    }

    for migration in migrations.iter().filter(|m| m.version > current_version) {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute_batch(migration.sql)?;
        let now = Utc::now().to_rfc3339();
        tx.execute(
            "INSERT OR REPLACE INTO _schema_migrations (version, name, applied_at, app_version) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![migration.version, migration.name, now, APP_VERSION],
        )?;
        tx.pragma_update(None, "user_version", migration.version)?;
        tx.commit()?;
    }

    Ok(latest_version)
}

// ============================================================================
// Machine-Local AppDb (reader configs, event library bookmarks, station settings)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryEventEntry {
    pub root_path: String,
    pub event_id: String,
    pub name: String,
    pub last_opened_at: String,
    pub exists_on_disk: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReaderConfig {
    pub reader_id: String,
    pub display_name: String,
    pub destination: String,
    pub photographer: String,
    pub camera_brand: String,
    pub camera_folder_path: String,
    pub rename_prefix: String,
    pub auto_rename: bool,
    pub file_rename_prefix: Option<String>,
    pub require_subfolder: bool,
    pub current_order: u32,
}

pub struct AppDb {
    conn: Mutex<Connection>,
}

impl AppDb {
    pub fn open(db_path: &Path) -> Result<Self> {
        let mut conn = open_rw_connection(db_path)?;
        let backup_dir = db_path.parent().map(|p| p.join("backups"));
        run_migrations(&mut conn, APP_MIGRATIONS, backup_dir.as_deref(), "app")?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn upsert_library_event(&self, root_path: &Path, event_id: &str, name: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO library_events (root_path, event_id, name, last_opened_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(root_path) DO UPDATE SET
               event_id = excluded.event_id,
               name = excluded.name,
               last_opened_at = excluded.last_opened_at",
            rusqlite::params![root_path.to_string_lossy().as_ref(), event_id, name, now],
        )?;
        Ok(())
    }

    pub fn list_library_events(&self) -> Result<Vec<LibraryEventEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT root_path, event_id, name, last_opened_at
             FROM library_events
             ORDER BY last_opened_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let root_path: String = row.get(0)?;
            let exists_on_disk = is_event_bundle(Path::new(&root_path));
            Ok(LibraryEventEntry {
                root_path,
                event_id: row.get(1)?,
                name: row.get(2)?,
                last_opened_at: row.get(3)?,
                exists_on_disk,
            })
        })?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }

    pub fn remove_library_event(&self, root_path: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM library_events WHERE root_path = ?1", [root_path])?;
        Ok(())
    }

    pub fn list_reader_configs(&self) -> Result<Vec<ReaderConfig>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT reader_id, display_name, destination, photographer, camera_brand,
                    camera_folder_path, rename_prefix, auto_rename, file_rename_prefix,
                    require_subfolder, current_order
             FROM reader_configs
             ORDER BY display_name ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ReaderConfig {
                reader_id: row.get(0)?,
                display_name: row.get(1)?,
                destination: row.get(2)?,
                photographer: row.get(3)?,
                camera_brand: row.get(4)?,
                camera_folder_path: row.get(5)?,
                rename_prefix: row.get(6)?,
                auto_rename: row.get::<_, i64>(7)? != 0,
                file_rename_prefix: row.get(8)?,
                require_subfolder: row.get::<_, i64>(9)? != 0,
                current_order: row.get::<_, u32>(10)?,
            })
        })?;
        let mut configs = Vec::new();
        for row in rows {
            configs.push(row?);
        }
        Ok(configs)
    }

    pub fn save_reader_config(&self, cfg: &ReaderConfig) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO reader_configs (
                reader_id, display_name, destination, photographer, camera_brand,
                camera_folder_path, rename_prefix, auto_rename, file_rename_prefix,
                require_subfolder, current_order, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(reader_id) DO UPDATE SET
                display_name = excluded.display_name,
                destination = excluded.destination,
                photographer = excluded.photographer,
                camera_brand = excluded.camera_brand,
                camera_folder_path = excluded.camera_folder_path,
                rename_prefix = excluded.rename_prefix,
                auto_rename = excluded.auto_rename,
                file_rename_prefix = excluded.file_rename_prefix,
                require_subfolder = excluded.require_subfolder,
                current_order = excluded.current_order,
                updated_at = excluded.updated_at",
            rusqlite::params![
                cfg.reader_id,
                cfg.display_name,
                cfg.destination,
                cfg.photographer,
                cfg.camera_brand,
                cfg.camera_folder_path,
                cfg.rename_prefix,
                if cfg.auto_rename { 1 } else { 0 },
                cfg.file_rename_prefix,
                if cfg.require_subfolder { 1 } else { 0 },
                cfg.current_order,
                now,
            ],
        )?;
        Ok(())
    }

    pub fn increment_reader_order(&self, reader_id: &str) -> Result<u32> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE reader_configs SET current_order = current_order + 1, updated_at = ?2 WHERE reader_id = ?1",
            rusqlite::params![reader_id, now],
        )?;
        let order: Option<u32> = conn
            .query_row(
                "SELECT current_order FROM reader_configs WHERE reader_id = ?1",
                [reader_id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(order.unwrap_or(1))
    }

    pub fn clear_reader_configs(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM reader_configs", [])?;
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let val = conn
            .query_row("SELECT value FROM app_settings WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()?;
        Ok(val)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            rusqlite::params![key, value],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn migration_runner_backups_on_upgrade_and_guards_newer_schema() {
        let dir = tempdir().unwrap();
        let db_path = event_db_path(dir.path());
        let backup_path = backups_dir(dir.path());

        let mut conn = open_rw_connection(&db_path).unwrap();
        let v = run_migrations(&mut conn, EVENT_MIGRATIONS, Some(&backup_path), "photofinish")
            .unwrap();
        assert_eq!(v, 1);
        // Fresh init should not create a backup
        assert!(!backup_path.exists());

        // Simulate upgrading from v1 -> v2
        let v2_migrations = [
            EVENT_MIGRATIONS[0],
            Migration {
                version: 2,
                name: "0002_test_add_col",
                sql: "ALTER TABLE event_config ADD COLUMN custom_note TEXT;",
            },
        ];
        let v2 = run_migrations(&mut conn, &v2_migrations, Some(&backup_path), "photofinish")
            .unwrap();
        assert_eq!(v2, 2);

        // Pre-migration backup must exist and be readable at schema v1
        let backups: Vec<_> = fs::read_dir(&backup_path)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(backups.len(), 1);
        let backup_conn = open_ro_connection(&backups[0].path()).unwrap();
        let backup_ver: i64 = backup_conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(backup_ver, 1);

        // Opening the v2 DB with an older app (only knowing v1) must fail safely
        let err = run_migrations(&mut conn, EVENT_MIGRATIONS, Some(&backup_path), "photofinish")
            .unwrap_err();
        assert!(matches!(
            err,
            Error::SchemaTooNew {
                db_version: 2,
                app_version: 1
            }
        ));
    }
}
