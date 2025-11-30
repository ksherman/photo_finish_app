# PhotoFinish - Ingestion System

**Version:** 1.0  
**Date:** November 29, 2025

---

## Overview

Photos are ingested via direct file copy to NAS, not HTTP upload. The Tauri desktop app handles memory card reading and file copying. Phoenix discovers files via file watcher and processes them.

---

## Workflow Summary

```
Photographer → Memory Card → Tauri App → NAS → File Watcher → Processing → Viewer
     │              │            │         │          │            │
   capture      folder per    barcode    copy     detect      thumbnail
               competitor     scan                            + preview
```

---

## Tauri Ingestion Application

### Purpose

Desktop app for copying photos from memory cards. Required because browsers cannot access USB drives or local filesystems.

### Technology Stack

- **Backend:** Rust (Tauri framework)
- **Frontend:** VueJS 3 + Tailwind CSS
- **Platform:** macOS (Apple Silicon primary)

### Core Features

1. **Session Configuration**
   - Set destination root path
   - Select card reader drive
   - Load photographer profile
   - Load roster from Phoenix API or file

2. **Memory Card Workflow**
   - Insert card
   - Scan envelope barcode → auto-fills rotation/apparatus
   - Click "Copy Files" → copies to NAS
   - Rename folders (camera names → competitor names)
   - Notify Phoenix via API

3. **Folder Renaming**
   - Camera creates folders: `EOS100`, `EOS101`, `EOS102`
   - Staff renames using roster: `EOS100` → `1022 Kevin S`
   - Dropdown with roster entries for quick selection

### UI Screens

**Main Screen:**
```
┌────────────────────────────────────────┐
│ Session: Gym A / Session 3             │
│ Photographer: KDS                      │
├────────────────────────────────────────┤
│ Card Reader: /Volumes/EOS_DIGITAL      │
│ Status: ● Card Ready (1,234 files)     │
│                                        │
│ Envelope Code: [Group 1A/Beam_____]    │
│                                        │
│ Destination:                           │
│ /nas/.../Gym A/Session 3/              │
│ Group 1A/Beam/0001/                    │
│                                        │
│ [Copy Files to Server]                 │
│                                        │
│ Progress: ██████░░░░ 756/1234          │
└────────────────────────────────────────┘
```

**Folder Renaming:**
```
┌────────────────────────────────────────┐
│ Rename Folders                         │
├────────────────────────────────────────┤
│ EOS100 → [1022 Kevin S ▼]    12 photos │
│ EOS101 → [1023 Sarah J ▼]    15 photos │
│ EOS102 → [1024 Emma W  ▼]    18 photos │
│ EOS103 → [Skip (empty)]       0 photos │
├────────────────────────────────────────┤
│ [Auto-Assign] [Apply All] [Cancel]     │
└────────────────────────────────────────┘
```

### Phoenix API Integration

```
GET  /api/events/:event_id/roster
     → Returns competitor list

POST /api/ingestion/notify
     Body: {
       event_id,
       photographer,
       envelope_code,
       order_number,
       destination_path,
       file_count
     }
     → Triggers Phoenix processing

POST /api/ingestion/rename
     Body: {
       renames: [{original, new, photo_count}]
     }
     → Reports folder renames
```

---

## File Organization

### Two-Phase Approach

**Phase 1: Ingestion Path** (preserves source structure)
```
/originals/{EVENT}/{PHOTOGRAPHER}/{GYM}/{SESSION}/{ORDER}/{SOURCE}/
  └── IMG_001.jpg

Example:
/originals/st-valentines-meet/kds/gym-a/session-3/0001/EOS100/IMG_8234.jpg
```

Components:
- `{EVENT}` — Event slug
- `{PHOTOGRAPHER}` — Photographer initials (e.g., "KDS")
- `{GYM}` — Gym identifier
- `{SESSION}` — Session number
- `{ORDER}` — Memory card sequence (0001, 0002, ...)
- `{SOURCE}` — Original folder from card

**Phase 2: Finalization** (clean archival structure, optional)
```
/originals/{EVENT}/{COMPETITOR_NUMBER}/{APPARATUS}/
  └── IMG_001.jpg

Example:
/originals/st-valentines-meet/1022-kevin-s/floor/IMG_8234.jpg
```

### Why Two Phases

- **Traceability:** Can always trace back to source card
- **Order matters:** Card sequence helps troubleshooting
- **Flexible:** Doesn't force immediate categorization
- **Safe:** Preserves photographer's original organization

---

## Phoenix File Watcher

### Implementation

```elixir
defmodule PhotoFinish.PhotoWatcher do
  use GenServer
  
  def start_link(opts) do
    GenServer.start_link(__MODULE__, opts, name: __MODULE__)
  end
  
  def init(_opts) do
    photo_root = Application.get_env(:photofinish, :photo_root)
    {:ok, watcher} = FileSystem.start_link(dirs: [photo_root])
    FileSystem.subscribe(watcher)
    {:ok, %{watcher: watcher}}
  end
  
  def handle_info({:file_event, _pid, {path, events}}, state) do
    if should_process?(path, events) do
      PhotoFinish.Ingestion.enqueue_photo(path)
    end
    {:noreply, state}
  end
  
  defp should_process?(path, events) do
    is_jpeg?(path) and
    (:created in events or :modified in events) and
    not temp_file?(path)
  end
  
  defp is_jpeg?(path) do
    ext = path |> Path.extname() |> String.downcase()
    ext in [".jpg", ".jpeg"]
  end
  
  defp temp_file?(path), do: String.starts_with?(Path.basename(path), ".")
end
```

### Manual Scan (Backup)

```elixir
defmodule PhotoFinish.ManualScanner do
  def scan_directory(path, event_id) do
    path
    |> File.ls!()
    |> Enum.filter(&is_jpeg?/1)
    |> Enum.reject(&already_imported?(&1, event_id))
    |> Enum.each(&PhotoFinish.Ingestion.enqueue_photo/1)
  end
end
```

---

## Processing Pipeline

### Oban Job

```elixir
defmodule PhotoFinish.Workers.ProcessPhoto do
  use Oban.Worker, queue: :photos, max_attempts: 3
  
  @impl Oban.Worker
  def perform(%Oban.Job{args: %{"path" => path, "event_id" => event_id}}) do
    with {:ok, photo} <- create_record(path, event_id),
         {:ok, photo} <- extract_exif(photo),
         {:ok, photo} <- generate_preview(photo),
         {:ok, photo} <- generate_thumbnail(photo),
         {:ok, photo} <- mark_ready(photo) do
      broadcast_new_photo(photo)
      :ok
    end
  end
end
```

### Image Processing

Three versions per photo:

| Version | Size | Quality | Watermark | Purpose |
|---------|------|---------|-----------|---------|
| Original | As-is | N/A | No | Orders/printing |
| Preview | 1280px long edge | 90% | TBD | Viewing |
| Thumbnail | 320px long edge | 85% | TBD | Grid display |

```elixir
defmodule PhotoFinish.ImageProcessor do
  def process(original_path, photo_id, event_slug) do
    {:ok, img} = Vix.Vips.Image.new_from_file(original_path)
    
    # Generate preview (1280px)
    preview_path = preview_path(photo_id, event_slug)
    {:ok, preview} = Vix.Vips.Operation.thumbnail_image(img, 1280)
    # preview = apply_watermark(preview)  # TODO: implement
    :ok = Vix.Vips.Image.write_to_file(preview, preview_path, Q: 90)
    
    # Generate thumbnail from preview (320px)
    thumbnail_path = thumbnail_path(photo_id, event_slug)
    {:ok, thumb} = Vix.Vips.Operation.thumbnail_image(preview, 320)
    :ok = Vix.Vips.Image.write_to_file(thumb, thumbnail_path, Q: 85)
    
    {:ok, %{preview_path: preview_path, thumbnail_path: thumbnail_path}}
  end
end
```

### Parallel Processing

```elixir
# In Oban config
config :photofinish, Oban,
  queues: [photos: 4]  # 4 concurrent workers
```

At 4 workers × 2 sec/photo = ~7,200 photos/hour.

---

## Path Parsing

```elixir
defmodule PhotoFinish.PathParser do
  @doc """
  Parse ingestion path to extract metadata.
  Path format: /originals/{event}/{photographer}/{gym}/{session}/{order}/{source}/file.jpg
  """
  def parse(path) do
    parts = path |> Path.split() |> Enum.drop(1)  # drop root
    
    %{
      event_slug: Enum.at(parts, 1),
      photographer: Enum.at(parts, 2),
      gym_slug: Enum.at(parts, 3),
      session_slug: Enum.at(parts, 4),
      order_number: Enum.at(parts, 5),
      source_folder: Enum.at(parts, 6),
      filename: List.last(parts)
    }
  end
end
```

---

## Error Handling

| Problem | Solution |
|---------|----------|
| File locked/being written | Retry with backoff (max 3 attempts) |
| Corrupt image | Mark as error, admin review |
| Duplicate file | Mark as duplicate, skip processing |
| Missing directory | Auto-create if `auto_create_nodes: true` |
| File moved/deleted | Mark as error, clean up record |
| Large file (>50MB) | Reject with warning |

### Duplicate Detection

```elixir
def is_duplicate?(path, event_id) do
  filename = Path.basename(path)
  file_size = File.stat!(path).size
  
  from(p in Photo,
    where: p.event_id == ^event_id,
    where: p.filename == ^filename,
    where: p.file_size_bytes == ^file_size
  )
  |> Repo.exists?()
end
```

---

## Real-Time Updates

```elixir
defp broadcast_new_photo(photo) do
  Phoenix.PubSub.broadcast(
    PhotoFinish.PubSub,
    "photos:node:#{photo.node_id}",
    {:new_photo, photo}
  )
  
  # Also broadcast to competitor topic if assigned
  if photo.competitor_id do
    Phoenix.PubSub.broadcast(
      PhotoFinish.PubSub,
      "photos:competitor:#{photo.competitor_id}",
      {:new_photo, photo}
    )
  end
end
```

---

## Admin Monitoring

Dashboard displays:
- Total photos discovered
- Photos ready
- Photos processing
- Errors (with review link)
- Last scan time
- Auto-watch status
- [Scan Now] button

Error log shows:
- File path
- Error message
- Timestamp
- Actions: [Retry] [Delete] [Ignore]
