# PhotoFinish - Admin Interface

**Version:** 1.0  
**Date:** November 29, 2025

---

## Overview

Password-protected web interface for event management, photo organization, roster import, and order processing.

**Authentication:** Basic password auth (single admin account for MVP)

---

## Main Sections

1. **Dashboard** — Overview and monitoring
2. **Event Setup** — Hierarchy configuration
3. **Roster** — Competitor import and management
4. **Photos** — File browser and organization
5. **Orders** — Order management and fulfillment

---

## Dashboard

```
┌─────────────────────────────────────────────────────┐
│ PhotoFinish Admin                    [Event: STV ▼] │
├─────────────────────────────────────────────────────┤
│                                                     │
│  Photos                    Orders                   │
│  ┌─────────────────┐       ┌─────────────────┐     │
│  │ Ready: 12,456   │       │ Today: 47       │     │
│  │ Processing: 12  │       │ Pending: 8      │     │
│  │ Errors: 3       │       │ Revenue: $4,230 │     │
│  └─────────────────┘       └─────────────────┘     │
│                                                     │
│  Ingestion                 System                   │
│  ┌─────────────────┐       ┌─────────────────┐     │
│  │ Auto-watch: ● ON│       │ Viewers: 12     │     │
│  │ Last scan: 2m   │       │ Uptime: 4h 23m  │     │
│  │ [Scan Now]      │       │ Storage: 45% ↗  │     │
│  └─────────────────┘       └─────────────────┘     │
│                                                     │
│  Recent Activity                                    │
│  ├ 12:34 - 24 photos added (1022 Kevin S)          │
│  ├ 12:32 - Order STV-0812 paid (cash)              │
│  ├ 12:30 - 31 photos added (1023 Sarah J)          │
│  └ 12:28 - Roster updated (+5 competitors)         │
│                                                     │
└─────────────────────────────────────────────────────┘
```

---

## Event Setup

### Create Event

Fields:
- Name (required)
- Slug (auto-generated, editable)
- Date
- Description
- Order code (3 letters for order numbers)
- Tax rate (default 8.5%)

### Hierarchy Configuration

Define levels for the event:

```
Level 1: Gym        (allow_photos: false)
Level 2: Session    (allow_photos: false)
Level 3: Apparatus  (allow_photos: false)
Level 4: Flight     (allow_photos: false)
Level 5: Competitor (allow_photos: true)
```

### Pre-create Nodes

Option to create hierarchy structure before event:
- Bulk create gyms, sessions, apparatuses
- Or allow dynamic creation during ingestion

---

## Roster Management

### Import

**CSV Format:**
```csv
competitor_number,first_name,last_name,team,level,age_group
1022,Kevin,Smith,Bay Area Gymnastics,5,Junior
1023,Sarah,Johnson,Elite Tumbling,7,Senior
```

**Import UI:**
```
┌─────────────────────────────────────────────────────┐
│ Import Roster                                       │
├─────────────────────────────────────────────────────┤
│                                                     │
│  [Choose File] roster.csv                          │
│                                                     │
│  Preview:                                           │
│  ┌───────┬─────────┬──────────┬─────────────────┐  │
│  │ #     │ Name    │ Team     │ Level           │  │
│  ├───────┼─────────┼──────────┼─────────────────┤  │
│  │ 1022  │ Kevin S │ Bay Area │ 5               │  │
│  │ 1023  │ Sarah J │ Elite    │ 7               │  │
│  │ 1024  │ Emma W  │ Bay Area │ 5               │  │
│  └───────┴─────────┴──────────┴─────────────────┘  │
│                                                     │
│  Found: 156 competitors                            │
│  New: 156 | Updates: 0 | Errors: 0                 │
│                                                     │
│  [Cancel]                           [Import 156]   │
└─────────────────────────────────────────────────────┘
```

### CRUD Operations

- **Add:** Manual entry form
- **Edit:** Click row to edit
- **Delete:** Soft delete (sets `deleted_at`)
- **Search:** Filter by name, number, team

### Roster Table

```
┌─────────────────────────────────────────────────────┐
│ Roster (156 competitors)        [+ Add] [Import]   │
├─────────────────────────────────────────────────────┤
│ Search: [_______________]                           │
├───────┬───────────┬─────────────────┬───────┬──────┤
│ #     │ Name      │ Team            │ Photos│ Edit │
├───────┼───────────┼─────────────────┼───────┼──────┤
│ 1022  │ Kevin S   │ Bay Area Gym    │ 24    │ [✎] │
│ 1023  │ Sarah J   │ Elite Tumbling  │ 31    │ [✎] │
│ 1024  │ Emma W    │ Bay Area Gym    │ 0     │ [✎] │
└───────┴───────────┴─────────────────┴───────┴──────┘
```

---

## Photo Management

### File Browser

Tree view + grid layout:

```
┌─────────────────────────────────────────────────────┐
│ Photos                              [Scan Now]     │
├──────────────┬──────────────────────────────────────┤
│ 📁 Gym A     │  Gym A > Session 2 > Floor          │
│   📁 Sess 1  │                                      │
│   📁 Sess 2  │  ┌────┐ ┌────┐ ┌────┐ ┌────┐       │
│     📁 Floor │  │ 📷 │ │ 📷 │ │ 📷 │ │ 📷 │       │
│     📁 Beam  │  │1022│ │1022│ │1022│ │1023│       │
│   📁 Sess 3  │  └────┘ └────┘ └────┘ └────┘       │
│ 📁 Gym B     │                                      │
│              │  Selected: 4 photos                  │
│              │  [Move To...] [Delete]               │
└──────────────┴──────────────────────────────────────┘
```

### Photo Actions

- **View details:** EXIF, file info, ingestion path
- **Move:** Reassign to different competitor/node
- **Delete:** Remove with confirmation
- **Bulk select:** Multi-select for batch operations

### Correction Workflow

**Scenario: Photos in wrong competitor folder**

1. Navigate to incorrect folder
2. Select misplaced photos
3. Click "Move To..."
4. Select correct competitor from dropdown
5. Confirm → files physically moved, DB updated

### Ingestion Monitoring

```
┌─────────────────────────────────────────────────────┐
│ Ingestion Status                                    │
├─────────────────────────────────────────────────────┤
│ Auto-watch: ● Enabled        [Disable]             │
│ Last scan: 2 minutes ago     [Scan Now]            │
│                                                     │
│ Queue: 12 photos processing                         │
│ ████████████░░░░░░░░ 65%                           │
├─────────────────────────────────────────────────────┤
│ Errors (3)                                          │
├──────────────────────────────────────┬─────────────┤
│ /originals/.../IMG_8234.jpg          │ Corrupt     │
│ /originals/.../IMG_8235.jpg          │ Too large   │
│ /originals/.../IMG_8236.jpg          │ Read error  │
├──────────────────────────────────────┴─────────────┤
│ [Retry All] [Delete All] [Clear Resolved]          │
└─────────────────────────────────────────────────────┘
```

---

## Order Management

See `06-ordering.md` for complete ordering system documentation.

### Orders Dashboard

```
┌─────────────────────────────────────────────────────┐
│ Orders                    Today: $4,230 (47 orders)│
├─────────────────────────────────────────────────────┤
│ Filter: [All ▼] [Pending ▼]  Search: [___________] │
├─────────┬──────────┬─────────┬─────────┬───────────┤
│ Order   │ Customer │ Total   │ Payment │ Status    │
├─────────┼──────────┼─────────┼─────────┼───────────┤
│ STV-0815│ Jane D   │ $118.50 │ Pending │ [Mark Paid]│
│ STV-0814│ Mike T   │ $100.00 │ Cash    │ Fulfilling│
│ STV-0813│ Sarah K  │ $148.00 │ Square  │ Ready     │
└─────────┴──────────┴─────────┴─────────┴───────────┘
```

### Quick Actions

- **Mark Paid:** Record payment method + reference
- **View Order:** Full details + fulfillment
- **Print Receipt:** Generate printable receipt

---

## System Settings

```
┌─────────────────────────────────────────────────────┐
│ Settings                                            │
├─────────────────────────────────────────────────────┤
│                                                     │
│ File Paths                                          │
│ ├ Photo root: /mnt/nas/originals                   │
│ ├ Preview root: /mnt/nas/previews                  │
│ └ Thumbnail root: /mnt/nas/thumbnails              │
│                                                     │
│ Processing                                          │
│ ├ Thumbnail size: [320] px                         │
│ ├ Preview size: [1280] px                          │
│ ├ Concurrent workers: [4]                          │
│ └ Auto-create nodes: [✓]                           │
│                                                     │
│ [Save Changes]                                      │
└─────────────────────────────────────────────────────┘
```

---

## LiveView Implementation

### Admin Layout

```elixir
defmodule PhotoFinishWeb.AdminLive do
  use PhotoFinishWeb, :live_view
  
  on_mount {PhotoFinishWeb.AdminAuth, :require_admin}
  
  def mount(_params, _session, socket) do
    if connected?(socket) do
      # Subscribe to admin-relevant topics
      Phoenix.PubSub.subscribe(PhotoFinish.PubSub, "admin:updates")
    end
    
    {:ok, assign_stats(socket)}
  end
end
```

### Authentication Plug

```elixir
defmodule PhotoFinishWeb.AdminAuth do
  def on_mount(:require_admin, _params, session, socket) do
    if session["admin_authenticated"] do
      {:cont, socket}
    else
      {:halt, redirect(socket, to: "/admin/login")}
    end
  end
end
```
