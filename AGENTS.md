# AGENTS.md — PhotoFinish Context Bootstrap

> **Purpose:** Fast-start orientation for future sessions (human or AI). Read this first to understand the business problem, current codebase layout, architecture, conventions, and active open decisions without scanning the whole repo.

---

## 1. What PhotoFinish Is

**PhotoFinish** is a local-network photo ingest, viewing, and on-site sales system for live sporting events (primary use case: **youth gymnastics meets**).

- **Scale per meet:** up to ~100,000–165,000 JPEG photos (~325–700 GB originals, ~35 GB thumbnails/previews), 1,000+ competitors, ~15 concurrent viewing stations (iPads/tablets on local Wi-Fi).
- **Season & multi-venue reality:** ~850,000 photos across ~12 competition weekends, with up to **4 concurrent meets running simultaneously at different venues** on peak state-meet weekends.
- **Business scale:** ~$310K–$350K/season (~$250K+ card-present on-site, ~$60K–$100K online post-event).
- **Legacy replacement:** Replaces an older Windows stack archived in `as-built/` (**Photo Parata** — a self-contained Windows `.exe` with embedded SQLite + Apache/PHP — plus `KioskViewingStation.exe` and `downloader.exe`) and replaces PhotoReflect (~15–18% fee) with **ShootProof Unlimited ($600/yr, 0% commission, Studio API)**.
- **Core meet workflow:**
  `Photographers (SD cards) → Ingest (copy & rename to storage) → Local Server (index, thumbnail/preview, match roster) → Families browse & order on kiosk tablets (Digital / Single Digital / Prints / +$10 Instant USB) → Square checkout → Local USB copy & post-meet ShootProof API upload (galleries, zip downloads, WHCC/Bay Photo drop-ship)`.

---

## 2. Repository Map

The unified Rust + Tauri 2 + Vue 3 app lives directly at the repository root. *(Earlier v1 Elixir/Phoenix `server/` and standalone `apps/ingest/` prototypes are preserved under git tag `archive/v1-prototypes`.)*

| Path | What It Contains | Notes |
| --- | --- | --- |
| `crates/photofinish-core/` | **Core Rust library:** SQLite migrations (`db/`), event bundles (`event/{mod,subjects,folders,photos,usb,tests}.rs`), imaging (`imaging/{mod,macos}.rs`), Apple Vision portraits (`avatars.rs`, `vision_avatar.m`), macOS hardware/USB (`hardware/{mod,macos}.rs`), FSEvents watcher (`watcher.rs`), roster parser (`roster.rs`), orders & shipping (`orders.rs`), embedded Axum LAN kiosk server (`server.rs`), presets (`preset.rs`), shared utilities (`util.rs`) | Cargo workspace member |
| `src-tauri/` | **Tauri 2 desktop shell:** IPC commands (`commands.rs`), app bootstrap & managed state (`lib.rs`), icons & `tauri.conf.json` (`com.photofinish.desktop`) | Cargo workspace member |
| `src/` | **Vue 3 / TS / Pinia / Tailwind v4 UI:** Desktop views (`views/`), modular components (`components/`, `components/orders/`, `components/roster/`), Pinia stores (`stores/`), shared utilities (`utils/{format,subjects,csv}.ts`), and embedded Kiosk Web SPA (`src/kiosk/`) | Desktop + Kiosk share `src/utils/` & `src/types/` |
| `kiosk/` & `public/` | Kiosk HTML entry point & PWA icons/favicons (`vite.kiosk.config.ts` builds into `crates/photofinish-core/kiosk-dist/`) | Embedded via `rust-embed` in release builds |
| `docs/` | `docs/STATUS.md` (active implementation status), `docs/01`–`08` (core domain & workflow specs), and `docs/future/` (**ShootProof Studio API v3 direction** in `shootproof-integration.md`, payment analysis, etc.) | Reference & design specs |
| `as-built/`, `test-images/`, `test-ingest/` | Untracked local legacy Windows binaries/screenshots and sample meet photo/roster fixtures | Gitignored local test fixtures |

---

## 3. Tech Stack

- **Core (`crates/photofinish-core`):** Rust (Cargo workspace), `rusqlite` (bundled SQLite 3 in WAL mode + online pre-migration backups), `axum` 0.8 (embedded LAN kiosk server), macOS native `ImageIO` + `CoreGraphics` hardware JPEG thumbnail/preview pipeline (~170–180 photos/sec on M4 Max) with optimized pure-Rust SIMD fallback (`image` 0.25 + `fast_image_resize` 5, resize-before-EXIF-orient + 1280→320 cascade), macOS Apple Vision portrait avatar engine (`vision_avatar.m`), batched SQLite commit flusher, macOS `diskutil`/`ioreg` USB hardware serial discovery, and macOS FSEvents auto-watcher (`notify`).
- **Desktop & Kiosk (`src-tauri/` + `src/`):** Tauri 2 (`photofinish-desktop`) + Vue 3 (Composition API) / TypeScript / Pinia / Tailwind CSS v4 / `pnpm`.
- **Target OS Priority:** Apple Silicon macOS (`arm64` / `aarch64-apple-darwin`) first; OS-specific hardware & imaging code isolated in `hardware/macos.rs` and `imaging/macos.rs` for later Windows support.

---

## 4. Ground-Truth Implementation Status

*(Mirrors `docs/STATUS.md`, updated 2026-11-24. Verified: **28/28** `photofinish-core` tests passing, Clippy `-D warnings` clean, `pnpm build` clean, release macOS `.app` / `.dmg` verified under `com.photofinish.desktop`.)*

### Built & Working
- **Core (`crates/photofinish-core`):**
  - Portable Event Bundle storage (`<event>/_photofinish/photofinish.db` in SQLite WAL mode, relative-only paths) + machine-local `photofinish_app.db` (library bookmarks, hardware-locked reader configs w/ persistent groups/labels/sort order via `0002_app_reader_layout.sql`, settings).
  - Forward-only embedded SQLite migration runner with automatic online pre-migration backups (`_photofinish/backups/`) and newer-schema protection (`SchemaTooNew`).
  - Sport-agnostic event presets (`gymnastics`, `dance`, `simple`, `custom`), customizable `subject_label`, configurable folder scaffolding, flexible path-to-tags parser, and chronological `ingest_batches` audit trail.
  - Hardware layer (`hardware/macos.rs`): `diskutil` + `ioreg` USB hardware serial discovery (with duplicate-serial `DeviceTreePath` disambiguation, virtual/disk-image exclusion for `.dmg`/`Docker`, and `diskutil eject`), external USB drive discovery (with free-space & `DCIM/` camera-card guard), non-blocking card copy with folder toggles & auto-rename, and streaming high-speed USB order copy.
  - Hybrid JPEG thumbnail (320px) + preview (1280px) pipeline (`imaging/`): macOS native `ImageIO` hardware DCT shrink-on-load + CoreGraphics cascade (~180 photos/sec on M4 Max, verified on 146,830 real photos) with optimized pure-Rust SIMD fallback, dedicated worker pool, batched SQLite commits (64/txn), live throughput/ETA telemetry, plus **native macOS Apple Vision portrait avatar generation** (`avatars.rs` + `vision_avatar.m` → `256×256` face+leotard crops in `_photofinish/avatars/`, served on `/photos/avatar/{subject_id}` with automatic thumbnail fallback).
  - Multi-file roster import (`.txt` / `.csv` w/ filename-inferred groups, idempotent upsert, automatic photo matching), fast `+status = 'ready'` indexed folder browsing (`list_folders` w/ inferred Same Group, Same Session & cross-apparatus reference thumbnails), atomic two-phase swap-safe batch rotation renaming (`apply_folder_reassignments`), and 3-mode safe folder removal (`remove_folder`: `_photofinish/trash`, permanent delete, DB-only).
  - Complete Order Desk engine (`orders.rs` + `event/usb.rs`): live stats, payment reconciliation, per-item fulfillment, untaxed print S&H tiers (`$3` ≤ 8×10, `$10` 11×14+, stored via `0002_event_shipping.sql`), USB copy plan resolution (`<Subject>/<Apparatus>/<filename>`), per-event product catalog & tax/code updates, and RFC-4180 single/multi-event CSV export (including `order_shipping`).
  - Embedded `axum` 0.8 LAN kiosk HTTP server (`rust-embed` Vue 3 SPA + `/api/status`, `/api/kiosk/showcase` w/ O(k log N) rowid sampling, `/api/subjects` w/ numeric ranking, `/api/subjects/{id}` w/ sport-agnostic `section_label` grouping, `/api/products`, `POST /api/orders` w/ offline `square-commerce-v1://` QR payload, `/photos/thumbnail/{id}`, `/photos/preview/{id}`, `/photos/avatar/{subject_id}`) with LAN IP detection.
- **Kiosk Web SPA (`src/kiosk/` → embedded in `crates/photofinish-core/kiosk-dist/`):**
  - Touch-first Vue 3 + Tailwind v4 SPA served directly from the embedded Axum server on `:4000` (live disk-reload in debug mode, `rust-embed` in release builds), including custom favicons, Apple Touch Icons, and `manifest.webmanifest` for iPad standalone Home Screen mode.
  - Home screen with calm 3-row ambient photo showcase marquee (`280s`/`800s`/`340s`) + 0ms instant client-side search (exact bib # → numeric bib prefix → name ranking + session filter).
  - Dedicated Subject Gallery page (`/subject/:id`) with sport-agnostic apparatus/section tabs & grouped sections (e.g. `All · Beam · Floor`), lazy-loaded thumbnail grid, and full-screen touch-swipe/keyboard lightbox with adjacent preview preloading.
  - v3 Pricing Ladder Cart (`/cart`): Single Digital ($25) with automatic **3-photo tipping-point upgrade prompt** → Full Digital ($85) + Prints (5×7–16×20) + **+$10 Instant Dual USB-C/A** add-on toggle + automatic untaxed print S&H (`$3` / `$10`).
  - Checkout (`/checkout`) with conditional shipping address for direct-to-home print drop-shipping + Order Confirmation (`/order/:orderNumber`) with Retina PNG Square POS QR code (`qrcode`) and subtotal/S&H/tax breakdown.
- **Desktop UI (`src/` + `src-tauri/`):**
  - Custom PhotoFinish icon branding (`BrandMark.vue`, native `.icns`/`.ico`/PNGs) and `additionalWatchFolders` in `tauri.conf.json` so `pnpm tauri dev` auto-rebuilds on `photofinish-core` edits.
  - **Events:** Create w/ presets & scaffolding (quietly remembering per-machine Station Code default), Open existing bundle, recursive multi-station Gather from nested NAS/season folders, read-only consolidated library overview (photos/orders/revenue, duplicate order-# collision detection, `SchemaTooNew` indicator), and one-click consolidated CSV export.
  - **Card Ingest:** Wide (`≥340px`) reader cards organized into persistent pointer-drag-and-drop groups (`Gym A: Floor → Beam`) & custom labels locked to hardware `reader_id`, Enter-to-ingest for barcode scanners, per-card `⏏ Eject`, `Auto-eject after ingest`, `⏏ Eject All`, direct `Name Folders →` handoff, live `CardNamingQueueStrip`, and `Ingest Folder…` for local/thumb-drive drops.
  - **Roster & Folders:** Multi-file import preview (`RosterPanel`), searchable roster table, live **Ingested Card Queue** (`ingest_batch_id`-scoped card naming w/ live-streaming thumbnails + `Next Card →` handoff), Unassigned & All Folders rotation manager with Same Group / Same Session / All Roster scoping, strict numeric bib-order roster with in-place grey-out of athletes already matched in the current batch or on the same apparatus in the session (+ `Hide matched` toggle), cross-apparatus reference thumbnails, click-to-zoom lightbox, staged Preview & Confirm modal, and 3-mode Remove Folder modal (`FolderModals`).
  - **Orders & USB:** Auto-refreshing Order Desk (5s poll), KPI cards, Square POS QR modal, payment reconciliation, one-click native USB-C/A fulfillment (`OrderUsbFulfillmentPanel`) with live MB/s progress + `DCIM/` guard + auto-eject, live Pricing & Tax catalog editor (`PricingCatalogPanel`), and single/multi-event CSV export.
  - **Status Bar:** Active event counts, live processing progress bar + `photos/sec` + ETA, default-on macOS FSEvents **Auto-Watch** toggle (debounced targeted Finder drop/rename/orphan cleanup + live sync badges), full-event two-way **Scan Folder**, and Start/Stop LAN kiosk server + LAN URL.
- **Next to build:**
  1. Real-meet UX polish (Roster & Folders ergonomics, live iPad LAN & hardware dry-run).
  2. ShootProof Studio API sync module (`ShootProof Sync` tab: OAuth 2.0, nested albums, 20–50 worker S3 upload, `zip-bundle` digital delivery for Square-paid orders, WHCC/Bay Photo `lab-shipment`) — on hold pending service/account verification.
  3. Optional Apple Developer ID code signing / notarization & Windows hardware layer (`hardware/windows.rs`) when needed.

---

## 5. Key Domain Conventions

- **Portable Event Bundle layout** (self-contained, movable between 4 meet servers & home NAS):
  ```text
  <event_folder>/
  ├── _photofinish/
  │   ├── photofinish.db      # SQLite (WAL) for this event (config, subjects, ingest_batches, photos, orders)
  │   ├── backups/            # Automatic pre-migration online SQLite snapshots (photofinish_v{N}_{ts}.db)
  │   ├── trash/              # Reversible folder removals from Roster & Folders (never re-scanned)
  │   ├── avatars/            # 256px Apple Vision portrait avatars ({subject_id}.jpg) — regenerable cache
  │   ├── thumbnails/         # 320px JPEGs ({photo_id}.jpg) — regenerable cache
  │   └── previews/           # 1280px JPEGs ({photo_id}.jpg) — regenerable cache
  └── <configurable folders>/ # e.g. Gymnastics preset: Gym A / Session 1A / Group 2B / Beam / 1059 Iza Z / IMG_001.jpg
  ```
  - **Relative paths only:** All photo/thumbnail/preview paths in `photofinish.db` are stored **relative to `<event_folder>`** so moving SSDs across machines never breaks paths.
  - **Machine-local DB (`photofinish_app.db` in OS AppData):** Stores event library bookmarks, per-USB-serial card reader configs, and machine settings (port, station code).
  - **Migration discipline:** Embedded forward-only SQL migrations (`crates/photofinish-core/src/db/migrations/`). Upgrading an existing event DB automatically takes an online SQLite snapshot in `_photofinish/backups/` before applying DDL. Opening a DB with `user_version > app_max_version` fails safely (`SchemaTooNew`).
  - **Sport-agnostic & configurable:** Events use configurable presets (`gymnastics`, `dance`, `simple`, `custom`), customizable `subject_label` (Competitor / Athlete / Dancer / Participant), flexible `folder_levels_json` path tagging (never hard-fails on non-standard folder depth), and an `ingest_batches` table preserving exact chronological card-copy order & photographer attribution.
- **IDs:** Prefixed unique strings via `photofinish_core::id`: `evt_`, `sub_`, `pho_`, `bat_`, `prd_`, `ord_`, `itm_`.
- **Money & Tax:** Integer **cents** (`price_cents`, `total_cents`, `shipping_cents`); tax rates in **basis points** (`tax_rate_basis_points`, default `850` = 8.5%). Print shipping (`shipping_cents`) is untaxed.
- **Order numbers:** Include event `order_code` + server `station_code` so multiple servers at the same large event can be merged later with zero collisions.
- **Images:** JPEG only (`.jpg`/`.jpeg`); originals are never mutated; derivatives apply EXIF orientation and output `320px` thumbnails + `1280px` previews.

---

## 6. Architectural Direction & Open Decisions

- **Portable Event Bundles:** Each event lives in a self-contained folder with `_photofinish/photofinish.db` and relative paths only, so 4 separate meet servers can be gathered onto one home workstation/NAS on Sunday night cleanly.
- **Online fulfillment via ShootProof API:** Online galleries, digital zip delivery, and WHCC/Bay Photo print drop-shipping will be handled via **ShootProof Unlimited ($600/yr, 0% commission, Studio OpenAPI)** (`docs/future/shootproof-integration.md`).
- **Safe schema evolution:** Every schema change is an embedded forward-only migration with automatic pre-migration SQLite online backups in `_photofinish/backups/` and a `SchemaTooNew` guard for mixed-version fleets.
- **Open product decisions (`docs/future/`):**
  - Payment provider / Square integration mode: offline QR (`square-commerce-v1://`) + Square Terminal API over hotspot/Starlink, vs. negotiated Square / Stripe / Helcim rates (`docs/08-payments.md`, `docs/future/payment-providers.md`).
  - Venue connectivity: default "Monday email link" SLA (zero venue internet needed) + optional 5G hotspot / Starlink Mini for priority paid-order trickle uploads (`docs/future/shootproof-integration.md`).

---

## 7. Domain Documentation Index

| Document | Topic |
| --- | --- |
| `docs/STATUS.md` | Up-to-date implementation status across all modules |
| `docs/01-architecture.md` | System overview, network topology, performance targets |
| `docs/02-data-model.md` | Database tables, flat photo location model, ordering schema |
| `docs/03-ingestion.md` | Ingest workflow, camera brands, rename patterns |
| `docs/04-viewer.md` | Public kiosk browsing UX, search, lightbox, touch/keyboard specs |
| `docs/05-admin.md` | Admin dashboard, event setup, roster import, photo browser |
| `docs/06-ordering.md` | Cart, checkout, USB folder structure, print batch workflow |
| `docs/07-api.md` | Planned HTTP `/api/*` and WebSocket contracts |
| `docs/08-payments.md` | Offline-safe Square POS QR code (`square-commerce-v1://`) workflow |
| `docs/future/shootproof-integration.md` | **Target v3 direction:** ShootProof Unlimited Studio API (online galleries, zip delivery, WHCC/Bay Photo drop-ship), pricing ladder, USB-C/A specs |
| `docs/future/online-gallery.md` | *(Superseded storefront)* Earlier custom access-code gallery + R2/B2 design (minors' privacy & season-view ideas still useful reference) |
| `docs/future/on-site-digital-delivery.md` | *(Superseded upload target)* Earlier digital/USB packaging, watermarking, Starlink/5G analysis |
| `docs/future/payment-providers.md` | Square vs Stripe vs Helcim vs Stax rate analysis at ~$350K/season |
| `docs/future/photographer-qc.md` | Low-priority future idea: side-by-side photographer exposure/color QC |

---

## 8. Quick-Start Dev Commands

```bash
pnpm install
pnpm build                      # Type-check Vue/TS & build desktop + kiosk bundles
cargo test --workspace          # Run core Rust tests (DB migrations, presets, ingest, imaging, Axum LAN API)
cargo clippy --workspace --all-targets -- -D warnings
pnpm tauri dev                  # Launch PhotoFinish desktop app + embedded LAN kiosk server (:4000)
pnpm tauri build                # Build macOS Apple Silicon .app / .dmg bundle
```
