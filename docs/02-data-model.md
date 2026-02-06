# PhotoFinish - Data Model

**Version:** 1.1
**Date:** November 29, 2025

---

## Schema Overview

```
events
  └── event_competitors (roster entries linked to event)
      └── photos (flat location fields: gym, session, group_name, apparatus)
  └── competitors (reusable competitor profiles)
  └── event_products (per-event pricing, links to product_templates)
  └── orders
      └── order_items (links to event_product + event_competitor)

product_templates (system-level catalog, not event-scoped)
```

---

## Core Tables

### events

Top-level container for a photo collection.

```sql
CREATE TABLE events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    slug VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    starts_at TIMESTAMP,
    ends_at TIMESTAMP,
    status VARCHAR(50) DEFAULT 'active',  -- active, archived
    order_code VARCHAR(10),  -- 3-letter code for order numbers (e.g., "STV")
    tax_rate_basis_points INTEGER DEFAULT 850, -- 8.5%
    next_order_number INTEGER DEFAULT 0,  -- counter for sequential order numbers
    inserted_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_events_status ON events(status);
CREATE INDEX idx_events_slug ON events(slug);
```

### competitors

Reusable competitor profiles (not event-specific).

```sql
CREATE TABLE competitors (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    first_name VARCHAR(100) NOT NULL,
    last_name VARCHAR(100),
    external_id VARCHAR(100),
    email VARCHAR(255),
    phone VARCHAR(50),
    metadata JSONB,
    inserted_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
```

### event_competitors

Links competitors to events with event-specific data.

```sql
CREATE TABLE event_competitors (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id UUID REFERENCES events(id) ON DELETE CASCADE,
    competitor_id UUID REFERENCES competitors(id) ON DELETE CASCADE,
    competitor_number VARCHAR(50) NOT NULL,
    display_name VARCHAR(255),
    team_name VARCHAR(255),
    session VARCHAR(50),
    level VARCHAR(50),
    age_group VARCHAR(50),
    is_active BOOLEAN DEFAULT true,
    metadata JSONB,
    inserted_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(event_id, competitor_number)
);

CREATE INDEX idx_event_competitors_event ON event_competitors(event_id);
CREATE INDEX idx_event_competitors_number ON event_competitors(event_id, competitor_number);
```

### photos

Photo files and metadata. Location is stored as flat fields parsed from the folder structure.

```sql
CREATE TABLE photos (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id UUID REFERENCES events(id) ON DELETE CASCADE,
    event_competitor_id UUID REFERENCES event_competitors(id) ON DELETE SET NULL,

    -- Location (flat, parsed from folder path)
    gym VARCHAR(100),
    session VARCHAR(50),
    group_name VARCHAR(100),
    apparatus VARCHAR(100),

    -- File paths
    ingestion_path VARCHAR(1000) NOT NULL,
    current_path VARCHAR(1000),
    preview_path VARCHAR(1000),
    thumbnail_path VARCHAR(1000),

    -- File info
    filename VARCHAR(255) NOT NULL,
    original_filename VARCHAR(255),
    file_size_bytes BIGINT,
    mime_type VARCHAR(100) DEFAULT 'image/jpeg',
    width INTEGER,
    height INTEGER,

    -- Ingestion tracking
    photographer VARCHAR(100),
    source_folder VARCHAR(255),

    -- Timestamps
    captured_at TIMESTAMP,
    discovered_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    processed_at TIMESTAMP,
    finalized_at TIMESTAMP,

    -- Status
    status VARCHAR(50) DEFAULT 'discovered',
    error_message TEXT,

    -- Metadata
    exif_data JSONB,
    metadata JSONB,

    inserted_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_photos_event ON photos(event_id);
CREATE INDEX idx_photos_event_competitor ON photos(event_competitor_id);
CREATE INDEX idx_photos_status ON photos(status);
CREATE INDEX idx_photos_location ON photos(event_id, gym, session, apparatus);
```

---

## Ordering Tables

### product_templates

System-level product catalog. Holds default pricing, not event-scoped.

| Column | Type | Notes |
|--------|------|-------|
| id | text PK | Prefixed: `ptm_abc1234` |
| product_type | atom | `:usb`, `:print`, `:collage`, `:custom_photo`, `:accessory` |
| product_name | text | Display name, e.g. "All Photos USB Drive" |
| product_size | text | Nullable. e.g. "5x7", "8x10" |
| default_price_cents | integer | Default price in cents |
| is_active | boolean | Default true. Inactive templates excluded from new events |
| display_order | integer | Controls ordering on forms/UI |
| timestamps | | |

### event_products

Per-event price overrides. Created by copying active product_templates when an event is initialized.

| Column | Type | Notes |
|--------|------|-------|
| id | text PK | Prefixed: `evp_abc1234` |
| event_id | text FK → events | |
| product_template_id | text FK → product_templates | |
| price_cents | integer | Actual price for this event |
| is_available | boolean | Default true. Can disable per event |
| timestamps | | |
| | | UNIQUE(event_id, product_template_id) |

### orders

| Column | Type | Notes |
|--------|------|-------|
| id | text PK | Prefixed: `ord_abc1234` |
| event_id | text FK → events | |
| order_number | text | Unique. Format: `{order_code}-{NNNN}` |
| customer_name | text | Required |
| customer_email | text | Nullable |
| customer_phone | text | Nullable |
| subtotal_cents | integer | Sum of line items |
| tax_rate_basis_points | integer | Snapshot from event at order time |
| tax_cents | integer | Calculated |
| total_cents | integer | subtotal + tax |
| payment_status | atom | `:pending`, `:paid`, `:refunded` |
| payment_reference | text | Nullable. For payment reconciliation |
| notes | text | Nullable. Staff notes |
| timestamps | | |

### order_items

| Column | Type | Notes |
|--------|------|-------|
| id | text PK | Prefixed: `itm_abc1234` |
| order_id | text FK → orders | |
| event_product_id | text FK → event_products | Price source |
| event_competitor_id | text FK → event_competitors | Which gymnast |
| quantity | integer | Default 1 |
| unit_price_cents | integer | Snapshot of price at order time |
| line_total_cents | integer | quantity * unit_price |
| fulfillment_status | atom | `:pending`, `:fulfilled` |
| timestamps | | |