-- PhotoFinish Portable Event Bundle Schema v1
-- Stored at: <event_root>/_photofinish/photofinish.db
-- All file paths are stored RELATIVE to <event_root> so event folders can be
-- moved freely between meet servers, external SSDs, and home NAS archives.

CREATE TABLE IF NOT EXISTS _schema_migrations (
    version INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    applied_at TEXT NOT NULL,
    app_version TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS event_config (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    slug TEXT NOT NULL,
    description TEXT,
    starts_at TEXT,
    ends_at TEXT,
    status TEXT NOT NULL DEFAULT 'active', -- active | archived
    preset_id TEXT NOT NULL DEFAULT 'gymnastics', -- gymnastics | dance | simple | custom
    subject_label TEXT NOT NULL DEFAULT 'Competitor',
    folder_levels_json TEXT NOT NULL DEFAULT '["Gym","Session","Group","Apparatus","Subject"]',
    preset_config_json TEXT NOT NULL DEFAULT '{}',
    order_code TEXT NOT NULL DEFAULT 'EVT',
    station_code TEXT NOT NULL DEFAULT 'A',
    next_order_number INTEGER NOT NULL DEFAULT 0,
    tax_rate_basis_points INTEGER NOT NULL DEFAULT 850,
    shootproof_event_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS subjects (
    id TEXT PRIMARY KEY,
    number TEXT,
    first_name TEXT NOT NULL,
    last_name TEXT,
    display_name TEXT NOT NULL,
    group_label TEXT, -- e.g. Session "1A", Division, Heat, Class
    team_name TEXT,
    external_id TEXT, -- e.g. USAG ID for optional cross-event matching
    metadata_json TEXT NOT NULL DEFAULT '{}',
    is_active INTEGER NOT NULL DEFAULT 1,
    shootproof_album_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_subjects_number
    ON subjects(number) WHERE number IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_subjects_display_name
    ON subjects(display_name);
CREATE INDEX IF NOT EXISTS idx_subjects_group_label
    ON subjects(group_label);

CREATE TABLE IF NOT EXISTS ingest_batches (
    id TEXT PRIMARY KEY,
    sequence_num INTEGER NOT NULL,
    reader_id TEXT,
    reader_name TEXT,
    volume_name TEXT,
    photographer TEXT,
    source_path TEXT,
    destination_rel_path TEXT NOT NULL,
    tags_json TEXT NOT NULL DEFAULT '{}',
    file_count INTEGER NOT NULL DEFAULT 0,
    started_at TEXT NOT NULL,
    completed_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_ingest_batches_sequence
    ON ingest_batches(sequence_num);

CREATE TABLE IF NOT EXISTS photos (
    id TEXT PRIMARY KEY,
    subject_id TEXT REFERENCES subjects(id) ON DELETE SET NULL,
    ingest_batch_id TEXT REFERENCES ingest_batches(id) ON DELETE SET NULL,
    relative_path TEXT NOT NULL UNIQUE,
    folder_path TEXT NOT NULL,
    leaf_folder TEXT,
    filename TEXT NOT NULL,
    original_filename TEXT,
    file_size_bytes INTEGER,
    width INTEGER,
    height INTEGER,
    tags_json TEXT NOT NULL DEFAULT '{}',
    photographer TEXT,
    captured_at TEXT,
    status TEXT NOT NULL DEFAULT 'discovered', -- discovered | processing | ready | error
    thumbnail_rel_path TEXT,
    preview_rel_path TEXT,
    error_message TEXT,
    shootproof_photo_id TEXT,
    shootproof_synced_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_photos_subject
    ON photos(subject_id);
CREATE INDEX IF NOT EXISTS idx_photos_status
    ON photos(status);
CREATE INDEX IF NOT EXISTS idx_photos_folder
    ON photos(folder_path);
CREATE INDEX IF NOT EXISTS idx_photos_batch
    ON photos(ingest_batch_id);

CREATE TABLE IF NOT EXISTS products (
    id TEXT PRIMARY KEY,
    product_type TEXT NOT NULL, -- single_digital | full_digital | usb_addon | print | custom
    name TEXT NOT NULL,
    size TEXT,
    price_cents INTEGER NOT NULL,
    is_available INTEGER NOT NULL DEFAULT 1,
    display_order INTEGER NOT NULL DEFAULT 0,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS orders (
    id TEXT PRIMARY KEY,
    order_number TEXT NOT NULL UNIQUE,
    station_code TEXT NOT NULL DEFAULT 'A',
    customer_name TEXT NOT NULL,
    customer_email TEXT,
    customer_phone TEXT,
    shipping_address_json TEXT,
    subtotal_cents INTEGER NOT NULL,
    tax_rate_basis_points INTEGER NOT NULL,
    tax_cents INTEGER NOT NULL,
    total_cents INTEGER NOT NULL,
    payment_status TEXT NOT NULL DEFAULT 'pending', -- pending | paid | refunded
    payment_method TEXT, -- square_qr | square_terminal | cash | check | other
    payment_reference TEXT,
    paid_at TEXT,
    notes TEXT,
    shootproof_order_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_orders_payment_status
    ON orders(payment_status);
CREATE INDEX IF NOT EXISTS idx_orders_created_at
    ON orders(created_at);

CREATE TABLE IF NOT EXISTS order_items (
    id TEXT PRIMARY KEY,
    order_id TEXT NOT NULL REFERENCES orders(id) ON DELETE CASCADE,
    product_id TEXT REFERENCES products(id) ON DELETE SET NULL,
    subject_id TEXT REFERENCES subjects(id) ON DELETE SET NULL,
    photo_id TEXT REFERENCES photos(id) ON DELETE SET NULL,
    product_type TEXT NOT NULL,
    product_name TEXT NOT NULL,
    product_size TEXT,
    quantity INTEGER NOT NULL DEFAULT 1,
    unit_price_cents INTEGER NOT NULL,
    line_total_cents INTEGER NOT NULL,
    fulfillment_status TEXT NOT NULL DEFAULT 'pending', -- pending | fulfilled
    fulfilled_at TEXT,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_order_items_order
    ON order_items(order_id);
CREATE INDEX IF NOT EXISTS idx_order_items_fulfillment
    ON order_items(fulfillment_status);
