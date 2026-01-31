# Admin & Data Model Redesign

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Simplify event setup and data model by removing over-engineered hierarchy system.

**Date:** January 31, 2025

---

## Context

The current `hierarchy_levels` and `hierarchy_nodes` tables over-engineer the folder structure. For gymnastics events (90%+ of use cases), the structure is always:

```
Event Root/
  Gym A/
    Session 1A/
      Group 1A/
        Beam/
          1059 Iza Z/
            photos.jpg
```

Only Gym and Session need pre-creation. Everything below is created dynamically by the barcode scanning + download process.

---

## Data Model Changes

### Remove Tables
- `hierarchy_levels` - delete entirely
- `hierarchy_nodes` - delete entirely

### ID Format

All entities use prefixed IDs: `prefix_` + 3 lowercase letters + 4 numbers

```elixir
def generate_id(prefix) do
  letters = Nanoid.generate(3, "abcdefghijklmnopqrstuvwxyz")
  numbers = Nanoid.generate(4, "0123456789")
  "#{prefix}_#{letters}#{numbers}"
end
```

| Entity | Prefix | Example |
|--------|--------|---------|
| Event | `evt_` | `evt_kqz4821` |
| Photo | `pho_` | `pho_mxr0179` |
| Competitor | `cmp_` | `cmp_bln6534` |
| Order | `ord_` | `ord_abc1234` |
| Order Item | `itm_` | `itm_def5678` |
| Product | `prd_` | `prd_ghi9012` |

### Modify `events` Table

```sql
CREATE TABLE events (
    id VARCHAR(12) PRIMARY KEY,  -- evt_abc1234
    name VARCHAR(255) NOT NULL,
    slug VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,

    -- Folder generation
    storage_root VARCHAR(500) NOT NULL,
    num_gyms INTEGER NOT NULL DEFAULT 1,
    sessions_per_gym INTEGER NOT NULL DEFAULT 1,

    -- Business
    order_code VARCHAR(10),
    tax_rate_basis_points INTEGER DEFAULT 850,

    -- Status
    status VARCHAR(50) DEFAULT 'active',
    starts_at TIMESTAMP,
    ends_at TIMESTAMP,

    inserted_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
```

### Modify `photos` Table

```sql
CREATE TABLE photos (
    id VARCHAR(12) PRIMARY KEY,  -- pho_abc1234
    event_id VARCHAR(12) REFERENCES events(id) ON DELETE CASCADE,
    competitor_id VARCHAR(12) REFERENCES competitors(id) ON DELETE SET NULL,

    -- Location (parsed from folder path)
    gym VARCHAR(10),              -- "A", "B", "C"
    session VARCHAR(10),          -- "1A", "2B"
    group_name VARCHAR(50),       -- "Group 1A", "Group 2B"
    apparatus VARCHAR(50),        -- "Beam", "Floor"

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

    -- Timestamps
    captured_at TIMESTAMP,
    discovered_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    processed_at TIMESTAMP,

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
CREATE INDEX idx_photos_competitor ON photos(competitor_id);
CREATE INDEX idx_photos_location ON photos(event_id, gym, session, apparatus);
CREATE INDEX idx_photos_status ON photos(status);
```

### Modify `competitors` Table

```sql
CREATE TABLE competitors (
    id VARCHAR(12) PRIMARY KEY,  -- cmp_abc1234
    event_id VARCHAR(12) REFERENCES events(id) ON DELETE CASCADE,

    competitor_number VARCHAR(50) NOT NULL,
    first_name VARCHAR(100) NOT NULL,
    last_name VARCHAR(100),
    display_name VARCHAR(255),

    team_name VARCHAR(255),
    level VARCHAR(50),
    age_group VARCHAR(50),

    email VARCHAR(255),
    phone VARCHAR(50),

    is_active BOOLEAN DEFAULT true,
    metadata JSONB,

    inserted_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,

    UNIQUE(event_id, competitor_number)
);

CREATE INDEX idx_competitors_event ON competitors(event_id);
CREATE INDEX idx_competitors_number ON competitors(event_id, competitor_number);
```

---

## Event Creation Flow

### UI

Simple form:
- Event Name (required)
- Storage Path (required) - where folders will be created
- Number of Gyms (default: 1) - creates Gym A, B, C...
- Sessions per Gym (default: 1) - creates Session 1A, 2A... per gym

Preview shows folder structure before creation.

### Folder Generation

On event create:
```elixir
def create_event_folders(storage_root, num_gyms, sessions_per_gym) do
  for gym_num <- 1..num_gyms,
      session_num <- 1..sessions_per_gym do
    gym_letter = <<(?A + gym_num - 1)>>
    session_name = "Session #{session_num}#{gym_letter}"
    path = Path.join([storage_root, "Gym #{gym_letter}", session_name])
    File.mkdir_p!(path)
  end
end
```

Creates structure like:
```
/storage_root/Gym A/Session 1A/
/storage_root/Gym A/Session 2A/
/storage_root/Gym B/Session 1B/
/storage_root/Gym B/Session 2B/
```

---

## Scanning & Path Parsing

### Path Format

```
/storage_root/Gym A/Session 1A/Group 2B/Beam/1059 Iza Z/IMG_001.jpg
              ─────  ──────────  ────────  ────  ──────────
              gym    session     group     apparatus  competitor
```

### Parsing Logic

```elixir
def parse_photo_path(full_path, storage_root) do
  relative = String.replace_prefix(full_path, storage_root <> "/", "")
  parts = String.split(relative, "/")

  case parts do
    [gym_folder, session_folder, group, apparatus, competitor_folder, filename] ->
      %{
        gym: parse_gym(gym_folder),           # "A"
        session: parse_session(session_folder), # "1A"
        group_name: group,                    # "Group 2B"
        apparatus: apparatus,                 # "Beam"
        competitor_folder: competitor_folder, # "1059 Iza Z"
        filename: filename
      }
    _ ->
      {:error, :invalid_path}
  end
end

defp parse_gym("Gym " <> letter), do: letter
defp parse_session("Session " <> id), do: id
```

### Competitor Matching

```elixir
def parse_competitor_number(folder_name) do
  case Regex.run(~r/^(\d+)\s+/, folder_name) do
    [_, number] -> {:ok, number}
    _ -> {:error, :no_match}
  end
end
```

Photos with no competitor match get `competitor_id: nil` for manual review.

---

## Viewer Browsing

### URL Structure

```
/e/evt_abc1234                           → Event home
/e/evt_abc1234/a                         → Gym A
/e/evt_abc1234/a/1a                      → Session 1A
/e/evt_abc1234/a/1a/group-2b             → Group 2B
/e/evt_abc1234/a/1a/group-2b/beam        → Beam
/e/evt_abc1234/a/1a/group-2b/beam/1059   → Competitor 1059's photos
```

### Queries

Simple filter on flat fields:
```elixir
Photo
|> where(event_id: ^event_id)
|> where(gym: "A")
|> where(session: "1A")
|> where(apparatus: "Beam")
```

No hierarchy table joins needed.

---

## Migration Strategy

Fresh start - delete existing migrations and reset:
```bash
rm priv/repo/migrations/*
mix ecto.reset
```

Create new migrations for simplified schema.

---

## Implementation Tasks

1. Add `nanoid` dependency and create ID generation module
2. Delete hierarchy_levels and hierarchy_nodes resources
3. Update events resource with new fields (num_gyms, sessions_per_gym, storage_root)
4. Update photos resource with location fields (gym, session, group_name, apparatus)
5. Update competitors resource (remove node_id)
6. Create folder generation module
7. Update scanner to parse paths and populate location fields
8. Build new event creation UI
9. Update admin event show page
10. Update viewer routes and queries
