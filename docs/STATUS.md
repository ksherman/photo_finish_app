# PhotoFinish — Implementation Status

**Last updated:** 2026-11-23  
**Active codebase:** `apps/photofinish/` (Unified Rust + Tauri 2 + Vue 3 + TypeScript + SQLite + Axum)  
**Verified test suite:** `cargo test --workspace` (18/18 passing), `pnpm build` (clean), `pnpm tauri build` (`PhotoFinish.app` 18.7 MiB / `.dmg` 7.0 MiB)

---

## Built & Working (`apps/photofinish/`)

### 1. Portable Storage & Safe Migrations (`crates/photofinish-core/src/db/`)
- **Portable Event Bundle (`<event>/_photofinish/photofinish.db`):** SQLite WAL mode, relative-only photo/derivative paths, forward-only embedded migrations (`0001_event_initial.sql`), automatic pre-migration online backups (`_photofinish/backups/`), and `SchemaTooNew` mixed-version protection.
- **Machine-Local App DB (`photofinish_app.db`):** Library bookmarks, machine settings, and hardware-locked USB card reader configs + persistent layout/groups (`0001_app_initial.sql`, `0002_app_reader_layout.sql`).

### 2. Event Bundle Manager (`EventsView.vue` + `event.rs` + `preset.rs`)
- Create events with sport-agnostic presets (`gymnastics`, `dance`, `simple`, `custom`), customizable `subject_label`, and optional folder scaffolding.
- Open existing event bundles or **Gather** multiple weekend event bundles from a shared parent / NAS directory.
- Always-visible **LAN Server Status Bar** (`ServerStatusBar.vue`) with photo counts, live thumbnail throughput (`photos/sec`) + ETA, manual safety-net Scan, and Start/Stop LAN Kiosk controls.

### 3. Hardware Card Ingest (`IngestView.vue`, `ReaderDashboardCard.vue`, `ReaderSetupModal.vue`, `hardware/`)
- macOS `diskutil` + `ioreg` USB hardware serial discovery (with `sdxc-builtin` support, duplicate-serial `DeviceTreePath` disambiguation, and automatic exclusion of virtual disks / `.dmg` images like `Docker`).
- Persistent **drag-and-drop reader groups & left-to-right ordering** (e.g., `Gym A: Floor → Beam`) + custom reader labels locked to hardware `reader_id` (slots remain pinned when cards are ejected).
- Per-folder copy toggles, folder/file auto-rename, Enter-to-ingest for USB barcode scanners, non-blocking copy progress via Tauri Channels, chronological `ingest_batches` audit log, per-card `⏏ Eject`, `Auto-eject after ingest`, and `⏏ Eject All`.

### 4. High-Speed Thumbnail & Preview Engine (`imaging.rs`)
- macOS native `ImageIO` hardware DCT shrink-on-load + CoreGraphics cascade (`1280px` preview → `320px` thumbnail) achieving **~170–180 photos/sec** on M4 Max (verified across 146,830 real 24MP meet photos with 0 errors), with an optimized pure-Rust SIMD fallback (`image` + `fast_image_resize`).
- Dedicated worker pool + 64-photo batched SQLite commit flusher + live telemetry.

### 5. Roster Import & Rotation Folder Management (`RosterAndFoldersView.vue`, `roster.rs`, `event.rs`)
- Multi-file `.txt` and `.csv` roster import with filename-inferred session/group labels, idempotent upsert, and instant automatic photo matching (verified 99.995% auto-match on a 146,830-photo meet).
- **Unassigned Only** and **All Folders (Fix Rotations)** browsing with multi-word location filtering.
- Smart scope picker (`Same Group` inferred from sibling apparatuses → `Same Session` → `All Roster` for late/out-of-group athletes) with cross-apparatus reference thumbnails and click-to-zoom preview lightbox.
- **Swap-safe batch rotation renaming:** Stage multiple folder changes → Preview & Confirm modal (`Swapped` / `Renamed` / `Merged` badges) → atomic two-phase disk staging + two-phase SQLite transaction (zero photo mixing on cyclic shifts, collision-safe merges).
- **Safe folder removal:** Confirmation modal with thumbnails supporting **Move to `_photofinish/trash`** (default, reversible, never re-scanned), **Permanent Delete from Disk**, or **Remove from Database Only**.

### 6. Embedded LAN Kiosk Web SPA (`src/kiosk/` → `crates/photofinish-core/kiosk-dist/`, `server.rs`)
- Touch-first Vue 3 + Tailwind v4 SPA embedded in the release binary via `rust-embed` (with live disk reload in debug builds) + PWA `manifest.webmanifest` and Apple Touch Icons for iPad Home Screen kiosks.
- Home screen with 3-row calm ambient photo showcase marquee (O(k log N) random `rowid` sampling) and 0ms client-side numeric-ranked search (`exact bib → bib prefix → name prefix → substring`).
- Subject Gallery (`/subject/:id`) with sport-agnostic apparatus/section tabs (`All · Beam · Floor`), lazy thumbnail grid, and touch-swipe/keyboard preview lightbox with neighbor preloading.
- v3 Pricing Ladder Cart (`/cart`): Single Digital ($25) with automatic **3-photo tipping-point upgrade prompt** → Full Digital ($85) + Prints (5×7–16×20) + **+$10 Instant Dual USB-C/A** add-on.
- Checkout (`/checkout`) with conditional shipping address for print drop-shipping + Confirmation (`/order/:orderNumber`) with offline Square POS QR (`square-commerce-v1://`).

### 7. Orders Desk & Native Instant USB-C/A Fulfillment (`OrdersView.vue`, `orders.rs`, `hardware/`)
- Auto-refreshing Order Desk (polls every 5s) with clickable KPI cards, queue filters, search, Square POS QR modal, payment reconciliation (`Square QR`, `Square Terminal`, `Cash`, `Check`, `Other`, `Refunded`), staff notes, and per-item fulfillment toggles.
- Native external USB drive discovery with free-space checks, `DCIM/` camera-card safety warnings, `<Subject>/<Apparatus>/<filename>` plan preview, streaming copy with live `MB/s` progress bar, auto-fulfill USB items, and auto-eject (`diskutil eject`).
- Per-event **Pricing & Tax** editor (order code, station code, sales tax rate, product prices/availability, add custom products — live on kiosks immediately).
- Single-event and **multi-event consolidated CSV export** across all gathered Event Bundles in the Library.

### 8. Branding & Release Packaging
- Full PhotoFinish brand icons (`icon.icns`, `icon.ico`, PNGs, favicons, Apple Touch Icon, `BrandMark.vue`).
- Standalone Apple Silicon release bundle (`PhotoFinish.app` 18.7 MiB, `PhotoFinish_0.1.0_aarch64.dmg` 7.0 MiB).

---

## Remaining / Next Up

1. **ShootProof Studio API Sync (`ShootProof Sync` tab):** OAuth 2.0 (`studio` scope), Event → Area → Subject nested album creation, 20–50 worker S3 upload, automated `zip-bundle` digital delivery for on-site Square-paid orders (both single-photo `photoIds` and full-album), and WHCC/Bay Photo `lab-shipment` print drop-ship (`docs/future/shootproof-integration.md`).
2. **Roster & Folders UX Polish:** Refine ergonomics based on live-meet practice runs.
3. **Optional Automatic Filesystem Watcher (`notify`):** Auto-index manual Finder drops without clicking Scan.
4. **Optional Multi-Card Copy Yield:** Auto-pause thumbnail workers while multiple SD cards are actively copying if real multi-card testing shows USB bus contention.
5. **Bundle Identifier & Code Signing:** Switch Tauri bundle identifier from `com.photofinish.app` (e.g. to `com.photofinish.desktop`) before multi-machine deployment, plus Apple Developer ID notarization if desired.
6. **Windows Hardware Layer (`hardware/windows.rs`):** If/when Windows meet servers are supported alongside Apple Silicon Macs.

---

## Legacy Reference Prototypes (Archived in Repo)

- `server/` (Elixir/Phoenix 1.8 + Ash 3 + PostgreSQL 18 + Oban + `vix`) and `apps/ingest/` (standalone Tauri prototype) remain untouched as historical references and are superseded by `apps/photofinish/`.
