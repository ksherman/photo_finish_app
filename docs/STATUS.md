# PhotoFinish — Implementation Status

**Last updated:** 2026-11-23  
**Active codebase:** `apps/photofinish/` (Unified Rust + Tauri 2 + Vue 3 + TypeScript + SQLite + Axum)  
**Verified test suite:** `cargo test --workspace` (25/25 passing), `pnpm build` (clean), `pnpm tauri build` (`com.photofinish.desktop`, `.app` / `.dmg` verified)

---

## Built & Working (`apps/photofinish/`)

### 1. Portable Storage & Safe Migrations (`crates/photofinish-core/src/db/`)
- **Portable Event Bundle (`<event>/_photofinish/photofinish.db`):** SQLite WAL mode, relative-only photo/derivative paths, forward-only embedded migrations (`0001_event_initial.sql`), automatic pre-migration online backups (`_photofinish/backups/`), and `SchemaTooNew` mixed-version protection.
- **Machine-Local App DB (`photofinish_app.db`):** Library bookmarks, machine settings, and hardware-locked USB card reader configs + persistent layout/groups (`0001_app_initial.sql`, `0002_app_reader_layout.sql`).

### 2. Event Bundle Manager & Multi-Station Gathering (`EventsView.vue` + `event.rs` + `preset.rs`)
- Create events with sport-agnostic presets (`gymnastics`, `dance`, `simple`, `custom`), customizable `subject_label`, optional folder scaffolding, and remembered per-machine `station_code` default (`A` by default, auto-saved if changed on a secondary server).
- Open existing event bundles or recursively **Gather** nested weekend event bundles from a shared parent / NAS directory (up to 6 levels deep, stopping immediately at bundle boundaries so photo folders are never walked).
- Read-only **Consolidated Multi-Station Overview** across all gathered bundles (live station badges, photo/roster/order counts, combined paid revenue, duplicate order-number collision detection, `SchemaTooNew` indicators, and one-click consolidated CSV export).
- **Auto-Watch (`watcher.rs` / macOS FSEvents) & Two-Way Reconcile:** Enabled by default whenever an event bundle is open — coalesces Finder drops, folder renames, and deleted folders with a 1.5s quiet debounce, reconciles only the affected subfolder(s), auto-prunes orphaned DB rows + cached `_photofinish/` thumbnails/previews (preserving order history via `ON DELETE SET NULL`), pauses automatically during internal Card Ingest & Rotation Renaming, and surfaces live `Auto-Watch` toggle + `+X new · Y removed` sync badges in `ServerStatusBar.vue`. Manual **Scan Folder** runs a full-event two-way reconcile to catch changes made while PhotoFinish was closed.

### 3. Hardware Card Ingest (`IngestView.vue`, `ReaderDashboardCard.vue`, `ReaderSetupModal.vue`, `hardware/`)
- macOS `diskutil` + `ioreg` USB hardware serial discovery (with `sdxc-builtin` support, duplicate-serial `DeviceTreePath` disambiguation, and automatic exclusion of virtual disks / `.dmg` images like `Docker`).
- Persistent **drag-and-drop reader groups & left-to-right ordering** (e.g., `Gym A: Floor → Beam`) + custom reader labels locked to hardware `reader_id` (slots remain pinned when cards are ejected).
- Per-folder copy toggles, folder/file auto-rename, Enter-to-ingest for USB barcode scanners, non-blocking copy progress via Tauri Channels, chronological `ingest_batches` audit log, per-card `⏏ Eject`, `Auto-eject after ingest`, and `⏏ Eject All`.

### 4. High-Speed Thumbnail & Preview Engine (`imaging.rs`)
- macOS native `ImageIO` hardware DCT shrink-on-load + CoreGraphics cascade (`1280px` preview → `320px` thumbnail) achieving **~170–180 photos/sec** on M4 Max (verified across 146,830 real 24MP meet photos with 0 errors), with an optimized pure-Rust SIMD fallback (`image` + `fast_image_resize`).
- Dedicated worker pool + 64-photo batched SQLite commit flusher + live telemetry.

### 5. Roster Import & Rotation Folder Management (`RosterAndFoldersView.vue`, `roster.rs`, `event.rs`)
- Multi-file `.txt` and `.csv` roster import with filename-inferred session/group labels, idempotent upsert, and instant automatic photo matching (verified 99.995% auto-match on a 146,830-photo meet).
- **Live Card Naming Queue (`ingest_batches` bridge):** Finished cards show a direct `Name Folders →` button and appear in a shared live Card Queue strip across `Card Ingest` and `Roster & Folders` (showing actively downloading cards `%` alongside finished cards waiting to be named, scoped strictly to that card's `ingest_batch_id` with live-streaming thumbnails and a 1-click `Next Card →` handoff when cleared). Also includes an `Ingest Folder…` modal for local folder/thumb-drive drops and multi-card testing.
- **Unassigned Only** and **All Folders (Fix Rotations)** browsing with multi-word location filtering.
- Smart scope picker (`Same Group` inferred from sibling apparatuses → `Same Session` → `All Roster` for late/out-of-group athletes) with cross-apparatus reference thumbnails, click-to-zoom preview lightbox, and **progressive session narrowing** (athletes already staged, matched in this rotation, or already photographed on the same apparatus/section in an earlier rotation of the session are automatically greyed out and sunk below remaining athletes, with an optional `Hide matched` toggle).
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
- Finalized macOS bundle identifier (`com.photofinish.desktop`) with automatic WAL-safe one-time migration from legacy `com.photofinish.app` app data.
- Standalone Apple Silicon release bundle (`PhotoFinish.app` + `PhotoFinish_0.1.0_aarch64.dmg`).

---

## Remaining / Next Up

1. **ShootProof Studio API Sync (`ShootProof Sync` tab):** OAuth 2.0 (`studio` scope), Event → Area → Subject nested album creation, 20–50 worker S3 upload, automated `zip-bundle` digital delivery for on-site Square-paid orders (both single-photo `photoIds` and full-album), and WHCC/Bay Photo `lab-shipment` print drop-ship (`docs/future/shootproof-integration.md`). *(On hold pending account/service verification.)*
2. **Roster & Folders UX Polish:** Refine ergonomics based on live-meet practice runs.
3. **Optional Multi-Card Copy Yield:** Auto-pause thumbnail workers while multiple SD cards are actively copying if real multi-card testing shows USB bus contention.
4. **Optional Apple Developer ID Code Signing & Notarization:** Sign & notarize the `.dmg` for zero-warning Gatekeeper install across meet Macs once an Apple Developer ID is configured.
5. **Windows Hardware Layer (`hardware/windows.rs`):** If/when Windows meet servers are supported alongside Apple Silicon Macs.

---

## Legacy Reference Prototypes (Archived in Repo)

- `server/` (Elixir/Phoenix 1.8 + Ash 3 + PostgreSQL 18 + Oban + `vix`) and `apps/ingest/` (standalone Tauri prototype) remain untouched as historical references and are superseded by `apps/photofinish/`.
