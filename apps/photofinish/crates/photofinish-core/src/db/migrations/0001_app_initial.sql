-- PhotoFinish Machine-Local App Database Schema v1
-- Stored in OS AppData (not inside event bundles).
-- Keeps machine-specific USB reader mappings, event library bookmarks, and settings.

CREATE TABLE IF NOT EXISTS _schema_migrations (
    version INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    applied_at TEXT NOT NULL,
    app_version TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS library_events (
    root_path TEXT PRIMARY KEY,
    event_id TEXT NOT NULL,
    name TEXT NOT NULL,
    last_opened_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS reader_configs (
    reader_id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    destination TEXT NOT NULL DEFAULT '',
    photographer TEXT NOT NULL DEFAULT '',
    camera_brand TEXT NOT NULL DEFAULT '',
    camera_folder_path TEXT NOT NULL DEFAULT 'DCIM',
    rename_prefix TEXT NOT NULL DEFAULT '',
    auto_rename INTEGER NOT NULL DEFAULT 0,
    file_rename_prefix TEXT,
    require_subfolder INTEGER NOT NULL DEFAULT 0,
    current_order INTEGER NOT NULL DEFAULT 1,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS app_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
