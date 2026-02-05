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
  └── orders
      └── order_items
  └── products (catalog)
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

### orders

```sql
CREATE TABLE orders (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id UUID REFERENCES events(id) ON DELETE CASCADE,
    
    order_number VARCHAR(50) UNIQUE NOT NULL,
    
    -- Customer
    customer_name VARCHAR(255) NOT NULL,
    customer_email VARCHAR(255),
    customer_phone VARCHAR(50),
    
    -- Financial
    subtotal_cents INTEGER NOT NULL,
    tax_rate_basis_points INTEGER NOT NULL,
    tax_cents INTEGER NOT NULL,
    total_cents INTEGER NOT NULL,
    
    -- Payment
    payment_status VARCHAR(50) NOT NULL DEFAULT 'pending',
    payment_method VARCHAR(50),
    payment_reference VARCHAR(255),
    paid_at TIMESTAMP,
    
    -- Accounting
    needs_accounting_review BOOLEAN DEFAULT false,
    accounting_notes TEXT,
    notes TEXT,
    internal_notes TEXT,
    
    inserted_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
```

### order_items

```sql
CREATE TABLE order_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    order_id UUID REFERENCES orders(id) ON DELETE CASCADE,
    
    product_type VARCHAR(50) NOT NULL,
    product_size VARCHAR(50),
    product_name VARCHAR(255),
    
    photo_id UUID REFERENCES photos(id) ON DELETE SET NULL,
    competitor_ids JSONB,
    
    unit_price_cents INTEGER NOT NULL,
    quantity INTEGER DEFAULT 1,
    line_total_cents INTEGER NOT NULL,
    
    fulfillment_status VARCHAR(50) DEFAULT 'pending',
    fulfilled_at TIMESTAMP,
    
    notes TEXT,
    inserted_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
```

### products

```sql
CREATE TABLE products (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id UUID REFERENCES events(id) ON DELETE CASCADE,
    
    product_type VARCHAR(50) NOT NULL,
    product_name VARCHAR(255) NOT NULL,
    product_description TEXT,
    product_size VARCHAR(50),
    
    price_cents INTEGER NOT NULL,
    
    is_active BOOLEAN DEFAULT true,
    display_order INTEGER DEFAULT 0,
    
    sku VARCHAR(100),
    metadata JSONB,
    
    inserted_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    UNIQUE(event_id, product_type, product_size)
);
```