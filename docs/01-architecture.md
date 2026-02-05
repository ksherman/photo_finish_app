# PhotoFinish - Architecture Overview

**Version:** 1.0  
**Date:** November 29, 2025  
**Status:** Ready for Development

---

## System Overview

PhotoFinish is a local-network photo management and sales system for live sporting events. Primary use case: youth gymnastics meets with up to 100,000 photos, 1,000+ competitors, and 15 concurrent viewing stations.

### Key Differentiators

- **Local network operation** — no internet dependency
- **Real-time photo availability** — photos viewable within seconds of copy to NAS
- **Integrated ordering** — on-site sales with USB and print fulfillment

---

## System Components

```
┌─────────────────────────────────────────────────┐
│      Tauri Ingestion App (Rust + Vue)           │
│      - Memory card reading                      │
│      - Barcode scanner input                    │
│      - File copying to NAS                      │
│      - Folder renaming                          │
└────────────────┬────────────────────────────────┘
                 │ HTTP API
                 ▼
┌────────────────────────────────────────────────┐
│              NAS / File Server                  │
│  /originals/   (source files)                  │
│  /previews/    (1280px, watermarked)           │
│  /thumbnails/  (320px)                         │
│  Optional: nginx on :8080                      │
└───────────────┬─────────────────────────────────┘
                │ mounted or HTTP
                ▼
┌────────────────────────────────────────────────┐
│           Phoenix Application Server            │
│  - Ingestion API (Tauri communication)         │
│  - File Watcher (GenServer)                    │
│  - Processing Pipeline (Oban)                  │
│  - Admin Interface (LiveView)                  │
│  - Viewer Interface (LiveView)                 │
│  - Ordering System                             │
│  - PostgreSQL Database                         │
│  Port: 4000                                    │
└───────────────┬─────────────────────────────────┘
                │
        ┌───────┴────────┐
   Viewer Stations    Admin Station
   (tablets/kiosks)   (laptop)
```

---

## Technology Stack

| Component | Technology | Rationale |
|-----------|------------|-----------|
| **Ingestion App** | Tauri (Rust + VueJS 3) | Native USB/filesystem access |
| **Server** | Elixir/Phoenix 1.7+ | Real-time, fault-tolerant, familiar |
| **Database** | PostgreSQL | Robust, reliable |
| **Frontend** | Phoenix LiveView + Tailwind | Minimal JS, real-time built-in |
| **Image Processing** | libvips (via `vix`) | Fast, validated on Apple Silicon |
| **Background Jobs** | Oban | Reliable, good monitoring |
| **File Storage** | NAS (network mount) | Fast ingestion, scalable |

---

## File Serving Strategy

**Option A: Phoenix serves all files** (MVP)
- NAS mounted at `/mnt/nas`
- Phoenix serves via `send_file/3`
- Simpler setup

**Option B: NAS serves directly** (Scale)
- nginx on NAS at `:8080`
- Phoenix stores URLs in database
- Offloads file serving from Phoenix

**Recommendation:** Start with Option A, migrate to B if needed.

---

## Deployment Model

**MVP: Manual Setup**
```bash
MIX_ENV=prod mix release
./bin/photofinish start
# Server runs on http://localhost:4000
```

**Configuration (runtime.exs):**
```elixir
config :photofinish,
  photo_root: System.get_env("PHOTO_ROOT") || "/mnt/nas/originals",
  preview_root: System.get_env("PREVIEW_ROOT") || "/mnt/nas/previews",
  thumbnail_root: System.get_env("THUMBNAIL_ROOT") || "/mnt/nas/thumbnails",
  nas_http_base: System.get_env("NAS_HTTP_BASE"),  # optional
  enable_file_watcher: System.get_env("ENABLE_FILE_WATCHER", "true") == "true",
  thumbnail_size: String.to_integer(System.get_env("THUMBNAIL_SIZE", "320")),
  preview_size: String.to_integer(System.get_env("PREVIEW_SIZE", "1280")),
  max_concurrent_processing: String.to_integer(System.get_env("MAX_CONCURRENT_PROCESSING", "4")),
  admin_password: System.get_env("ADMIN_PASSWORD") || raise("ADMIN_PASSWORD required")
```

**Environment Variables (.env):**
```bash
PHOTO_ROOT=/mnt/nas/originals
PREVIEW_ROOT=/mnt/nas/previews
THUMBNAIL_ROOT=/mnt/nas/thumbnails
DATABASE_URL=postgresql://postgres:postgres@localhost/photofinish_prod
ADMIN_PASSWORD=your_secure_password_here
ENABLE_FILE_WATCHER=true
```

---

## Scale & Performance Requirements

| Metric | Target |
|--------|--------|
| Photos per event | 100,000+ |
| Competitors | 1,000+ |
| Photos per competitor | ~100 (20-40 per apparatus × 2-3 apparatuses) |
| Concurrent viewers | 15 stations |
| Storage per event | ~325 GB (originals + previews + thumbnails) |

**Performance Targets:**
- Photo discovery → database: < 1 second
- Thumbnail generation: < 2 seconds per photo
- Thumbnail grid load: < 500ms for 50 thumbnails
- Search response: < 200ms
- Full photo load: < 2 seconds

**Optimization Strategy:**
1. Database indexing (see 02-data-model.md)
2. Parallel thumbnail generation (4+ cores)
3. Virtual scrolling / pagination in viewer
4. Selective PubSub broadcasting (per-node topics)
5. Lazy loading thumbnails

---

## Network Topology

```
[Internet] ── (not required) ── [Venue Network]
                                      │
                    ┌─────────────────┼─────────────────┐
                    │                 │                 │
              [NAS Storage]    [Phoenix Server]   [WiFi AP]
                    │                 │                 │
                    └────── Gigabit ──┴────────────────┤
                                                       │
                              ┌────────────────────────┤
                              │                        │
                     [Viewer Tablets]          [Ingestion Station]
                        (×15)                    (Tauri App)
```

**Hardware (High-Level):**
- **Server:** MacOS (Apple Silicon), 16GB+ RAM, Gigabit Ethernet
- **NAS:** 2TB+, Gigabit Ethernet, RAID recommended
- **Viewers:** 10"+ tablets, ~$300/station, good WiFi
- **Network:** Dedicated event network (not public WiFi)

---

## Security Model

| Interface | Authentication |
|-----------|---------------|
| Viewer | None (open at venue) |
| Admin | Basic password auth |
| API (Tauri→Phoenix) | None (trusted local network) |

**Other Security:**
- File validation (JPEG only, size limits)
- XSS/CSRF protection (Phoenix defaults)
- SQL injection prevention (Ecto parameterized queries)

---

## Deployment Checklist

**Pre-Event:**
- [ ] Server hardware ready
- [ ] NAS configured and accessible
- [ ] PostgreSQL running
- [ ] Application deployed
- [ ] Network configured
- [ ] Viewing stations tested
- [ ] Event created in system
- [ ] Event structure configured (gyms, sessions)
- [ ] Roster imported
- [ ] File watcher running

**During Event:**
- [ ] Monitor ingestion pipeline
- [ ] Check viewing stations periodically
- [ ] Verify new photos appearing
- [ ] Have manual scan as backup

**Post-Event:**
- [ ] Verify all photos ingested
- [ ] Review error log
- [ ] Backup database and photos
- [ ] Process remaining orders
