# Ingest App: Require Subfolder + Folder Listing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a "require subfolder" config toggle and a per-folder directory listing with include/ignore toggles to the Tauri ingest app's reader cards.

**Architecture:** Rust backend gains an `included_folders` filter on `copy_files_to_destination`. Vue frontend loads the camera folder list into `ReaderDashboardCard` as local state, renders it as a scrollable checklist with a rename preview, and derives the included set before invoking copy.

**Tech Stack:** Rust/Tauri 2, Vue 3 (Composition API), Pinia, TypeScript, Tailwind CSS

---

## Files Changed

| File | Change |
|------|--------|
| `apps/ingest/src-tauri/src/commands.rs` | Add `included_folders: Option<Vec<String>>` param to `copy_files_to_destination` |
| `apps/ingest/src/stores/session.ts` | Add `requireSubfolder: boolean` to `ReaderMapping` + `setReaderMapping` |
| `apps/ingest/src/stores/cardReader.ts` | Add `includedFolders?: string[]` param to `copyFiles()` |
| `apps/ingest/src/components/ReaderSetup.vue` | Add `requireSubfolder` checkbox |
| `apps/ingest/src/components/ReaderDashboardCard.vue` | Folder list UI, toggles, require-subfolder hint, auto-clear, pass includedFolders |

---

## Task 1: Rust — Add `included_folders` filter to `copy_files_to_destination`

**Files:**
- Modify: `apps/ingest/src-tauri/src/commands.rs`

- [ ] **Step 1: Add `included_folders` parameter and filter logic**

Replace the `copy_files_to_destination` signature and file-collection block in `commands.rs`. The full updated function:

```rust
#[tauri::command]
pub async fn copy_files_to_destination(
    source_path: String,
    destination_path: String,
    on_progress: Channel<CopyProgressEvent>,
    rename_prefix: Option<String>,
    auto_rename: bool,
    file_rename_prefix: Option<String>,
    included_folders: Option<Vec<String>>,
) -> Result<u32, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let source = Path::new(&source_path);
        let dest = Path::new(&destination_path);

        // Compile regex once
        let re_folder = regex::Regex::new(r"(\d{2,3})").map_err(|e| e.to_string())?;
        let re_file = regex::Regex::new(r"(\d+)").map_err(|e| e.to_string())?;

        // Create destination directory
        fs::create_dir_all(dest).map_err(|e| e.to_string())?;

        // Collect all JPEG files
        let files: Vec<_> = WalkDir::new(source)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                // Ignore hidden files
                if e.file_name().to_string_lossy().starts_with('.') {
                    return false;
                }
                // Filter by included folders if specified
                if let Some(ref folders) = included_folders {
                    let relative = e.path().strip_prefix(source).unwrap_or(e.path());
                    let mut components = relative.components();
                    if let Some(first) = components.next() {
                        if components.next().is_some() {
                            // File is inside a top-level subfolder — check if it's included
                            let folder_name = first.as_os_str().to_string_lossy().to_string();
                            if !folders.contains(&folder_name) {
                                return false;
                            }
                        }
                    }
                }
                // JPEG extension check
                e.path()
                    .extension()
                    .map(|ext| {
                        let ext_lower = ext.to_string_lossy().to_lowercase();
                        ext_lower == "jpg" || ext_lower == "jpeg"
                    })
                    .unwrap_or(false)
            })
            .map(|e| e.path().to_path_buf())
            .collect();

        let total = files.len() as u32;
        let mut copied = 0u32;
        let mut last_update = std::time::Instant::now();

        for file_path in files {
            // Preserve relative path structure
            let relative = file_path.strip_prefix(source).unwrap_or(&file_path);

            // Apply folder renaming if enabled
            let mut dest_path_buf = if auto_rename && rename_prefix.is_some() {
                let prefix = rename_prefix.as_ref().unwrap();
                let renamed_path = apply_folder_rename(relative, prefix, &re_folder);
                dest.join(renamed_path)
            } else {
                dest.join(relative)
            };

            // Apply file renaming if enabled
            if let Some(prefix) = &file_rename_prefix {
                if let Some(file_name) = dest_path_buf.file_name().and_then(|n| n.to_str()) {
                    if let Some(caps) = re_file.find_iter(file_name).last() {
                        let number = caps.as_str();
                        let ext = dest_path_buf.extension().and_then(|e| e.to_str()).unwrap_or("");

                        let new_name = if ext.is_empty() {
                            format!("{}_{}", prefix, number)
                        } else {
                            format!("{}_{}.{}", prefix, number, ext)
                        };

                        dest_path_buf.set_file_name(new_name);
                    }
                }
            }

            // Create parent directories
            if let Some(parent) = dest_path_buf.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }

            // Copy file
            fs::copy(&file_path, &dest_path_buf).map_err(|e| e.to_string())?;

            copied += 1;

            // Send progress update (throttle to every 100ms or completion)
            if copied == total || last_update.elapsed().as_millis() > 100 {
                let _ = on_progress.send(CopyProgressEvent {
                    total,
                    copied,
                    current_file: dest_path_buf
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                    percentage: (copied as f32 / total as f32) * 100.0,
                });
                last_update = std::time::Instant::now();
            }
        }

        Ok(copied)
    })
    .await
    .map_err(|e| e.to_string())?
}
```

- [ ] **Step 2: Verify it compiles**

```bash
cd apps/ingest/src-tauri && cargo build 2>&1 | tail -5
```

Expected: `Finished` with no errors. Fix any compile errors before proceeding.

- [ ] **Step 3: Commit**

```bash
git add apps/ingest/src-tauri/src/commands.rs
git commit -m "feat(ingest): add included_folders filter to copy command"
```

---

## Task 2: Session store — Add `requireSubfolder`

**Files:**
- Modify: `apps/ingest/src/stores/session.ts`

- [ ] **Step 1: Add `requireSubfolder` to the interface and update `setReaderMapping`**

Replace the `ReaderMapping` interface and `setReaderMapping` action with:

```ts
export interface ReaderMapping {
  readerId: string;
  displayName: string;
  destination: string;
  photographer: string;
  currentOrder: number;
  cameraBrand: string;
  cameraFolderPath: string;
  renamePrefix: string;
  autoRename: boolean;
  fileRenamePrefix?: string;
  requireSubfolder: boolean;
}
```

And update `setReaderMapping` in the `actions` block:

```ts
setReaderMapping(
  readerId: string,
  displayName: string,
  destination: string,
  photographer: string,
  cameraBrand: string,
  cameraFolderPath: string,
  renamePrefix: string,
  autoRename: boolean,
  fileRenamePrefix?: string,
  requireSubfolder: boolean = false
) {
  const existing = this.readerMappings[readerId];
  this.readerMappings[readerId] = {
    readerId,
    displayName,
    destination,
    photographer,
    cameraBrand,
    cameraFolderPath,
    renamePrefix,
    autoRename,
    fileRenamePrefix,
    requireSubfolder,
    currentOrder:
      existing && existing.destination === destination
        ? existing.currentOrder
        : 1,
  };
},
```

- [ ] **Step 2: Commit**

```bash
git add apps/ingest/src/stores/session.ts
git commit -m "feat(ingest): add requireSubfolder to ReaderMapping"
```

---

## Task 3: Card reader store — Add `includedFolders` to `copyFiles`

**Files:**
- Modify: `apps/ingest/src/stores/cardReader.ts`

- [ ] **Step 1: Add `includedFolders` parameter**

Update the `copyFiles` action signature and the `invoke` call:

```ts
async copyFiles(
  readerId: string,
  destinationPath: string,
  cameraFolderPath?: string,
  renamePrefix?: string,
  autoRename?: boolean,
  fileRenamePrefix?: string,
  includedFolders?: string[]
): Promise<number> {
  const reader = this.cardReaders.find(r => r.reader_id === readerId);
  if (!reader) throw new Error("Reader not found");

  this.copyStates[readerId] = {
    isCopying: true,
    progress: {
      total: 0,
      copied: 0,
      current_file: "",
      percentage: 0,
    },
    error: null,
    lastResult: null
  };

  let sourcePath = reader.mount_point;
  if (cameraFolderPath) {
    sourcePath = `${sourcePath}/${cameraFolderPath}`;
  }

  try {
    const startTime = performance.now();
    const channel = new Channel<CopyProgressEvent>();
    channel.onmessage = (progress) => {
      if (this.copyStates[readerId]) {
        this.copyStates[readerId].progress = progress;
      }
    };

    const copiedCount = await invoke<number>("copy_files_to_destination", {
      sourcePath,
      destinationPath,
      onProgress: channel,
      renamePrefix: renamePrefix || null,
      autoRename: autoRename || false,
      fileRenamePrefix: fileRenamePrefix || null,
      includedFolders: includedFolders ?? null,
    });

    const durationSeconds = (performance.now() - startTime) / 1000;
    this.copyStates[readerId].lastResult = { count: copiedCount, success: true, durationSeconds };
    return copiedCount;

  } catch (error) {
    const msg = String(error);
    if (this.copyStates[readerId]) {
      this.copyStates[readerId].error = msg;
      this.copyStates[readerId].lastResult = { count: 0, success: false, durationSeconds: 0 };
    }
    throw error;
  } finally {
    if (this.copyStates[readerId]) {
      this.copyStates[readerId].isCopying = false;
    }
  }
},
```

- [ ] **Step 2: Commit**

```bash
git add apps/ingest/src/stores/cardReader.ts
git commit -m "feat(ingest): pass includedFolders through to copy command"
```

---

## Task 4: ReaderSetup — Add `requireSubfolder` checkbox

**Files:**
- Modify: `apps/ingest/src/components/ReaderSetup.vue`

- [ ] **Step 1: Add `requireSubfolder` ref**

In the `<script setup>` block, add after `const autoRename = ref(false)`:

```ts
const requireSubfolder = ref(false);
```

- [ ] **Step 2: Initialize from existing mapping in `onMounted`**

Inside the `if (existing)` block in `onMounted`, add after `autoRename.value = existing.autoRename || false`:

```ts
requireSubfolder.value = existing.requireSubfolder || false;
```

In the `else` block, add:

```ts
requireSubfolder.value = false;
```

- [ ] **Step 3: Pass `requireSubfolder` to `save()`**

Update the `sessionStore.setReaderMapping(...)` call in `save()` to pass `requireSubfolder.value` as the final argument:

```ts
sessionStore.setReaderMapping(
  props.reader.reader_id,
  props.reader.display_name,
  destination.value,
  photographer.value,
  cameraBrand.value,
  selectedCameraBrand.value.folderPath,
  renamePrefix.value,
  autoRename.value,
  fileRenamePrefix.value,
  requireSubfolder.value
);
```

- [ ] **Step 4: Add checkbox to template**

Add this block after the folder rename section (after the `autoRename` checkbox `<label>`) and before the closing `</div>` of the `space-y-4` div:

```html
<div>
  <label class="flex items-center cursor-pointer">
    <input
      v-model="requireSubfolder"
      type="checkbox"
      class="w-4 h-4 text-blue-600 border-gray-300 rounded focus:ring-blue-500"
    />
    <span class="ml-2 text-sm text-gray-700">
      Require sub-folder before copying
    </span>
  </label>
  <p class="text-xs text-gray-500 mt-1 ml-6">
    Prevents copying until a sub-folder name is entered. Clears automatically after each copy.
  </p>
</div>
```

- [ ] **Step 5: Commit**

```bash
git add apps/ingest/src/components/ReaderSetup.vue
git commit -m "feat(ingest): add requireSubfolder config option to reader setup"
```

---

## Task 5: ReaderDashboardCard — Folder listing, toggles, require-subfolder, auto-clear

**Files:**
- Modify: `apps/ingest/src/components/ReaderDashboardCard.vue`

This task rewrites the component's `<script setup>` section and updates the template. Replace the entire file with the following:

- [ ] **Step 1: Replace `<script setup>` section**

```vue
<script setup lang="ts">
import { ref, computed, onMounted, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useSessionStore } from "@/stores/session";
import { useCardReaderStore } from "@/stores/cardReader";
import type { CardReaderInfo, DirectoryEntry } from "@/types";

const props = defineProps<{
  reader: CardReaderInfo;
}>();

const emit = defineEmits<{
  configure: [reader: CardReaderInfo];
}>();

const sessionStore = useSessionStore();
const cardReaderStore = useCardReaderStore();
const customSubPath = ref("");
const folderEntries = ref<DirectoryEntry[]>([]);
const folderToggles = ref<Record<string, boolean>>({});

const mapping = computed(() =>
  sessionStore.getReaderMapping(props.reader.reader_id)
);

const copyState = computed(() =>
  cardReaderStore.getCopyState(props.reader.reader_id)
);

const isConfigured = computed(() => !!mapping.value);
const isOffline = computed(() => props.reader.volume_name === "Offline");

const cleanSubPath = computed(() =>
  customSubPath.value.trim().replace(/^\/+|\/+$/g, "")
);

const subfolderRequired = computed(
  () => !!mapping.value?.requireSubfolder && !cleanSubPath.value
);

const canCopy = computed(
  () =>
    !isOffline.value &&
    isConfigured.value &&
    !copyState.value.isCopying &&
    props.reader.file_count > 0 &&
    !subfolderRequired.value
);

const destinationDisplay = computed(() => {
  if (!mapping.value) return "";
  return cleanSubPath.value
    ? `${mapping.value.destination}/${cleanSubPath.value}`
    : mapping.value.destination;
});

function previewFolderName(name: string, prefix: string): string {
  const match = name.match(/(\d{2,3})/);
  if (!match) return name;
  const lastTwo = match[1].slice(-2);
  return `${prefix} ${lastTwo}`;
}

async function loadFolderList() {
  if (!mapping.value || isOffline.value) return;
  const basePath = mapping.value.cameraFolderPath
    ? `${props.reader.mount_point}/${mapping.value.cameraFolderPath}`
    : props.reader.mount_point;
  try {
    const entries = await invoke<DirectoryEntry[]>("list_directory", { path: basePath });
    folderEntries.value = entries.filter((e) => e.is_directory);
    const defaults: Record<string, boolean> = {};
    for (const entry of folderEntries.value) {
      defaults[entry.name] = entry.file_count > 0;
    }
    folderToggles.value = defaults;
  } catch {
    folderEntries.value = [];
    folderToggles.value = {};
  }
}

function selectAll() {
  for (const key of Object.keys(folderToggles.value)) {
    folderToggles.value[key] = true;
  }
}

function resetDefaults() {
  for (const entry of folderEntries.value) {
    folderToggles.value[entry.name] = entry.file_count > 0;
  }
}

async function startCopy() {
  if (!canCopy.value || !mapping.value) return;

  let fullPath = mapping.value.destination;
  if (cleanSubPath.value) {
    fullPath = `${fullPath}/${cleanSubPath.value}`;
  }

  const includedFolders = Object.entries(folderToggles.value)
    .filter(([, included]) => included)
    .map(([name]) => name);

  try {
    await cardReaderStore.copyFiles(
      props.reader.reader_id,
      fullPath,
      mapping.value.cameraFolderPath,
      mapping.value.renamePrefix,
      mapping.value.autoRename,
      mapping.value.fileRenamePrefix,
      includedFolders.length > 0 ? includedFolders : undefined
    );

    customSubPath.value = "";
    sessionStore.incrementOrderForReader(props.reader.reader_id);
    await loadFolderList();
  } catch (error) {
    console.error("Copy failed:", error);
  }
}

onMounted(() => {
  loadFolderList();
});

watch(
  () => props.reader.volume_name,
  (newVal) => {
    if (newVal !== "Offline") {
      loadFolderList();
    }
  }
);
</script>
```

- [ ] **Step 2: Replace `<template>` section**

```vue
<template>
  <div class="bg-white rounded-lg shadow border border-gray-200 flex flex-col h-full">
    <!-- Header -->
    <div class="p-4 border-b border-gray-100 bg-gray-50 rounded-t-lg">
      <div class="flex justify-between items-start">
        <div>
          <h3 class="font-semibold text-gray-800" :class="{ 'text-gray-500': isOffline }">{{ reader.display_name }}</h3>
          <p class="text-xs text-gray-500 font-mono mt-1" v-if="!isOffline">{{ reader.volume_name }}</p>
          <p class="text-xs text-red-500 font-medium mt-1" v-else>Reader Disconnected</p>
        </div>
        <span
          v-if="!isOffline"
          class="px-2 py-1 text-xs font-medium rounded-full"
          :class="isConfigured ? 'bg-green-100 text-green-700' : 'bg-yellow-100 text-yellow-700'"
        >
          {{ isConfigured ? 'Ready' : 'Setup Needed' }}
        </span>
        <span
          v-else
          class="px-2 py-1 text-xs font-medium rounded-full bg-gray-200 text-gray-600"
        >
          Offline
        </span>
      </div>
    </div>

    <!-- Body -->
    <div class="p-4 flex-1 space-y-3 overflow-y-auto">
      <div v-if="isConfigured && mapping">
        <div class="grid grid-cols-2 gap-2 text-sm mb-3">
          <div>
            <span class="text-gray-500 block text-xs">Photographer</span>
            <span class="font-medium text-gray-900">{{ mapping.photographer }}</span>
          </div>
          <div v-if="!isOffline">
            <span class="text-gray-500 block text-xs">Files on Card</span>
            <span class="font-medium text-gray-900">
              {{ reader.file_count }}
              <span class="text-gray-400 font-normal ml-1 text-xs">in {{ reader.folder_count }} folders</span>
            </span>
          </div>
        </div>

        <!-- Folder Listing -->
        <div v-if="!isOffline && folderEntries.length > 0" class="mt-1">
          <div class="flex items-center justify-between text-xs mb-1">
            <span class="font-medium text-gray-700">Folders</span>
            <div class="flex gap-2 text-gray-400">
              <button @click="selectAll" class="hover:text-blue-600 transition-colors">All</button>
              <span>·</span>
              <button @click="resetDefaults" class="hover:text-blue-600 transition-colors">Reset</button>
            </div>
          </div>
          <div class="overflow-y-auto max-h-44 border border-gray-200 rounded-md divide-y divide-gray-100">
            <label
              v-for="entry in folderEntries"
              :key="entry.name"
              class="flex items-center gap-2 px-2 py-1.5 cursor-pointer hover:bg-gray-50 transition-colors"
              :class="{ 'opacity-50': !folderToggles[entry.name] }"
            >
              <input
                type="checkbox"
                :checked="folderToggles[entry.name]"
                @change="folderToggles[entry.name] = ($event.target as HTMLInputElement).checked"
                class="w-3.5 h-3.5 text-blue-600 border-gray-300 rounded flex-shrink-0"
                :disabled="copyState.isCopying"
              />
              <span class="flex-1 text-xs font-mono truncate text-gray-800 min-w-0">
                {{ entry.name }}
                <span
                  v-if="mapping.autoRename && mapping.renamePrefix"
                  class="text-gray-400 not-italic"
                >
                  → {{ previewFolderName(entry.name, mapping.renamePrefix) }}
                </span>
              </span>
              <span
                class="text-xs flex-shrink-0 font-medium"
                :class="entry.file_count === 0 ? 'text-yellow-500' : 'text-gray-400'"
              >
                {{ entry.file_count === 0 ? '⚠ 0' : entry.file_count }}
              </span>
            </label>
          </div>
        </div>

        <!-- Destination Preview -->
        <div class="text-sm bg-gray-50 p-2 rounded border border-gray-100">
          <span class="text-gray-500 block text-xs mb-1">Destination</span>
          <code class="text-xs text-blue-600 break-all block leading-tight">{{ destinationDisplay }}</code>
        </div>

        <!-- Sub-Folder Input -->
        <div class="pt-2 border-t border-dashed border-gray-200">
          <label class="block text-xs font-medium text-gray-700 mb-1">
            Sub-folder
            <span v-if="mapping.requireSubfolder" class="text-red-500 ml-1">*</span>
          </label>
          <input
            v-model="customSubPath"
            type="text"
            placeholder="e.g. Group 8B/Floor"
            class="w-full text-sm border rounded-md px-2 py-1.5 focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
            :class="subfolderRequired ? 'border-amber-400' : 'border-gray-300'"
            :disabled="copyState.isCopying"
          />
          <p v-if="subfolderRequired" class="text-xs text-amber-600 mt-1">
            Sub-folder required before copying
          </p>
        </div>
      </div>

      <div v-else class="text-center py-6 text-gray-500 text-sm">
        <p v-if="!isOffline">Configure this reader to start ingesting photos.</p>
        <p v-else>Reader is offline but can be configured.</p>
      </div>

      <!-- Progress / Status -->
      <div v-if="copyState.isCopying" class="mt-4">
        <div class="flex justify-between text-xs mb-1">
          <span class="text-blue-600 font-medium">Copying...</span>
          <span class="text-gray-600">{{ Math.round(copyState.progress?.percentage || 0) }}%</span>
        </div>
        <div class="w-full bg-gray-200 rounded-full h-2">
          <div
            class="bg-blue-600 h-2 rounded-full transition-all duration-300"
            :style="{ width: `${copyState.progress?.percentage || 0}%` }"
          ></div>
        </div>
        <p class="text-xs text-gray-500 mt-1 truncate">
          {{ copyState.progress?.copied }} / {{ copyState.progress?.total }} files
        </p>
      </div>

      <div v-if="copyState.lastResult?.success" class="mt-4 bg-green-50 p-2 rounded text-xs text-green-700 border border-green-100 flex items-center justify-between">
        <span>Copied {{ copyState.lastResult.count }} files in {{ copyState.lastResult.durationSeconds.toFixed(1) }}s</span>
        <button @click="cardReaderStore.resetCopyState(reader.reader_id)" class="ml-2 text-green-500 hover:text-green-700 flex-shrink-0" title="Dismiss">
          <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      <div v-if="copyState.error" class="mt-4 bg-red-50 p-2 rounded text-xs text-red-700 border border-red-100 flex items-center justify-between">
        <span>Error: {{ copyState.error }}</span>
        <button @click="cardReaderStore.resetCopyState(reader.reader_id)" class="ml-2 text-red-500 hover:text-red-700 flex-shrink-0" title="Dismiss">
          <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>
    </div>

    <!-- Footer Actions -->
    <div class="p-4 border-t border-gray-100 bg-gray-50 rounded-b-lg flex gap-2">
      <button
        @click="emit('configure', reader)"
        class="flex-1 px-3 py-2 text-sm font-medium text-gray-700 bg-white border border-gray-300 rounded-md hover:bg-gray-50 transition-colors"
      >
        {{ isConfigured ? 'Edit Config' : 'Configure' }}
      </button>

      <button
        v-if="isConfigured"
        @click="startCopy"
        :disabled="!canCopy"
        class="flex-1 px-3 py-2 text-sm font-medium text-white rounded-md transition-colors"
        :class="canCopy ? 'bg-blue-600 hover:bg-blue-700 shadow-sm' : 'bg-gray-400 cursor-not-allowed'"
      >
        {{ copyState.isCopying ? 'Copying' : 'Copy Files' }}
      </button>
    </div>
  </div>
</template>
```

- [ ] **Step 3: Verify TypeScript compiles**

```bash
cd apps/ingest && npx vue-tsc --noEmit 2>&1 | tail -20
```

Expected: no errors. Fix any type errors before proceeding.

- [ ] **Step 4: Commit**

```bash
git add apps/ingest/src/components/ReaderDashboardCard.vue
git commit -m "feat(ingest): add folder listing with toggles, require-subfolder, and auto-clear"
```

---

## Task 6: End-to-end smoke test

- [ ] **Step 1: Start the dev server**

```bash
cd apps/ingest && pnpm tauri dev
```

- [ ] **Step 2: Verify folder listing**
  - Insert a card reader
  - Configure it with camera brand set
  - Card body should show the scrollable folder list with file counts
  - Empty folders should have `⚠ 0` badge and be unchecked
  - Non-empty folders should be checked

- [ ] **Step 3: Verify rename preview**
  - In reader config, set a folder rename prefix (e.g. "Gymnast") and enable auto-rename
  - Folder list should show `105MSDCF → Gymnast 05` style arrows

- [ ] **Step 4: Verify require subfolder**
  - In reader config, enable "Require sub-folder before copying"
  - On the card: Copy Files button should be disabled when subfolder input is empty
  - Amber "Sub-folder required" hint should appear below the input
  - Enter a subfolder name → button enables
  - After successful copy → subfolder input clears

- [ ] **Step 5: Verify folder exclusion**
  - Uncheck one or more folders in the list
  - Run a copy
  - Confirm the excluded folders' files do not appear in the destination

- [ ] **Step 6: Final commit if any fixups were needed**

```bash
git add -p && git commit -m "fix(ingest): smoke test fixups"
```
