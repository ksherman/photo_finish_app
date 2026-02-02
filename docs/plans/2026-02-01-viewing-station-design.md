# Viewing Station MVP Design

**Date:** 2026-02-01
**Status:** Approved

---

## Overview

Build a search-first public viewing station for browsing and purchasing photos. Families search by competitor name/number rather than navigating the event hierarchy.

**Prerequisites:** Competitor import and folder-to-competitor association must be built first.

---

## Data Model Changes

### New Table: `event_competitors`

Join table linking competitors to events with per-event details.

```sql
CREATE TABLE event_competitors (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    competitor_id UUID REFERENCES competitors(id) ON DELETE CASCADE,
    event_id UUID REFERENCES events(id) ON DELETE CASCADE,
    node_id UUID REFERENCES hierarchy_nodes(id) ON DELETE SET NULL,

    session VARCHAR(50),              -- "3A", "11B"
    competitor_number VARCHAR(50) NOT NULL,
    display_name VARCHAR(255),        -- "1022 Kevin S"

    team_name VARCHAR(255),
    level VARCHAR(50),
    age_group VARCHAR(50),

    is_active BOOLEAN DEFAULT true,
    metadata JSONB,

    inserted_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,

    UNIQUE(event_id, competitor_number)
);

CREATE INDEX idx_event_competitors_event ON event_competitors(event_id);
CREATE INDEX idx_event_competitors_competitor ON event_competitors(competitor_id);
CREATE INDEX idx_event_competitors_session ON event_competitors(event_id, session);
CREATE INDEX idx_event_competitors_node ON event_competitors(node_id);
```

### Modified Table: `competitors`

Simplified to represent the person across events.

```sql
CREATE TABLE competitors (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    first_name VARCHAR(100) NOT NULL,
    last_name VARCHAR(100),

    external_id VARCHAR(100),         -- USAG number, for cross-event linking
    email VARCHAR(255),
    phone VARCHAR(50),

    metadata JSONB,

    inserted_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_competitors_external_id ON competitors(external_id);
CREATE INDEX idx_competitors_name ON competitors(last_name, first_name);
```

### Modified Table: `photos`

Change `competitor_id` to reference `event_competitors`.

```sql
ALTER TABLE photos
    DROP COLUMN competitor_id,
    ADD COLUMN event_competitor_id UUID REFERENCES event_competitors(id) ON DELETE SET NULL;

CREATE INDEX idx_photos_event_competitor ON photos(event_competitor_id);
```

---

## Feature 1: Competitor Import

### Admin UI Flow

1. Navigate to Event → "Competitors" tab
2. Click "Import Roster"
3. Select session from dropdown (e.g., "3A")
4. Upload `.txt` file
5. Preview parsed competitors
6. Confirm import

### Parsing Rules

Text file format: one competitor per line, `NUMBER NAME`

```
143 Avery W
169 Callie W
```

Parser splits on first space:
- `competitor_number`: "143"
- `name`: "Avery W" (stored as first_name + last_name if space found)

### Implementation

- New LiveView: `Admin.CompetitorLive.Import`
- Context function: `Events.import_competitors(event_id, session, file_content)`
- Creates both `competitors` and `event_competitors` records
- No deduplication for now (future: match by external_id)

---

## Feature 2: Folder-to-Competitor Association

### Admin UI Flow

1. Navigate to Event → hierarchy node (e.g., "Group 3A / Beam")
2. See split view:
   - Left: Unassigned child folders with photo counts
   - Right: Session competitors (filtered to session 3A)
3. Click folder, then click competitor to assign
4. Repeat for all folders
5. Click "Save Assignments"

### UI Layout

```
┌────────────────────────────┬────────────────────────────────────┐
│ UNASSIGNED FOLDERS         │ SESSION 3A COMPETITORS             │
│                            │                                    │
│ ● Gymnast 01  (24 photos)  │ 1022  Kevin S      ←───────────   │
│ ○ Gymnast 02  (31 photos)  │ 1045  Sarah J                      │
│ ○ Gymnast 03  (28 photos)  │ 1089  Emma W                       │
└────────────────────────────┴────────────────────────────────────┘
```

### Implementation

- New LiveView: `Admin.NodeLive.Associate`
- On save:
  - Set `event_competitor.node_id` to the folder's hierarchy_node
  - Update all `photos` in that node: set `event_competitor_id`

---

## Feature 3: Viewing Station (Public)

### URL Structure

```
/view                           # Event home, search bar
/view/search?q=kevin            # Search results
/view/competitor/:id            # Photo grid for competitor
/view/photo/:id                 # Lightbox (direct link)
```

### Home Screen

Large search bar, event name, total photo count. No hierarchy navigation.

```
┌─────────────────────────────────────────────────────────────────┐
│                        PhotoFinish                              │
│           ┌─────────────────────────────────────┐              │
│           │ 🔍  Search by name or number...     │              │
│           └─────────────────────────────────────┘              │
│                   2024 Xcel Regionals                          │
│                     45,230 photos                              │
└─────────────────────────────────────────────────────────────────┘
```

### Search

- Live search as user types (debounced)
- Searches: competitor_number, first_name, last_name
- Returns top 10 matches with photo counts
- Click result → photo grid

### Photo Grid

- Thumbnails in responsive grid (3-6 columns)
- Lazy loading
- Tap thumbnail → lightbox
- Back button → search

### Lightbox

- Full preview image (1280px)
- Swipe/arrow navigation
- "Add to Cart" button (cart functionality deferred)
- Close button

### Implementation

- New LiveView: `ViewerLive.Home` (search)
- New LiveView: `ViewerLive.Competitor` (photo grid + lightbox)
- No authentication required
- Tablet-optimized (large tap targets, touch gestures)

---

## Implementation Order

1. **Data model migration** - New tables, modify photos
2. **Competitor import** - Admin UI + parsing
3. **Folder association** - Admin UI + assignment logic
4. **Viewing station** - Public search + photo grid

---

## Future Enhancements (Not in MVP)

- CSV import with additional fields (team, level, email)
- Cross-event competitor deduplication via external_id
- Hierarchy browsing as alternate navigation
- Real-time photo updates via PubSub
- Cart and checkout
