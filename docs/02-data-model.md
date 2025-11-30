# PhotoFinish - Data Model

**Version:** 1.0  
**Date:** November 29, 2025

---

## Schema Overview

```
events
  └── hierarchy_levels (defines structure per event)
  └── hierarchy_nodes (actual organizational units)
      └── photos
  └── competitors (roster data)
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
    event_date DATE,
    status VARCHAR(50) DEFAULT 'active',  -- active, archived
    order_code VARCHAR(10),  -- 3-letter code for order numbers (e.g., "STV")
    tax_rate DECIMAL(5,2) DEFAULT 8.5,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
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
    level_name VARCHAR(100) NOT NULL,  -- "Gym", "Session", "Event"
    level_name_plural VARCHAR(100),
    is_required BOOLEAN DEFAULT true,
    allow_photos BOOLEAN DEFAULT false,  -- Only lowest level typically true
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(event_id, level_number)
);

CREATE INDEX idx_hierarchy_levels_event ON hierarchy_levels(event_id);
```

**Example (Gymnastics Meet):**
| level_number | level_name | allow_photos |
|--------------|------------|--------------|
| 1 | Gym | false |
| 2 | Session | false |
| 3 | Apparatus | false |
| 4 | Flight | false |
| 5 | Competitor | true |

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
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_hierarchy_nodes_event ON hierarchy_nodes(event_id);
CREATE INDEX idx_hierarchy_nodes_parent ON hierarchy_nodes(parent_id);
CREATE INDEX idx_hierarchy_nodes_level ON hierarchy_nodes(event_id, level_number);
CREATE INDEX idx_hierarchy_nodes_slug ON hierarchy_nodes(event_id, slug);
CREATE INDEX idx_hierarchy_nodes_path ON hierarchy_nodes(event_id, parent_id, level_number);
```

### competitors

Roster data linked to hierarchy nodes.

```sql
CREATE TABLE competitors (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id UUID REFERENCES events(id) ON DELETE CASCADE,
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
    
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    deleted_at TIMESTAMP,
    
    UNIQUE(event_id, competitor_number)
);

CREATE INDEX idx_competitors_event ON competitors(event_id);
CREATE INDEX idx_competitors_node ON competitors(node_id);
CREATE INDEX idx_competitors_number ON competitors(event_id, competitor_number);
CREATE INDEX idx_competitors_name ON competitors(event_id, last_name, first_name);
CREATE INDEX idx_competitors_team ON competitors(event_id, team_name);
CREATE INDEX idx_competitors_active ON competitors(event_id, is_active) WHERE deleted_at IS NULL;
```

### photos

Photo files and metadata.

```sql
CREATE TABLE photos (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id UUID REFERENCES events(id) ON DELETE CASCADE,
    node_id UUID REFERENCES hierarchy_nodes(id) ON DELETE CASCADE,
    competitor_id UUID REFERENCES competitors(id) ON DELETE SET NULL,
    
    -- File paths (two-phase approach)
    ingestion_path VARCHAR(1000) NOT NULL,  -- Original path when discovered
    current_path VARCHAR(1000),  -- Current path after moves
    preview_path VARCHAR(1000),
    thumbnail_path VARCHAR(1000),
    
    -- Optional URLs (if NAS serves directly)
    file_url VARCHAR(1000),
    preview_url VARCHAR(1000),
    thumbnail_url VARCHAR(1000),
    
    -- File info
    filename VARCHAR(255) NOT NULL,
    original_filename VARCHAR(255),
    file_size_bytes BIGINT,
    mime_type VARCHAR(100) DEFAULT 'image/jpeg',
    width INTEGER,
    height INTEGER,
    
    -- Ingestion tracking
    photographer VARCHAR(100),
    order_number VARCHAR(50),  -- Memory card sequence
    source_folder VARCHAR(255),
    
    -- Timestamps
    captured_at TIMESTAMP,  -- From EXIF
    discovered_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    processed_at TIMESTAMP,
    finalized_at TIMESTAMP,
    
    -- Status
    status VARCHAR(50) DEFAULT 'discovered',
    -- discovered, processing, ready, finalized, error, duplicate
    error_message TEXT,
    
    -- Metadata
    exif_data JSONB,
    metadata JSONB
);

CREATE INDEX idx_photos_event ON photos(event_id);
CREATE INDEX idx_photos_node ON photos(node_id);
CREATE INDEX idx_photos_competitor ON photos(competitor_id);
CREATE INDEX idx_photos_status ON photos(status);
CREATE INDEX idx_photos_event_status ON photos(event_id, status);
CREATE INDEX idx_photos_filename ON photos(filename);
CREATE INDEX idx_photos_captured ON photos(captured_at);
CREATE INDEX idx_photos_photographer ON photos(photographer);
CREATE INDEX idx_photos_event_node ON photos(event_id, node_id);
```

**Photo Status Values:**
| Status | Description |
|--------|-------------|
| `discovered` | Found by watcher, not processed |
| `processing` | Generating preview/thumbnail |
| `ready` | Available for viewing |
| `finalized` | Moved to archival structure |
| `error` | Processing failed |
| `duplicate` | Already imported |

---

## Ordering Tables

### orders

```sql
CREATE TABLE orders (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id UUID REFERENCES events(id) ON DELETE CASCADE,
    
    order_number VARCHAR(50) UNIQUE NOT NULL,  -- "STV-0793"
    
    -- Customer
    customer_name VARCHAR(255) NOT NULL,
    customer_email VARCHAR(255),
    customer_phone VARCHAR(50),
    
    -- Financial
    subtotal_cents INTEGER NOT NULL,
    tax_rate DECIMAL(5,2) NOT NULL,
    tax_cents INTEGER NOT NULL,
    total_cents INTEGER NOT NULL,
    
    -- Payment
    payment_status VARCHAR(50) NOT NULL DEFAULT 'pending',
    -- pending, paid, refunded
    payment_method VARCHAR(50),  -- square, cash, check
    payment_reference VARCHAR(255),
    paid_at TIMESTAMP,
    
    -- Accounting
    needs_accounting_review BOOLEAN DEFAULT false,
    accounting_notes TEXT,
    
    notes TEXT,
    internal_notes TEXT,
    
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_orders_event ON orders(event_id);
CREATE INDEX idx_orders_number ON orders(order_number);
CREATE INDEX idx_orders_payment_status ON orders(event_id, payment_status);
CREATE INDEX idx_orders_created ON orders(event_id, created_at DESC);
```

### order_items

```sql
CREATE TABLE order_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    order_id UUID REFERENCES orders(id) ON DELETE CASCADE,
    
    product_type VARCHAR(50) NOT NULL,
    -- usb_drive, print, collage, custom_photo
    product_size VARCHAR(50),  -- NULL for USB, or "5x7", "8x10", etc.
    product_name VARCHAR(255),
    
    -- For USB: stores competitor IDs
    -- For prints: stores single photo ID
    photo_id UUID REFERENCES photos(id) ON DELETE SET NULL,
    competitor_ids JSONB,  -- For USB: ["uuid1", "uuid2"]
    
    -- Pricing
    unit_price_cents INTEGER NOT NULL,
    quantity INTEGER DEFAULT 1,
    line_total_cents INTEGER NOT NULL,
    
    -- Fulfillment
    fulfillment_status VARCHAR(50) DEFAULT 'pending',
    -- pending, preparing, ready, delivered
    fulfilled_at TIMESTAMP,
    
    notes TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_order_items_order ON order_items(order_id);
CREATE INDEX idx_order_items_photo ON order_items(photo_id);
CREATE INDEX idx_order_items_fulfillment ON order_items(fulfillment_status);
CREATE INDEX idx_order_items_type ON order_items(product_type);
```

### products

```sql
CREATE TABLE products (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id UUID REFERENCES events(id) ON DELETE CASCADE,
    -- NULL event_id = global default
    
    product_type VARCHAR(50) NOT NULL,
    product_name VARCHAR(255) NOT NULL,
    product_description TEXT,
    product_size VARCHAR(50),
    
    price_cents INTEGER NOT NULL,
    
    is_active BOOLEAN DEFAULT true,
    display_order INTEGER DEFAULT 0,
    
    sku VARCHAR(100),
    metadata JSONB,
    
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    UNIQUE(event_id, product_type, product_size)
);

CREATE INDEX idx_products_event_active ON products(event_id, is_active);
```

**Default Products:**
| Type | Size | Price |
|------|------|-------|
| usb_drive | - | $100.00 |
| print | 5x7 | $18.00 |
| print | 8x10 | $30.00 |
| print | 11x14 | $40.00 |
| print | 16x20 | $65.00 |

---

## Key Queries

### Get hierarchy path for a node

```sql
WITH RECURSIVE node_path AS (
    SELECT id, parent_id, level_number, name, slug, 1 as depth
    FROM hierarchy_nodes
    WHERE id = $1
    
    UNION ALL
    
    SELECT hn.id, hn.parent_id, hn.level_number, hn.name, hn.slug, np.depth + 1
    FROM hierarchy_nodes hn
    INNER JOIN node_path np ON hn.id = np.parent_id
)
SELECT * FROM node_path ORDER BY depth DESC;
```

### Get photos for a competitor

```sql
SELECT p.*
FROM photos p
WHERE p.competitor_id = $1
  AND p.status = 'ready'
ORDER BY p.captured_at, p.filename;
```

### Search competitors by name

```sql
SELECT c.*, COUNT(p.id) as photo_count
FROM competitors c
LEFT JOIN photos p ON p.competitor_id = c.id AND p.status = 'ready'
WHERE c.event_id = $1
  AND c.is_active = true
  AND (c.first_name ILIKE $2 OR c.last_name ILIKE $2 OR c.competitor_number ILIKE $2)
GROUP BY c.id
ORDER BY c.last_name, c.first_name;
```

### Get all photos for USB order (multiple competitors)

```sql
SELECT p.*, c.display_name, c.competitor_number,
       hn.name as apparatus_name
FROM photos p
JOIN competitors c ON p.competitor_id = c.id
JOIN hierarchy_nodes hn ON p.node_id = hn.id
WHERE c.id = ANY($1::uuid[])  -- Array of competitor IDs
  AND p.status IN ('ready', 'finalized')
ORDER BY c.competitor_number, hn.display_order, p.filename;
```

### Photo count by hierarchy node (including descendants)

```sql
WITH RECURSIVE node_tree AS (
    SELECT id FROM hierarchy_nodes WHERE id = $1
    UNION ALL
    SELECT hn.id FROM hierarchy_nodes hn
    INNER JOIN node_tree nt ON hn.parent_id = nt.id
)
SELECT COUNT(*)
FROM photos p
WHERE p.node_id IN (SELECT id FROM node_tree)
  AND p.status = 'ready';
```

---

## Ecto Schema Notes

### Associations

```elixir
# Event has many of everything
schema "events" do
  has_many :hierarchy_levels, HierarchyLevel
  has_many :hierarchy_nodes, HierarchyNode
  has_many :competitors, Competitor
  has_many :photos, Photo
  has_many :orders, Order
  has_many :products, Product
end

# HierarchyNode is self-referential
schema "hierarchy_nodes" do
  belongs_to :event, Event
  belongs_to :parent, HierarchyNode
  has_many :children, HierarchyNode, foreign_key: :parent_id
  has_many :photos, Photo, foreign_key: :node_id
end

# Photo belongs to node and optionally competitor
schema "photos" do
  belongs_to :event, Event
  belongs_to :node, HierarchyNode
  belongs_to :competitor, Competitor
end

# Competitor linked to node when photos exist
schema "competitors" do
  belongs_to :event, Event
  belongs_to :node, HierarchyNode
  has_many :photos, Photo
end
```

### Soft Deletes

Use `deleted_at` timestamp for competitors. Query with:
```elixir
from c in Competitor, where: is_nil(c.deleted_at)
```
