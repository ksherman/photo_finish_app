# PhotoFinish

Local-network photo ingest, viewing kiosk, and on-site sales system for live sporting events (primary use case: youth gymnastics meets with 100,000–165,000+ photos, 1,000+ competitors, and ~15 concurrent viewing tablets on local Wi-Fi — with zero venue internet required).

## Active App (`apps/photofinish/`)

PhotoFinish is a single, self-contained **Rust + Tauri 2 + Vue 3 + TypeScript + SQLite** desktop application with an embedded **Axum** LAN kiosk server and native Apple Silicon hardware JPEG thumbnailing (~180 photos/sec on M4 Max).

- **Portable Event Bundles:** Every meet lives in a self-contained folder (`<event>/_photofinish/photofinish.db`, relative paths only) so up to 4 concurrent weekend meet stations can be gathered cleanly onto one workstation/NAS on Sunday night.
- **Built-in Modules:**
  - **Events Bundle Manager:** Create with sport-agnostic presets (`gymnastics`, `dance`, `simple`, `custom`), Open existing bundles, or Gather multiple bundles from a season/NAS folder.
  - **Card Ingest:** Hardware-serial-locked USB/SD card readers organized into persistent drag-and-drop groups (e.g., `Gym A: Floor → Beam`), folder toggles, auto-rename, Enter-to-ingest for barcode scanners, and one-click / auto-eject (`diskutil eject`).
  - **Roster & Folders:** Multi-file `.txt`/`.csv` roster import with automatic photo matching, safe folder removal (`_photofinish/trash`, permanent delete, or DB-only), and swap-safe batch rotation renaming with preview & confirm.
  - **Embedded LAN Kiosk (`:4000`):** Touch-first Vue 3 PWA (Add-to-Home-Screen ready for iPads) with ambient showcase marquee, instant 0ms numeric search, apparatus/section gallery tabs, swipe lightbox, v3 pricing ladder cart with 3-photo tipping-point upgrade prompt, and offline Square POS QR checkout.
  - **Orders & Instant USB-C/A Fulfillment:** Live auto-refreshing Order Desk, Square POS QR & payment reconciliation, native external USB drive detection (with `DCIM/` safety warning), high-speed streaming copy + auto-eject, per-event pricing/tax catalog editor, and single or multi-event consolidated CSV export.

### Prerequisites (Development)

- Apple Silicon macOS (primary target)
- Rust toolchain (`rustc` / `cargo`)
- Node.js 22+ and `pnpm`

### Quick Start

```bash
cd apps/photofinish
pnpm install
pnpm tauri dev          # Launch desktop app + embedded LAN kiosk server (:4000)
```

### Testing & Release Packaging

```bash
cd apps/photofinish
pnpm build              # Type-check Vue/TS and build desktop + kiosk bundles
cargo test --workspace  # Run Rust core test suite
pnpm tauri build        # Build macOS .app (~19 MiB) & .dmg (~7 MiB) release bundles
```

## Documentation

- [AGENTS.md](AGENTS.md) — Current architecture, repository map, domain conventions, and engineering context
- [docs/STATUS.md](docs/STATUS.md) — Up-to-date implementation status across modules
- [docs/future/shootproof-integration.md](docs/future/shootproof-integration.md) — Post-meet online gallery, automated digital `zip-bundle`, and WHCC/Bay Photo drop-ship architecture via ShootProof Studio API
- [docs/01-architecture.md](docs/01-architecture.md) – [docs/08-payments.md](docs/08-payments.md) — Domain & workflow specifications

## Legacy Reference Prototypes

`server/` (Elixir/Phoenix + Ash + PostgreSQL) and `apps/ingest/` (standalone Tauri ingest prototype) remain in the repository for reference only; active development lives in `apps/photofinish/`.
