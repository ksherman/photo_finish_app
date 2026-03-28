# Ingest App: Require Subfolder + Folder Listing Design

**Date:** 2026-03-28
**Scope:** Tauri ingest app (`apps/ingest/`) only — no server changes

---

## Overview

Two related improvements to the ingest app's copy workflow:

1. **Require Subfolder** — a config toggle that blocks copying until the operator has typed a subfolder name, then auto-clears the field after a successful copy.
2. **Folder Listing with Toggles** — a scrollable per-folder breakdown inside each reader card showing file counts, a rename preview (when folder rename is enabled), and a per-folder include/ignore toggle that controls which folders get copied.

---

## Feature 1: Require Subfolder

### Config

Add `requireSubfolder: boolean` (default `false`) to the `ReaderMapping` interface in `session.ts` and to `setReaderMapping()`.

### ReaderSetup.vue

Add a checkbox: "Require sub-folder before copying". No conditional visibility — always shown below the folder rename section.

### ReaderDashboardCard.vue

**`canCopy` update:** Add condition `!(mapping.requireSubfolder && !cleanSubPath.value)`.

**Hint text:** When `requireSubfolder` is on and `cleanSubPath` is empty, show a small inline message near the subfolder input ("Sub-folder required to copy") so the operator understands why the button is disabled.

**Auto-clear:** After `startCopy()` resolves successfully, set `customSubPath.value = ""`. This applies regardless of whether `requireSubfolder` is on — clearing after copy is always the right behavior.

---

## Feature 2: Folder Listing with Toggles

### Data Flow

- On mount (when reader is configured and online), call `invoke('list_directory', { path })` for the reader's camera folder path to get `DirectoryEntry[]`.
- After a successful copy completes, reload the list and reset all toggles to their defaults.
- Toggle state lives in a local `ref<Record<string, boolean>>` inside `ReaderDashboardCard` — no store changes. Keyed by folder name, `true` = include, `false` = ignore.

**Toggle defaults on load/reload:**
- Non-empty folder (file_count > 0): `true` (included)
- Empty folder (file_count === 0): `false` (ignored)

### Rename Preview

Computed in Vue, mirroring the Rust `apply_folder_rename` logic. Only shown when `mapping.autoRename` is true and `mapping.renamePrefix` is set.

```ts
function previewFolderName(name: string, prefix: string): string {
  const match = name.match(/(\d{2,3})/);
  if (!match) return name;
  const lastTwo = match[1].slice(-2);
  return `${prefix} ${lastTwo}`;
}
```

### UI (inside ReaderDashboardCard)

Sits above the subfolder input, below the photographer/file-count summary. Fixed height (~180px) with `overflow-y-auto`.

Each row contains:
- Checkbox toggle (include/ignore)
- Folder name (monospace, truncated if needed)
- File count badge (grayed out / yellow warning dot if 0)
- Arrow + rename preview (only when `autoRename` is on)

Above the list, a single-line row of small controls:
- "Select all" link — sets all toggles to `true`
- "Reset defaults" link — reapplies the empty/non-empty default logic

Empty folders get a yellow warning indicator (e.g., `⚠ 0 files`) and are toggled off by default.

### Rust: `copy_files_to_destination`

Add parameter: `included_folders: Option<Vec<String>>`

- `None` → copy all top-level folders (existing behavior, backward compatible)
- `Some(list)` → skip any top-level folder whose name is not in the list

The file walk already operates folder-by-folder; the exclusion check is added at the top-level folder boundary.

### Store: `cardReaderStore.copyFiles()`

Add `includedFolders?: string[]` parameter, passed through to the Rust command as `included_folders: includedFolders ?? null`.

### `ReaderDashboardCard` calls `startCopy()`

Derives `includedFolders` from the toggle state before calling `cardReaderStore.copyFiles()`:

```ts
const includedFolders = Object.entries(folderToggles.value)
  .filter(([, included]) => included)
  .map(([name]) => name);
```

---

## Files Changed

| File | Change |
|------|--------|
| `src/types/index.ts` | No changes needed (DirectoryEntry already has file_count) |
| `src/stores/session.ts` | Add `requireSubfolder` to `ReaderMapping` + `setReaderMapping` |
| `src/stores/cardReader.ts` | Add `includedFolders?` param to `copyFiles()` |
| `src/components/ReaderSetup.vue` | Add `requireSubfolder` checkbox |
| `src/components/ReaderDashboardCard.vue` | Folder list UI, toggles, require-subfolder hint, auto-clear, pass includedFolders to copy |
| `src-tauri/src/commands.rs` | Add `included_folders: Option<Vec<String>>` to `copy_files_to_destination` |

---

## Out of Scope

- Persisting folder toggle state across sessions
- Per-reader folder exclusion config (e.g., "always ignore slot 100+")
- Any server-side changes
