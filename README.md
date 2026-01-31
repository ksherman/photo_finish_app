# PhotoFinish

Local-network photo management and sales system for live sporting events. Primary use case: youth gymnastics meets with up to 100,000 photos, 1,000+ competitors, and 15 viewing stations.

## Prerequisites

- Elixir 1.15+
- PostgreSQL
- Node.js 18+ and pnpm
- Rust (for Tauri ingest app)

## Phoenix Server

```bash
cd server

# First-time setup
mix setup

# Start the server
mix phx.server
```

The server runs at http://localhost:4000

**Admin interface:** http://localhost:4000/admin/events (requires basic auth - see `ADMIN_USERNAME` and `ADMIN_PASSWORD` env vars, defaults to admin/secret)

**Dev tools:**
- LiveDashboard: http://localhost:4000/dev/dashboard
- Oban Dashboard: http://localhost:4000/oban
- Ash Admin: http://localhost:4000/ash_admin

## Ingest App (Tauri)

```bash
cd apps/ingest

# Install dependencies
pnpm install

# Run in development
pnpm tauri dev

# Build for production
pnpm tauri build
```

## Documentation

See `docs/` for detailed specifications:

- `01-architecture.md` - System overview and deployment
- `02-data-model.md` - Database schema and entities
- `03-ingestion.md` - Tauri app and file processing
- `04-viewer.md` - Public photo browsing UI
- `05-admin.md` - Admin dashboard
- `06-ordering.md` - Shopping cart and checkout
- `07-api.md` - HTTP endpoints and WebSocket channels
