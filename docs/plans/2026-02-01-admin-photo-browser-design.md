# Admin Photo Browser Design

**Date:** 2026-02-01

## Overview

Add hierarchical photo browsing to the Admin Event Show page, allowing admins to drill down through the folder structure and view processed photos.

## Requirements

- Browse photos via hierarchical drill-down: Gym → Session → Group → Apparatus → Photos
- Inline navigation on Event Show page (expand/collapse, no separate routes)
- Photo grid at leaf level showing thumbnails with filenames
- Competitor info displayed at top of grid (if assigned)
- Future: Reassign photos to different competitors

## Data Structure

Photos have flat location fields: `gym`, `session`, `group_name`, `apparatus`

Navigation state tracked as path list:
- `[]` → Show gym cards
- `["Gym A"]` → Show session cards
- `["Gym A", "Session 1"]` → Show group cards
- `["Gym A", "Session 1", "Group 1A"]` → Show apparatus cards
- `["Gym A", "Session 1", "Group 1A", "Vault"]` → Show photo grid

## UI Components

**Breadcrumb bar:**
```
Photos > Gym A > Session 1 > Group 1A
```
Clickable links to navigate back up the hierarchy.

**Level cards:**
- Name (e.g., "Gym A", "Vault")
- Photo count badge

**Photo grid:**
- Header with competitor info (if photos assigned to one competitor)
- Responsive thumbnail grid
- Filename below each thumbnail

## Storage Changes

Thumbnails and previews stored within event's storage_root:

```
{storage_root}/
├── Gym A/
│   └── Session 1/
│       └── ... (source photos)
├── _thumbnails/
│   └── {photo_id}.jpg
└── _previews/
    └── {photo_id}.jpg
```

## Implementation Tasks

1. **PhotoProcessor** - Use `event.storage_root/_thumbnails/` and `_previews/`
2. **Scanner** - Skip directories starting with `_`
3. **Event Show page** - Add hierarchical browser with:
   - Path-based navigation state
   - Breadcrumb component
   - Level cards with counts
   - Photo grid at leaf level
   - Image serving for thumbnails

## Scope

All changes in existing modules:
- `PhotoFinish.Ingestion.PhotoProcessor`
- `PhotoFinish.Ingestion.Scanner` (or wherever scan logic lives)
- `PhotoFinishWeb.Admin.EventLive.Show`

No new routes needed.
