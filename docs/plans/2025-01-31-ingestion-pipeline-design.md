# Ingestion Pipeline Design

**Date:** 2025-01-31
**Status:** Ready for Implementation

## Overview

Manual scan-triggered photo discovery and processing pipeline. Admin clicks "Scan" in the UI, Phoenix walks the event's storage directory, creates database records, and queues background jobs for thumbnail/preview generation.

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Discovery trigger | Manual "Scan" button | Avoids race conditions when reorganizing files; automate later |
| Folder handling | Mirror to hierarchy nodes | Folder structure = hierarchy; parse path to create nodes |
| Unmatched folders | Import as unlinked photos | Photos get DB records, admin assigns competitors later |
| Processing | Background queue (Oban) | Fast scan, thumbnails fill in async |
| Root path | Event's `storage_directory` | Configured per-event in admin |
| Duplicate detection | filename + size + mtime | Fast, practical for this use case |
| Error handling | Mark as "error" status | Visible in admin for review/retry |

## Data Flow

```
Admin clicks "Scan"
    → Scanner walks event.storage_directory
    → For each folder: create/update hierarchy_node
    → For each JPEG: create photo record (if not exists)
    → For leaf folders: attempt competitor matching
    → Queue Oban jobs for thumbnail/preview generation
```

## Photo States

| Status | Meaning |
|--------|---------|
| `discovered` | File found, not yet processed |
| `processing` | Thumbnail/preview being generated |
| `ready` | Fully processed, viewable |
| `error` | Processing failed, needs attention |

## Competitor Linking

- Leaf folders (where photos live) are parsed for competitor patterns
- Pattern: `{number} {name}` (e.g., "1022 Kevin S")
- Regex: `~r/^(\d+)\s+/` extracts competitor number
- If pattern matches roster entry → link photos to competitor
- If no match → photos remain unlinked (`competitor_id: nil`)

## Module Structure

```
lib/photo_finish/
├── ingestion/
│   ├── scanner.ex          # Walks directories, creates records
│   ├── folder_parser.ex    # Parses folder path into hierarchy levels
│   ├── competitor_matcher.ex # Matches folder names to roster entries
│   └── photo_processor.ex  # Oban worker - generates thumbnails/previews
```

### Scanner

- `scan_event(event_id)` - Entry point
- Walks `event.storage_directory` recursively
- Creates `hierarchy_node` records mirroring folder structure
- Creates `photo` records for each JPEG found
- Returns `{:ok, %{photos_found: n, photos_new: n, errors: []}}`

### Folder Parser

- Input: `/NAS/events/valentines/Gym A/Session 3/Group 1A/Beam/1022 Kevin S`
- Strips event's `storage_directory` prefix
- Output: List of `{level_number, folder_name}` tuples

### Competitor Matcher

- Input: folder name like "1022 Kevin S"
- Extracts competitor number via regex
- Looks up in event's roster by `competitor_number`
- Returns `{:ok, competitor}` or `:no_match`

### Photo Processor (Oban Worker)

```elixir
use Oban.Worker, queue: :media, max_attempts: 3

@thumbnail_size 320
@preview_size 1280
```

**Processing steps:**
1. Read original JPEG from `photo.ingestion_path`
2. Generate thumbnail (320px longest edge)
3. Generate preview (1280px longest edge)
4. Save to configured paths
5. Update photo record with paths and `status: ready`

## File Paths

| Type | Location | Example |
|------|----------|---------|
| Original | NAS (stays in place) | `/NAS/events/.../1022 Kevin S/IMG_001.JPG` |
| Thumbnail | Thumbnail root | `/NAS/thumbnails/valentines-2025/{photo_id}.jpg` |
| Preview | Preview root | `/NAS/previews/valentines-2025/{photo_id}.jpg` |

## Admin UI

On Event Show page (`/admin/events/:id`), add "Ingestion" section:

```
┌─────────────────────────────────────────────────────┐
│ Ingestion                                           │
├─────────────────────────────────────────────────────┤
│ Storage Path: /NAS/events/valentines-2025           │
│                                                     │
│ Photos: 12,456 ready | 23 processing | 3 errors     │
│                                                     │
│ Last Scan: 2 minutes ago                            │
│                                                     │
│ [Scan Now]                                          │
└─────────────────────────────────────────────────────┘
```

- "Scan Now" runs scan synchronously, shows flash with results
- Error count links to filtered photo list
- "Unlinked Photos" view for photos without competitor assignment

## Error Handling

**File System Errors:**

| Error | Handling |
|-------|----------|
| Storage directory doesn't exist | Scan returns error, flash message |
| File unreadable | Mark photo as `error`, continue scan |
| Corrupt JPEG | Mark photo as `error` with message |
| NAS disconnected mid-scan | Scan fails, admin retries |

**Scan Idempotency:**

- Running scan twice is safe
- Existing photos (matched by filename+size+mtime) are skipped
- Existing hierarchy nodes are reused (matched by path)

## Configuration

Uses existing config from `runtime.exs`:
- `photo_root` - where originals live (event's storage_directory)
- `preview_root` - where previews are written
- `thumbnail_root` - where thumbnails are written
- `thumbnail_size` - default 320px
- `preview_size` - default 1280px
- `max_concurrent_processing` - Oban concurrency, default 4

## Not in MVP

- File watcher (automatic discovery)
- Watermarking on previews
- Scan progress indicator
- "Verify" action to check files still exist
- Bulk retry for errored photos
