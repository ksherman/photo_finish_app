# PhotoFinish - Ingestion System

**Version:** 1.2
**Date:** November 29, 2025

---

## Overview

Photos are ingested via direct file copy from memory cards to NAS using a Tauri desktop application. The app features a multi-reader dashboard that allows independent configuration and parallel ingestion from multiple sources.

---

## Workflow Summary

```
Photographer → Memory Card → Card Reader → Tauri App → NAS
     │              │             │            │         │
  capture      folder per     USB/SD      configure   copy with
             apparatus      (persists)    & rename   progress
```

---

## Tauri Ingestion Application

### Purpose

Desktop app for copying photos from memory cards to NAS. Required because browsers cannot access USB drives or local filesystems.

### Technology Stack

- **Backend:** Rust (Tauri 2.x framework)
- **Frontend:** Vue 3 (Composition API) + TypeScript
- **State Management:** Pinia with persistence
- **Styling:** Tailwind CSS v4
- **Platform:** macOS (Apple Silicon primary)

### Core Features

#### 1. Card Reader Discovery & Mapping

- **Automatic detection** of removable media using macOS `diskutil`
- **Unique identification** via DeviceTreePath (persists across card swaps)
- **Reader mapping**: Each reader can be permanently mapped to a destination
- **Multi-reader support**: Dashboard grid layout supports unlimited concurrent readers

**Reader Information Collected:**
```rust
struct CardReaderInfo {
    reader_id: String,          // DeviceTreePath (unique per physical slot)
    display_name: String,       // "Built-in SD Reader", "External USB Reader"
    mount_point: String,        // /Volumes/EOS_DIGITAL
    volume_name: String,        // Name of mounted card
    bus_protocol: String,       // "Secure Digital", "USB", etc.
    is_internal: bool,          // Built-in vs external
    disk_id: String,            // disk4, disk5, etc.
    file_count: u32,            // Number of JPEGs on card
    folder_count: u32,          // Number of folders on card
}
```

#### 2. Session Configuration

Each reader is configured individually with:
- **Destination path**: Base session folder on NAS
- **Photographer initials**: e.g., "KDS"
- **Camera brand**: Sony, Canon, Nikon, Fujifilm, Panasonic, Olympus
- **Folder Rename settings**: Optional auto-rename folders (e.g. "105MSDCF" -> "Gymnast 05")
- **File Rename settings**: Optional auto-rename files (e.g. "IMG_1234.jpg" -> "Gymnast_1234.jpg")

**Persistence:** Configuration stored in Pinia with `pinia-plugin-persistedstate`, survives app restarts.

**Reader Mapping Interface:**
```typescript
interface ReaderMapping {
  readerId: string;           // DeviceTreePath
  displayName: string;        // Human-readable reader name
  destination: string;        // Full path to session folder
  photographer: string;       // Photographer initials
  currentOrder: number;       // Batch counter (tracked internally)
  cameraBrand: string;        // "sony", "canon", "nikon", etc.
  cameraFolderPath: string;   // "DCIM" (where camera stores files)
  renamePrefix: string;       // e.g., "Gymnast" for auto-renaming folders
  autoRename: boolean;        // Toggle for auto-rename folders
  fileRenamePrefix?: string;  // e.g., "Gymnast" for auto-renaming files
}
```

#### 3. Camera Brand Support

Different cameras organize files differently. The app supports:

| Brand | Folder Structure |
|-------|-----------------|
| Sony | `/DCIM/100MSDCF/` |
| Canon | `/DCIM/100CANON/` |
| Nikon | `/DCIM/100NIKON/` |
| Fujifilm | `/DCIM/100_FUJI/` |
| Panasonic | `/DCIM/100_PANA/` |
| Olympus | `/DCIM/100OLYMP/` |

When a camera brand is selected during setup:
- Live preview shows folder and file counts from the correct path
- Copy operation automatically uses the correct source path

#### 4. Auto-Rename Feature (Folders)

Automatically renames camera-generated folders to human-readable names during copy.

**Pattern:**
- Camera folder: `105MSDCF`, `101CANON`
- Extracts last 2 digits of embedded number
- Applies custom prefix
- Result: `Gymnast 05`, `Gymnast 01`

#### 5. Auto-Rename Feature (Files)

Automatically renames individual image files while preserving the sequence number.

**Pattern:**
- Source: `IMG_1234.JPG`
- Prefix: `Gymnast`
- Result: `Gymnast_1234.JPG`

#### 6. File Copy Operation

**Source Path Construction:**
```
{mount_point}/{camera_folder_path}/{folders}
/Volumes/EOS_DIGITAL/DCIM/105MSDCF/IMG001.JPG
```

**Destination Path Construction:**
```
{destination}/{sub_folder}/{folder}/{files}
/NAS/session-1/Group 8/Gymnast 05/Gymnast_001.JPG
              ^^^^^^^  ^^^^^^^^^^ ^^^^^^^^^^^
              manual   auto       auto-renamed
              input    renamed    file
```

**Features:**
- Recursive JPEG-only copy (`.jpg`, `.jpeg`)
- Preserves folder structure relative to source
- Real-time progress updates via Tauri Channel API
- Skips hidden files (starting with `.`)
- Non-blocking I/O (runs on separate thread)
- Throttled progress updates (every 100ms)

**Implementation:**
```rust
#[tauri::command]
pub async fn copy_files_to_destination(
    source_path: String,
    destination_path: String,
    on_progress: Channel<CopyProgressEvent>,
    rename_prefix: Option<String>,
    auto_rename: bool,
    file_rename_prefix: Option<String>,
) -> Result<u32, String> {
    // Spawns blocking thread
    // Collects files
    // Renames folders and files
    // Copies with progress
}
```

### UI Screens

#### Dashboard View

The main screen displays a grid of all connected card readers. Each card shows:

**Reader Card:**
- **Header:** Reader name, Volume name, Status (Ready/Setup Needed)
- **Info:** Photographer initials, File count, Folder count
- **Destination:** Live preview of the full copy path
- **Input:** Optional "Sub-folder" field (e.g., "Group 8B")
- **Actions:** "Configure" (opens modal), "Copy Files"

#### Reader Configuration (Modal)

Modal for configuring a specific reader.
- Destination folder picker
- Photographer initials
- Camera brand selection
- File Rename Prefix input
- Folder Rename Prefix input + toggle
- Live preview of files found based on settings

### Stores

#### Session Store (`src/stores/session.ts`)

**State:**
```typescript
{
  readerMappings: Record<string, ReaderMapping>
}
```

**Actions:**
- `setReaderMapping()` - Save/update reader configuration
- `getReaderMapping(readerId)` - Retrieve configuration
- `incrementOrderForReader(readerId)` - Increment internal batch counter

#### Card Reader Store (`src/stores/cardReader.ts`)

**State:**
```typescript
{
  cardReaders: CardReaderInfo[],
  // Per-reader copy state
  copyStates: Record<string, {
    isCopying: boolean,
    progress: CopyProgressEvent | null,
    error: string | null,
    lastResult: { count: number, success: boolean, durationSeconds: number }
  }>
}
```

**Actions:**
- `discoverReaders()` - Query system for removable media
- `copyFiles(readerId, ...)` - Execute copy for specific reader

---

## File Organization

Files are organized by Session, optional Sub-folder, and Camera Folder:

```
{destination}/{sub_folder}/{camera_folder}/{filename}
```

- **destination**: Base NAS path (e.g. `/NAS/Events/GymMeet/Session1`)
- **sub_folder**: Optional manual input (e.g. `Floor/Group 5`)
- **camera_folder**: Original or renamed camera folder (e.g. `Gymnast 05`)
- **filename**: Original or renamed file (e.g. `Gymnast_1234.jpg`)

---

## Future Enhancements

1.  **Phoenix API notifications** - Notify server when copy completes (POST to API)
2.  **Roster integration** - Advanced folder renaming using competitor names
3.  **Duplicate detection** - Checksum verification to prevent re-copying
4.  **Barcode scanning** - Future workflow optimization

---

## Development

**Location:** `/apps/ingest`

**Commands:**
```bash
pnpm install              # Install dependencies
pnpm tauri dev            # Run dev server with hot reload
pnpm tauri build          # Build production app
```