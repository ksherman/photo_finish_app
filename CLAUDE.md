# CLAUDE.md - PhotoFinish

## What This Is

PhotoFinish is a local-network photo management and sales system for live sporting events. Primary use case: youth gymnastics meets with up to 100,000 photos, 1,000+ competitors, and 15 viewing stations.

**Core workflow:** Photographers capture photos → Tauri app copies to NAS → Phoenix processes & serves → Families browse at kiosks → Orders placed and fulfilled on-site.

## Tech Stack

- **Server:** Elixir/Phoenix 1.8+, PostgreSQL, Oban, libvips (via `vix`)
- **Frontend:** Phoenix LiveView + Tailwind CSS
- **Ingestion App:** Tauri (Rust + VueJS 3)
- **Deployment:** Local network, Mac server, NAS storage

## Domain Documentation

Reference these docs for detailed specs:

| Doc                       | Use When                                                  |
| ------------------------- | --------------------------------------------------------- |
| `server/AGENTS.md`        | Documentation for elixir application                      |
| `docs/01-architecture.md` | System overview, deployment, infrastructure decisions     |
| `docs/02-data-model.md`   | Database schema, entities, migrations, key queries        |
| `docs/03-ingestion.md`    | Tauri app, file watcher, image processing pipeline        |
| `docs/04-viewer.md`       | Public browsing UI, navigation, search, real-time updates |
| `docs/05-admin.md`        | Admin dashboard, roster management, photo organization    |
| `docs/06-ordering.md`     | Shopping cart, checkout, payment, fulfillment workflows   |
| `docs/07-api.md`          | HTTP endpoints, WebSocket channels                        |

## Key Conventions

- **Money:** Store as cents (`price_cents`, `total_cents`)
- **IDs:** UUIDs for all primary keys
- **Images:** JPEG only, three versions (original, preview 1280px, thumbnail 320px)
- **Contexts:** `Events`, `Photos`, `Orders`, `Ingestion`

## Commands

```bash
mix phx.server          # Run server
mix test                # Run tests
mix ecto.migrate        # Run migrations
```