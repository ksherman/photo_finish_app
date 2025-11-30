# PhotoFinish - Ingestion System

**Version:** 1.1
**Date:** November 29, 2025

---

## Overview

Photos are ingested via direct file copy from memory cards to NAS using a Tauri desktop application. The app identifies card readers, maps them to session destinations, and copies files with optional folder renaming.

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
- **Multi-reader support**: Built-in SD slot and external USB readers tracked separately

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
}
```

**Implementation:**
```rust
#[tauri::command]
pub fn discover_card_readers() -> Result<Vec<CardReaderInfo>, String> {
    // Uses `diskutil list -plist` and `diskutil info -plist`
    // Filters for removable media only
    // Counts JPEG files recursively
}
```

#### 2. Session Configuration

Each reader is configured once with:
- **Destination path**: Session folder on NAS (e.g., `/NAS/originals/event/gym-a/session-1`)
- **Photographer initials**: e.g., "KDS"
- **Camera brand**: Sony, Canon, Nikon, Fujifilm, Panasonic, Olympus
- **Rename settings**: Optional auto-rename with custom prefix

**Persistence:** Configuration stored in Pinia with `pinia-plugin-persistedstate`, survives app restarts.

**Reader Mapping Interface:**
```typescript
interface ReaderMapping {
  readerId: string;           // DeviceTreePath
  displayName: string;        // Human-readable reader name
  destination: string;        // Full path to session folder
  photographer: string;       // Photographer initials
  currentOrder: number;       // Batch counter (increments after copy)
  cameraBrand: string;        // "sony", "canon", "nikon", etc.
  cameraFolderPath: string;   // "DCIM" (where camera stores files)
  renamePrefix: string;       // e.g., "Gymnast" for auto-rename
  autoRename: boolean;        // Toggle for auto-rename during copy
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
- DCIM folder itself is not copied, only its contents

#### 4. Auto-Rename Feature

Automatically renames camera-generated folders to human-readable names during copy.

**Pattern:**
- Camera folder: `105MSDCF`, `101CANON`, `999_FUJI`
- Extracts last 2 digits of embedded number
- Applies custom prefix
- Result: `Gymnast 05`, `Gymnast 01`, `Gymnast 99`

**Implementation:**
```rust
fn apply_folder_rename(relative_path: &Path, prefix: &str) -> PathBuf {
    // Regex: r"(\d{2,3})" extracts 2-3 digit number
    // Takes last 2 digits
    // Formats as "{prefix} {number}"
    // Example: "105MSDCF" → "Gymnast 05"
}
```

**UI Control:**
- Text input for rename prefix in Reader Setup
- Checkbox toggle to enable/disable (disabled if no prefix entered)
- Renaming happens during copy, not as separate step

#### 5. File Copy Operation

**Source Path Construction:**
```
{mount_point}/{camera_folder_path}/{folders}
/Volumes/EOS_DIGITAL/DCIM/105MSDCF/IMG001.JPG
```

**Destination Path Construction:**
```
{destination}/{order}/{folder}/{files}
/NAS/originals/event/gym-a/session-1/0001/Gymnast 05/IMG001.JPG
                                      ^^^^  ^^^^^^^^^^
                                      auto  auto-renamed
                                      increment
```

**Features:**
- Recursive JPEG-only copy (`.jpg`, `.jpeg`)
- Preserves folder structure relative to source
- Real-time progress updates via Tauri Channel API
- Order number auto-increments after successful copy
- Skips hidden files (starting with `.`)

**Implementation:**
```rust
#[tauri::command]
pub async fn copy_files_to_destination(
    source_path: String,
    destination_path: String,
    on_progress: Channel<CopyProgressEvent>,
    rename_prefix: Option<String>,
    auto_rename: bool,
) -> Result<u32, String> {
    // Collects all JPEG files using WalkDir
    // Applies folder renaming if enabled
    // Copies files with progress updates
    // Returns count of copied files
}
```

**Progress Events:**
```rust
struct CopyProgressEvent {
    total: u32,
    copied: u32,
    current_file: String,
    percentage: f32,
}
```

### UI Screens

#### Reader Setup (Step 1)

Configuration screen shown when:
- No reader is configured yet
- User clicks "Change" on session summary

**Features:**
- List of detected card readers with "Configured" badge
- Shows reader name, volume name, photo count, mount point
- Destination folder picker (browse button)
- Photographer initials text input
- Camera brand dropdown
- Live preview: "Found: 1,234 photos in 12 folders"
- Rename prefix input with auto-rename checkbox
- Save button creates/updates reader mapping

```
┌────────────────────────────────────────┐
│ Reader Setup                           │
├────────────────────────────────────────┤
│ Select Card Reader                     │
│                                        │
│ ┌──────────────────────────────────┐  │
│ │ Built-in SD Reader         [✓]   │  │
│ │ EOS_DIGITAL - 1,234 photos       │  │
│ │ /Volumes/EOS_DIGITAL             │  │
│ └──────────────────────────────────┘  │
│                                        │
│ Destination Folder                     │
│ /NAS/.../gym-a/session-1  [Browse]     │
│                                        │
│ Photographer Initials                  │
│ KDS                                    │
│                                        │
│ Camera Brand                           │
│ Sony (DCIM) ▼                          │
│ Found: 1,234 photos in 12 folders      │
│                                        │
│ Folder Rename Prefix (Optional)        │
│ Gymnast                                │
│ ☑ Auto-rename folders during copy      │
│                                        │
│                      [Save & Start]    │
└────────────────────────────────────────┘
```

#### Main View (Step 2)

Active session screen after configuration.

```
┌────────────────────────────────────────┐
│ PhotoFinish Ingest                     │
│ Copy photos from memory card to server │
├────────────────────────────────────────┤
│ Session                      [Change]  │
│ Reader: Built-in SD Reader             │
│ Camera: Sony (DCIM)                    │
│ Photographer: KDS                      │
│ /NAS/.../gym-a/session-1               │
├────────────────────────────────────────┤
│ Card Reader Status                     │
│ ● Card Ready                           │
│ Volume: EOS_DIGITAL                    │
│ 1,234 photos ready to copy             │
│                                        │
│ Folders on card:                       │
│ 105MSDCF     124 photos                │
│ 106MSDCF      98 photos                │
│ 107MSDCF     156 photos                │
├────────────────────────────────────────┤
│ Destination Preview                    │
│ Next Order: #0001                      │
│ Destination: .../session-1/0001/       │
│ (will create: Gymnast 05, etc.)        │
├────────────────────────────────────────┤
│ [Copy 1,234 Files to Server]           │
├────────────────────────────────────────┤
│ Progress: ██████████ 1,234 / 1,234     │
│ IMG_8234.JPG                           │
├────────────────────────────────────────┤
│ ✓ Successfully copied 1,234 files!     │
│ Order incremented to #0002             │
└────────────────────────────────────────┘
```

**States:**
- No card: Disabled copy button
- Card ready: Shows file count, enabled copy button
- Copying: Progress bar, current file name, "Copying..." button
- Complete: Success message with file count and next order number

### Stores

#### Session Store (`src/stores/session.ts`)

**State:**
```typescript
{
  readerMappings: Map<readerId, ReaderMapping>,
  activeReaderId: string | null
}
```

**Actions:**
- `setReaderMapping()` - Save/update reader configuration
- `getReaderMapping(readerId)` - Retrieve configuration
- `removeReaderMapping(readerId)` - Delete configuration
- `setActiveReader(readerId)` - Set current reader
- `incrementOrder()` - Bump order counter after copy

**Computed:**
- `activeMapping` - Current reader's configuration
- `isConfigured` - Has active reader with mapping
- `destinationPath` - Full path including order number
- `currentOrderNumber` - Zero-padded order (0001, 0002)
- `currentDestination` - Human-readable destination
- `currentPhotographer` - Active photographer initials

#### Card Reader Store (`src/stores/cardReader.ts`)

**State:**
```typescript
{
  cardReaders: CardReaderInfo[],
  selectedReader: CardReaderInfo | null,
  directories: DirectoryEntry[],
  totalFileCount: number,
  copyProgress: CopyProgressEvent | null,
  isCopying: boolean,
  copyErrors: string[]
}
```

**Actions:**
- `discoverReaders()` - Query system for removable media
- `selectReader(reader, cameraFolderPath)` - Set active reader
- `loadDirectoriesFromReader(cameraFolderPath)` - Load folder list
- `copyFiles(destination, cameraPath, prefix, autoRename)` - Execute copy
- `clearSelection()` - Reset state

**Auto-refresh:**
- Polls `discoverReaders()` every 3 seconds when not copying
- Updates file counts for selected reader
- Detects reader disconnection and clears selection

---

## File Organization

### Current Implementation (MVP)

Files are copied directly to session destination with order tracking:

```
{destination}/{order}/{folder}/{files}

Example:
/NAS/originals/event/gym-a/session-1/0001/Gymnast 05/IMG_8234.jpg
/NAS/originals/event/gym-a/session-1/0001/Gymnast 06/IMG_8235.jpg
/NAS/originals/event/gym-a/session-1/0002/Gymnast 05/IMG_8300.jpg
                                     ^^^^
                                     increments with each card
```

**Components:**
- `{destination}` - Session folder chosen in setup
- `{order}` - Auto-incrementing batch number (0001, 0002, ...)
- `{folder}` - Camera folder, optionally renamed (e.g., "Gymnast 05")
- `{files}` - Original JPEG files from card

### Why Track Order

- **Traceability:** Can identify which memory card batch a photo came from
- **Chronology:** Card sequence helps determine shooting order
- **Troubleshooting:** Easy to identify problematic batches
- **Photographer workflow:** Matches physical card handling sequence

---

## Phoenix Integration (Future)

The following features are planned but not yet implemented:

### File Watcher

Phoenix will monitor the NAS destination folders and automatically:
- Detect new JPEG files
- Extract EXIF metadata
- Generate preview (1280px) and thumbnail (320px)
- Broadcast real-time updates to viewing stations

### API Endpoints (Stubbed)

```
GET  /api/events/:event_id/roster
     → Returns competitor list (for future roster integration)

POST /api/ingestion/notify
     Body: {
       event_id,
       photographer,
       order_number,
       destination_path,
       file_count
     }
     → Triggers Phoenix processing (optional manual trigger)
```

### Processing Pipeline

Three versions per photo:

| Version | Size | Quality | Purpose |
|---------|------|---------|---------|
| Original | As-is | N/A | Orders/printing (untouched on NAS) |
| Preview | 1280px long edge | 90% | Viewing at kiosks |
| Thumbnail | 320px long edge | 85% | Grid display |

---

## Technical Implementation Details

### macOS Card Reader Discovery

**Using `diskutil` command-line tool:**

1. `diskutil list -plist` - Get all disks and partitions
2. `diskutil info -plist {disk}` - Get disk details
3. Filter for `Removable: true` or `RemovableMedia: true`
4. Extract `DeviceTreePath` as unique identifier
5. Find mounted partitions with `MountPoint`
6. Count JPEG files using `walkdir`

**Why DeviceTreePath?**
- Unique per physical card reader slot
- Persists across different cards
- Built-in SD: `IODeviceTree:/pcie@0/pcie-sdreader@0`
- USB reader: `IODeviceTree:/arm-io@.../{unique-path}`

### File Counting

```rust
fn count_jpeg_files(path: &Path) -> u32 {
    WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| {
                    let ext_lower = ext.to_string_lossy().to_lowercase();
                    ext_lower == "jpg" || ext_lower == "jpeg"
                })
                .unwrap_or(false)
        })
        .count() as u32
}
```

### Copy Progress Tracking

```rust
// Tauri backend sends progress events
let channel = new Channel<CopyProgressEvent>();
channel.onmessage = (progress) => {
  this.copyProgress = progress;
};

await invoke("copy_files_to_destination", {
  sourcePath,
  destinationPath,
  onProgress: channel,
  renamePrefix,
  autoRename
});
```

### State Persistence

```typescript
// In src/main.ts
import { createPinia } from 'pinia'
import piniaPluginPersistedstate from 'pinia-plugin-persistedstate'

const pinia = createPinia()
pinia.use(piniaPluginPersistedstate)

// In session store
export const useSessionStore = defineStore('session', {
  state: () => ({ /* ... */ }),
  persist: true  // Auto-persists to localStorage
})
```

---

## Error Handling

| Problem | Solution |
|---------|----------|
| No card readers detected | Show message with manual refresh button |
| Card removed during copy | Copy operation fails, show error |
| Destination path doesn't exist | Rust creates all parent directories |
| File copy fails | Error added to `copyErrors[]`, operation continues |
| Same card copied twice | Order increments, files copied to new folder |

---

## Future Enhancements

**Not yet implemented:**

1. **Barcode scanning** - Envelope codes for auto-categorization
2. **Roster integration** - Load competitor list from Phoenix API
3. **Phoenix API notifications** - Notify server when copy completes
4. **Manual folder renaming** - Dropdown with roster entries
5. **Duplicate detection** - Skip files already copied
6. **RAW file support** - Currently JPEG-only
7. **Multi-card batch copy** - Copy from multiple readers sequentially
8. **Copy verification** - Checksum validation after copy

---

## Development

**Location:** `/apps/ingest`

**Commands:**
```bash
pnpm install              # Install dependencies
pnpm tauri dev            # Run dev server with hot reload
pnpm tauri build          # Build production app
```

**Key Files:**
- `src-tauri/src/commands.rs` - Rust backend commands
- `src/stores/session.ts` - Session & reader mapping state
- `src/stores/cardReader.ts` - Card reader discovery & copy state
- `src/components/ReaderSetup.vue` - Reader configuration UI
- `src/components/CardReaderStatus.vue` - Card status display
- `src/views/MainView.vue` - Main application view

**Dependencies:**
- Rust: `tauri@2`, `walkdir`, `plist`, `regex`, `serde`, `tokio`
- Vue: `pinia`, `pinia-plugin-persistedstate`, `@tauri-apps/api`, `@tauri-apps/plugin-fs`, `@tauri-apps/plugin-dialog`
