# PhotoFinish - Data Model

**Version:** 1.1
**Date:** November 29, 2025

---

## Schema Overview

```
events
  └── hierarchy_levels (defines structure: Gym -> Session -> Group -> Apparatus -> Competitor)
  └── hierarchy_nodes (actual folders: "Gym A", "Session 11A", "1713 Julia V")
      └── photos
  └── competitors (logical roster data)
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

### hierarchy_levels

Defines hierarchy structure per event.

```sql
CREATE TABLE hierarchy_levels (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id UUID REFERENCES events(id) ON DELETE CASCADE,
    level_number INTEGER NOT NULL,  -- 1, 2, 3, 4, 5
    level_name VARCHAR(100) NOT NULL,  -- "Gym", "Session", "Apparatus"
    level_name_plural VARCHAR(100),
    is_required BOOLEAN DEFAULT true,
    allow_photos BOOLEAN DEFAULT false,  -- If true, photos can live at this level
    inserted_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(event_id, level_number)
);

CREATE INDEX idx_hierarchy_levels_event ON hierarchy_levels(event_id);
```

**Standard Structure (Gymnastics):**
| level_number | level_name | Example |
|--------------|------------|---------|
| 1 | Gym | "Gym A" |
| 2 | Session | "Session 11A" |
| 3 | Group | "Group 3A" |
| 4 | Apparatus | "Beam" |
| 5 | Competitor | "1713 Julia V" (Photos live here) |

### hierarchy_nodes

Actual organizational units (folders/categories).

```sql
CREATE TABLE hierarchy_nodes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id UUID REFERENCES events(id) ON DELETE CASCADE,
    parent_id UUID REFERENCES hierarchy_nodes(id) ON DELETE CASCADE,
    level_number INTEGER NOT NULL,
    name VARCHAR(255) NOT NULL,
    slug VARCHAR(255) NOT NULL,
    display_order INTEGER DEFAULT 0,
    metadata JSONB,
    inserted_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_hierarchy_nodes_event ON hierarchy_nodes(event_id);
CREATE INDEX idx_hierarchy_nodes_parent ON hierarchy_nodes(parent_id);
CREATE INDEX idx_hierarchy_nodes_level ON hierarchy_nodes(event_id, level_number);
CREATE INDEX idx_hierarchy_nodes_slug ON hierarchy_nodes(event_id, slug);
CREATE INDEX idx_hierarchy_nodes_path ON hierarchy_nodes(event_id, parent_id, level_number);
```

### competitors

Roster data (Logical People).

```sql
CREATE TABLE competitors (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id UUID REFERENCES events(id) ON DELETE CASCADE,
    
    -- Link to the physical folder node if one exists for this person
    node_id UUID REFERENCES hierarchy_nodes(id) ON DELETE SET NULL,
    
    competitor_number VARCHAR(50) NOT NULL,  -- Meet ID: "1022"
    first_name VARCHAR(100) NOT NULL,
    last_name VARCHAR(100),
    display_name VARCHAR(255),  -- Computed or override: "1022 Kevin S"
    
    team_name VARCHAR(255),
    level VARCHAR(50),  -- Gymnastics level
    age_group VARCHAR(50),
    
    email VARCHAR(255),
    phone VARCHAR(50),
    
    is_active BOOLEAN DEFAULT true,
    metadata JSONB,
    
    inserted_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    deleted_at TIMESTAMP,
    
    UNIQUE(event_id, competitor_number)
);

CREATE INDEX idx_competitors_event ON competitors(event_id);
CREATE INDEX idx_competitors_node ON competitors(node_id);
CREATE INDEX idx_competitors_number ON competitors(event_id, competitor_number);
CREATE INDEX idx_competitors_name ON competitors(event_id, last_name, first_name);
```

### photos

Photo files and metadata.

```sql
CREATE TABLE photos (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id UUID REFERENCES events(id) ON DELETE CASCADE,
    node_id UUID REFERENCES hierarchy_nodes(id) ON DELETE CASCADE, -- The folder it is in
    competitor_id UUID REFERENCES competitors(id) ON DELETE SET NULL, -- The person it is of
    
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
CREATE INDEX idx_photos_node ON photos(node_id);
CREATE INDEX idx_photos_competitor ON photos(competitor_id);
CREATE INDEX idx_photos_status ON photos(status);
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