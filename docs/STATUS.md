# PhotoFinish - Implementation Status

**Last updated:** 2026-02-05

---

## Data Model (current)

The data model uses flat location fields on photos instead of hierarchy tables. See `02-data-model.md` for full schema.

| Table | Status | Notes |
|-------|--------|-------|
| `events` | Done | Includes `storage_root`, `num_gyms`, `sessions_per_gym` |
| `competitors` | Done | Reusable profiles (not event-specific) |
| `event_competitors` | Done | Per-event roster entries with `session`, `level`, `team_name` |
| `photos` | Done | Flat location: `gym`, `session`, `group_name`, `apparatus`. FK to `event_competitor` |
| `orders` | Not started | |
| `order_items` | Not started | |
| `products` | Not started | |

---

## Tauri Ingestion App (`apps/ingest`)

**Status: Complete**

- Multi-reader dashboard with card-per-reader layout
- Camera brand detection (Sony, Canon, Nikon, Fujifilm, Panasonic, Olympus)
- File copy with progress tracking via Tauri Channel API
- Auto-rename folders and files during copy
- Pinia state persistence across restarts
- Session configuration per reader (destination, photographer, camera brand)

---

## Phoenix Server (`server/`)

### Contexts & Resources (Ash Framework)

| Context | Resource | Status |
|---------|----------|--------|
| `Events` | `Event` | Done |
| `Competitors` | `Competitor` | Done |
| `Competitors` | `EventCompetitor` | Done |
| `Photos` | `Photo` | Done |
| `Ingestion` | `Scanner` | Done — walks storage directory, creates photo records |
| `Ingestion` | `PhotoProcessor` | Done — Oban job for thumbnail/preview generation |
| `Ingestion` | `PathParser` | Done — extracts gym/session/apparatus from folder path |
| `Ingestion` | `CompetitorMatcher` | Done — matches folders to event_competitors |
| `Ingestion` | `RosterImport` | Done — imports roster from parsed data |
| `Ingestion` | `RosterParser` | Done — parses `.txt` roster files |
| `Ingestion` | `FolderAssociation` | Done — links folder names to event_competitors |
| `Ingestion` | `FolderGenerator` | Done — creates folder hierarchy from event config |
| `Viewer` | `Search` | Done — competitor search by name/number/team |
| `Orders` | — | Not started |
| `Accounts` | `User` | Done — Ash Authentication |

### Admin Interface (LiveView)

| Feature | Route | Status |
|---------|-------|--------|
| Event list | `/admin/events` | Done |
| Event create/edit | `/admin/events/new`, `/admin/events/:id/edit` | Done |
| Event detail + photo browser | `/admin/events/:id` | Done |
| Roster import | `/admin/events/:id/competitors/import` | Done — CSV preview, session field |
| Folder association | `/admin/events/:id/folders/associate` | Done — link folders to competitors |
| Dashboard (stats) | — | Not started |
| Ingestion monitoring | — | Not started |
| Bulk photo operations | — | Not started |
| Order management | — | Not started |
| System settings | — | Not started |

### Viewer Interface (LiveView)

| Feature | Route | Status |
|---------|-------|--------|
| Home (search + showcase) | `/viewer` | Done — redesigned with photo showcase |
| Competitor photos | `/viewer/competitor/:id` | Done — photo grid |
| Photo serving | `/photos/thumbnail/:id`, `/photos/preview/:id` | Done |
| Lightbox modal | — | Not started |
| Shopping cart | — | Not started |
| Checkout | — | Not started |
| Order confirmation | — | Not started |
| Real-time PubSub updates | — | Not started |
| Pagination | — | Not started |

### API Endpoints

| Endpoint | Status |
|----------|--------|
| All `/api/*` endpoints | Not started |
| WebSocket channels | Not started |

---

## What to Build Next

### Option A: Ordering System (revenue-critical)
Build the ordering pipeline end-to-end:
1. Create `orders`, `order_items`, `products` migrations
2. Build Order/OrderItem/Product Ash resources
3. Shopping cart in viewer (session-based)
4. Checkout form + order number generation
5. Admin order dashboard + payment recording
6. USB fulfillment UI

### Option B: Viewer Polish
Improve the browsing experience:
1. Lightbox modal with arrow navigation
2. Pagination for large photo sets
3. Real-time PubSub photo updates
4. Touch/keyboard shortcuts

### Option C: API Layer
Enable Tauri-to-Phoenix communication:
1. Ingestion notification endpoint (POST /api/ingestion/notify)
2. Roster sync endpoint
3. Event listing for Tauri app
