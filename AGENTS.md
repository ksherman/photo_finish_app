# AGENTS.md — PhotoFinish Context Bootstrap

> **Purpose:** Fast-start orientation for future sessions (human or AI). Read this first to understand the business problem, current codebase state, architecture, conventions, and active open decisions without scanning the whole repo.

---

## 1. What PhotoFinish Is

**PhotoFinish** is a local-network photo ingest, viewing, and on-site sales system for live sporting events (primary use case: **youth gymnastics meets**).

- **Scale per meet:** up to ~100,000–165,000 JPEG photos (~325–700 GB originals, ~35 GB thumbnails/previews), 1,000+ competitors, ~15 concurrent viewing stations (iPads/tablets on local Wi-Fi).
- **Season & multi-venue reality:** ~850,000 photos across ~12 competition weekends, with up to **4 concurrent meets running simultaneously at different venues** on peak state-meet weekends.
- **Business scale:** ~$310K–$350K/season (~$250K+ card-present on-site, ~$60K–$100K online post-event).
- **Legacy replacement:** Replaces an older Windows stack archived in `as-built/` (**Photo Parata** — a self-contained Windows `.exe` with embedded SQLite + Apache/PHP — plus `KioskViewingStation.exe` and `downloader.exe`) and replaces PhotoReflect (~15–18% fee) with **ShootProof Unlimited ($600/yr, 0% commission, Studio API)**.
- **Core meet workflow:**
  `Photographers (SD cards) → Ingest App (copy & rename to storage) → Local Server (index, thumbnail/preview, match roster) → Families browse & order on kiosk tablets (Digital / Single Digital / Prints / +$10 Instant USB) → Square checkout → Local USB copy & post-meet ShootProof API upload (galleries, zip downloads, WHCC/Bay Photo drop-ship)`.

---

## 2. Repository Map

| Path | What It Contains | Footprint / Notes |
| --- | --- | --- |
| `apps/photofinish/` | **Active unified app (v2):** Cargo workspace (`photofinish-core` + `photofinish-desktop` Tauri 2) + Vue 3 / TS / Pinia / Tailwind v4 + bundled SQLite + embedded Axum LAN kiosk server + pure-Rust SIMD imaging | Primary development target (Apple Silicon macOS priority) |
| `server/` | *(Reference prototype)* Elixir/Phoenix 1.8 web server, Ash 3 domain layer, LiveView Admin & Viewer UI, Oban + `vix` (libvips) | ~7.5k LOC handwritten Elixir, ~3k LOC tests — never proven in production; keep as reference only |
| `server/AGENTS.md` | Auto-generated Elixir / Phoenix 1.8 / LiveView / Ash usage rules (4,005 lines — consult only if reading/editing `server/`) | Do not load blindly into context |
| `apps/ingest/` | *(Reference / superseded by `apps/photofinish/`)* Original standalone Tauri 2 card-copying app | ~500 LOC Rust, ~1.1k LOC Vue/TS — refined UX ported into `apps/photofinish/` |
| `docker-compose.yml` | Local PostgreSQL 18 container (`photofinish_db`) mapped to host port `8200` | Used by `server/` in dev |
| `docs/` | Core architecture, data model, ingestion, viewer, admin, ordering, API, and Square payment specs | `docs/STATUS.md` is **stale** (dated 2026-02-05, before orders & later ingest commits) |
| `docs/future/` | Future design docs: **ShootProof Studio API v3 direction** (`shootproof-integration.md`, supersedes custom R2/B2 storefront), older custom online gallery/digital delivery notes, payment provider comparison, photographer QC | `shootproof-integration.md` is the latest target direction |
| `as-built/` | Legacy Windows software (`Photo Parata`, `KioskViewingStation`, `downloader.exe`) and screenshots | Reference only (untracked) |
| `test-images/` & `test-xcel/` | Sample meet folder structures, sample `.txt` rosters, example camera DCIM cards, and test outputs | Local test fixtures |

---

## 3. Tech Stack

- **Active Unified App (`apps/photofinish/`):**
  - **Core (`crates/photofinish-core`):** Rust (Cargo workspace), `rusqlite` (bundled SQLite 3 in WAL mode + online pre-migration backups), `axum` 0.8 (embedded LAN kiosk server), `image` 0.25 + `fast_image_resize` 5 (pure-Rust SIMD NEON JPEG thumbnail/preview pipeline with EXIF orientation), macOS `diskutil`/`ioreg` USB hardware serial discovery.
  - **Desktop (`src-tauri/` + `src/`):** Tauri 2 (`photofinish-desktop`) + Vue 3 (Composition API) / TypeScript / Pinia / Tailwind CSS v4 / pnpm.
  - **Target OS Priority:** Apple Silicon macOS (`arm64` / `aarch64-apple-darwin`) first; OS hardware layer isolated under `hardware/` for later Windows support.
- **Legacy Reference Prototypes (`server/` & `apps/ingest/`):** Elixir/Phoenix 1.8 + Ash 3 + PostgreSQL 18 + Oban + `vix`, plus standalone Tauri ingest prototype.

---

## 4. Ground-Truth Implementation Status

*(Supersedes the outdated `docs/STATUS.md`.)*

### Built & Working — `apps/photofinish/` (Active Unified App)
- **Core (`crates/photofinish-core`):**
  - Portable Event Bundle storage (`<event>/_photofinish/photofinish.db` in SQLite WAL mode, relative-only paths) + machine-local `photofinish_app.db` (library bookmarks, USB-serial reader configs, settings).
  - Forward-only embedded SQLite migration runner with automatic online pre-migration backups (`_photofinish/backups/`) and newer-schema protection (`SchemaTooNew`).
  - Sport-agnostic event presets (`gymnastics`, `dance`, `simple`, `custom`), customizable `subject_label`, configurable folder scaffolding, flexible path-to-tags parser, and chronological `ingest_batches` audit trail.
  - Card reader discovery (macOS `diskutil` + `ioreg` USB hardware serial) and non-blocking copy with folder toggles, folder/file auto-rename, and immediate DB indexing.
  - Pure-Rust SIMD JPEG thumbnail (320px) + preview (1280px) generator (`image` 0.25 + `fast_image_resize` 5) with EXIF orientation support and bounded background worker pool.
  - Embedded `axum` 0.8 LAN kiosk HTTP server (`/`, `/api/status`, `/api/subjects`, `/api/subjects/{id}/photos`, `/photos/thumbnail/{id}`, `/photos/preview/{id}`) with LAN IP detection.
- **Desktop UI (`src/` + `src-tauri/`):**
  - Event Bundle Manager (Create w/ presets & scaffolding, Open existing bundle, Gather multiple bundles from a parent NAS/season folder).
  - Card Ingest dashboard (ported & upgraded from `apps/ingest/`, saving reader configs in SQLite and ingesting directly into the active event).
  - Always-visible LAN Server Status Bar (active event counts, manual safety-net Scan, Start/Stop LAN kiosk server + LAN URL).
- **Next to build in `apps/photofinish/`:**
  1. Roster import (`.txt`/`.csv`) & folder-to-subject matching UI (with optional folder rename).
  2. Full touch-first Kiosk Vue SPA (embedded via `rust-embed`) with v3 pricing ladder cart (Single Digital + 3-photo upsell → Full Digital + Prints + +$10 Instant USB) and Square QR / Terminal checkout.
  3. Order desk & native one-click USB-C/A fulfillment (reusing hardware engine) + multi-event consolidated CSV export.
  4. ShootProof Studio API sync module (nested albums, 20–50 worker S3 upload, `zip-bundle`, WHCC/Bay Photo `lab-shipment`).
  5. Filesystem watcher (`notify`) for manual Finder drops + Windows reader discovery (`hardware/windows.rs`) when needed.

### Legacy Reference — `server/` (Unproven Elixir Prototype)
- **Ash Domains & Resources:**
  - `PhotoFinish.Events`: `Event`, `Competitor`, `EventCompetitor`
  - `PhotoFinish.Photos`: `Photo`
  - `PhotoFinish.Orders`: `ProductTemplate`, `EventProduct`, `Order`, `OrderItem` (plus `OrderNumber` sequential `{CODE}-NNNN` generator and transactional `Orders.place_order/1`)
  - `PhotoFinish.Accounts`: `User`, `Token`, `ApiKey` (AshAuthentication scaffolded; admin routes currently use `Plug.BasicAuth`)
- **Ingestion & Organization Pipeline:**
  - `Scanner` (recursive JPEG discovery, skips `_`-prefixed dirs like `_thumbnails`, `_previews`)
  - `PathParser` (extracts flat `gym`, `session`, `group_name`, `apparatus`, `competitor_folder`, `filename`)
  - `CompetitorMatcher` (auto-matches folder names to `EventCompetitor` numbers)
  - `PhotoProcessor` (Oban `:media` worker using `vix`/libvips, concurrency `max(2, schedulers - 2)`, bulk-created in chunks of 5,000, broadcasts on PubSub `"photos:event:#{event_id}"`)
  - `FolderGenerator` (creates Gym/Session folder skeleton from event config)
  - `FolderAssociation` (links unassigned folders to competitors AND physically renames folders on disk)
  - `RosterParser` / `RosterImport` (parses `.txt` roster files with preview UI)
- **Admin LiveViews (`/admin/*`, Basic Auth):**
  - Event CRUD & show page (live photo browser, manual "Scan" button, processing progress bar, per-event product pricing init/editing)
  - Roster import (`/admin/events/:event_id/import-roster`)
  - Folder-to-competitor association (`/admin/events/:event_id/folders`)
  - Product template CRUD (`/admin/products`)
  - Order list & detail (`/admin/events/:event_id/orders`, mark paid with payment reference, notes, mark items fulfilled)
  - Oban Web dashboard (`/admin/oban`)
- **Viewer LiveViews (`/viewer/*`, public):**
  - Multi-event picker (`/viewer`, auto-redirects if only one active event)
  - Event home (`/viewer/:event_id`) with 3-row photo showcase marquee + debounced competitor search (number/first/last name)
  - Competitor gallery (`/viewer/:event_id/competitor/:id`) with thumbnail grid & keyboard-navigable preview lightbox
  - USB order vertical slice (`/viewer/:event_id/competitor/:id/order` — review → checkout → confirmation with order number)
  - Photo serving controllers (`/viewer/photos/thumbnail/:id`, `/viewer/photos/preview/:id`) using `send_file` with 1-year cache headers.

### Built & Working — `apps/ingest/`
- Multi-reader dashboard polling every 3s, merging online readers with offline persisted configs.
- Stable hardware reader identity: USB hardware serial number via `ioreg` (macOS), `"sdxc-builtin"` for internal SD slot, `DeviceTreePath` fallback.
- Per-reader persisted configuration: destination path, photographer initials, camera brand/folder (`DCIM`, etc.), optional folder auto-rename (`105MSDCF` → `Gymnast 05`), optional file auto-rename (`IMG_1234.jpg` → `Gymnast_1234.jpg`), `requireSubfolder` guard.
- Top-level folder listing with per-folder include toggles and non-blocking JPEG copy with throttled Tauri Channel progress events.
- **Limitation:** `discover_card_readers` calls macOS `diskutil` directly and `platform/macos.rs` calls `ioreg` — **does not work on Windows yet**.

### Not Built Yet (Key Gaps for Live Meet Readiness)
1. **Automatic file watcher** — currently requires clicking "Scan" in Admin (no FS watcher GenServer).
2. **Ingest ↔ Server integration (`/api/*`)** — Tauri app only copies files to disk; no HTTP API endpoints exist yet.
3. **Full shopping cart & print ordering** — viewer checkout is currently a single-competitor USB vertical slice (no multi-item cart or per-photo print selection).
4. **Square POS / payment integration** — QR code (`square-commerce-v1://`) and Terminal API are documented in `docs/08-payments.md` & `docs/future/payment-providers.md`, not yet wired into checkout UI.
5. **Automated fulfillment tooling** — no automated USB drive copy tool or print-lab batch ZIP/CSV export (admin currently marks items fulfilled manually).
6. **Viewer scale polish** — no pagination or LiveView streams/PubSub auto-updates on the public viewer gallery yet.
7. **Deployment / multi-location redistribution** — no production release packaging, auto-start appliance config, or cross-platform installer yet.
8. **Future cloud features** — access codes, R2/B2 upload queue, watermarking, online gallery (`docs/future/`).

---

## 5. Key Domain Conventions (Unified App: `apps/photofinish/`)

- **Portable Event Bundle layout** (self-contained, movable between 4 meet servers & home NAS):
  ```text
  <event_folder>/
  ├── _photofinish/
  │   ├── photofinish.db      # SQLite (WAL) for this event (config, subjects, ingest_batches, photos, orders)
  │   ├── backups/            # Automatic pre-migration online SQLite snapshots (photofinish_v{N}_{ts}.db)
  │   ├── thumbnails/         # 320px JPEGs ({photo_id}.jpg) — regenerable cache
  │   └── previews/           # 1280px JPEGs ({photo_id}.jpg) — regenerable cache
  └── <configurable folders>/ # e.g. Gymnastics preset: Gym A / Session 1A / Group 2B / Beam / 1059 Iza Z / IMG_001.jpg
  ```
  - **Relative paths only:** All photo/thumbnail/preview paths in `photofinish.db` are stored **relative to `<event_folder>`** so moving SSDs across machines never breaks paths.
  - **Machine-local DB (`photofinish_app.db` in OS AppData):** Stores event library bookmarks, per-USB-serial card reader configs, and machine settings (port, station code).
  - **Migration discipline:** Embedded forward-only SQL migrations (`crates/photofinish-core/src/db/migrations/`). Upgrading an existing event DB automatically takes an online SQLite snapshot in `_photofinish/backups/` before applying DDL. Opening a DB with `user_version > app_max_version` fails safely (`SchemaTooNew`).
  - **Sport-agnostic & configurable:** Events use configurable presets (`gymnastics`, `dance`, `simple`, `custom`), customizable `subject_label` (Competitor / Athlete / Dancer / Participant), flexible `folder_levels_json` path tagging (never hard-fails on non-standard folder depth), and an `ingest_batches` table preserving exact chronological card-copy order & photographer attribution.
- **IDs:** Prefixed unique strings via `photofinish_core::id`: `evt_`, `sub_`, `pho_`, `bat_`, `prd_`, `ord_`, `itm_`.
- **Money & Tax:** Integer **cents** (`price_cents`, `total_cents`); tax rates in **basis points** (`tax_rate_basis_points`, default `850` = 8.5%).
- **Order numbers:** Include event `order_code` + server `station_code` so multiple servers at the same large event can be merged later with zero collisions.
- **Images:** JPEG only (`.jpg`/`.jpeg`); originals are never mutated; derivatives apply EXIF orientation and output `320px` thumbnails + `1280px` previews.

---

## 6. Architectural Direction & Open Decisions

> **Active Direction (as of Nov 2026): Unified Rust + Tauri + Vue 3 + SQLite App (`apps/photofinish/`) + ShootProof API**
>
> Two realities settled the earlier Elixir vs. Rust question:
> 1. **Up to 4 concurrent meets in a single weekend** require a zero-setup, double-click app that non-developer operators can run without managing Postgres, Docker, or Elixir.
> 2. **No custom cloud web store is needed** — online galleries, digital zip delivery, and WHCC/Bay Photo print drop-shipping will be handled via **ShootProof Unlimited ($600/yr, 0% commission, Studio OpenAPI)** (`docs/future/shootproof-integration.md`).
>
> **Core principles for `apps/photofinish/`:**
> - **Portable Event Bundles:** Each event lives in a self-contained folder with `_photofinish/photofinish.db` and relative paths only, so 4 separate meet servers can be gathered onto one home workstation/NAS on Sunday night cleanly.
> - **Safe schema evolution:** Every schema change is an embedded forward-only migration with automatic pre-migration SQLite online backups in `_photofinish/backups/` and a `SchemaTooNew` guard for mixed-version fleets.
> - **Sport-agnostic by design:** Gymnastics presets work out of the box, without hardcoding gymnastics-only table/column names.
> - **Target priority:** Apple Silicon macOS (`arm64`) first, keeping OS hardware code isolated for future Windows support.
> - Treat `server/` as a **reference prototype** (never proven in production — everything remains negotiable as real-world meets inform the design).

Other open product decisions documented in `docs/future/`:
- Payment provider / Square integration mode: offline QR (`square-commerce-v1://`) + Square Terminal API over hotspot/Starlink, vs. negotiated Square / Stripe / Helcim rates (`docs/08-payments.md`, `docs/future/payment-providers.md`).
- Venue connectivity: default "Monday email link" SLA (zero venue internet needed) + optional 5G hotspot / Starlink Mini for priority paid-order trickle uploads (`docs/future/shootproof-integration.md`).

---

## 7. Domain Documentation Index

| Document | Topic |
| --- | --- |
| `docs/01-architecture.md` | System overview, network topology, performance targets |
| `docs/02-data-model.md` | Database tables, flat photo location model, ordering schema |
| `docs/03-ingestion.md` | Tauri ingestion app features, camera brands, rename patterns |
| `docs/04-viewer.md` | Public kiosk browsing UX, search, lightbox, touch/keyboard specs |
| `docs/05-admin.md` | Admin dashboard, event setup, roster import, photo browser |
| `docs/06-ordering.md` | Cart, checkout, USB folder structure, print batch workflow |
| `docs/07-api.md` | Planned HTTP `/api/*` and WebSocket contracts (not yet implemented) |
| `docs/08-payments.md` | Offline-safe Square POS QR code (`square-commerce-v1://`) workflow |
| `docs/future/shootproof-integration.md` | **Target v3 direction:** ShootProof Unlimited Studio API (online galleries, zip delivery, WHCC/Bay Photo drop-ship), pricing ladder, USB-C/A specs |
| `docs/future/online-gallery.md` | *(Superseded storefront)* Earlier custom access-code gallery + R2/B2 design (minors' privacy & season-view ideas still useful reference) |
| `docs/future/on-site-digital-delivery.md` | *(Superseded upload target)* Earlier digital/USB packaging, watermarking, Starlink/5G analysis |
| `docs/future/payment-providers.md` | Square vs Stripe vs Helcim vs Stax rate analysis at ~$350K/season |
| `docs/future/photographer-qc.md` | Low-priority future idea: side-by-side photographer exposure/color QC |

---

## 8. Quick-Start Dev Commands

### Unified App (`cd apps/photofinish`) — **Primary**
```bash
pnpm install
pnpm build                      # Type-check Vue/TS & build Vite bundle
cargo test --workspace          # Run core Rust tests (DB migrations, presets, ingest, imaging, Axum LAN API)
pnpm tauri dev                  # Launch PhotoFinish desktop app + embedded LAN kiosk server (:4000)
pnpm tauri build                # Build macOS Apple Silicon .app / .dmg bundle
```

### Legacy Reference: Phoenix Server (`cd server`)
```bash
docker compose up -d            # PostgreSQL 18 on localhost:8200
cd server && mix setup && mix phx.server
```

### Legacy Reference: Original Ingest App (`cd apps/ingest`)
```bash
pnpm install && pnpm tauri dev
```
