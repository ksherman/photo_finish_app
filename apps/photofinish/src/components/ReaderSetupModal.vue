<script setup lang="ts">
import { ref, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { ask, open } from "@tauri-apps/plugin-dialog";
import { useEventStore } from "@/stores/event";
import { useIngestStore } from "@/stores/ingest";
import type { CardReaderInfo, DirectoryEntry } from "@/types";

const props = defineProps<{
  reader: CardReaderInfo;
  defaultGroup?: string | null;
}>();

const emit = defineEmits<{
  close: [];
  saved: [];
}>();

const eventStore = useEventStore();
const ingestStore = useIngestStore();

const customLabel = ref("");
const groupName = ref("");
const destination = ref("");
const photographer = ref("");
const renamePrefix = ref("");
const autoRename = ref(false);
const requireSubfolder = ref(false);
const fileRenamePrefix = ref("");
const hasExistingConfig = ref(false);

const previewFileCount = ref(0);
const previewFolderCount = ref(0);

onMounted(() => {
  const existing = ingestStore.getReaderConfig(props.reader.reader_id);
  if (existing) {
    hasExistingConfig.value = true;
    customLabel.value = existing.customLabel || "";
    groupName.value = existing.groupName || "";
    destination.value = existing.destination;
    photographer.value = existing.photographer;
    renamePrefix.value = existing.renamePrefix || "";
    autoRename.value = existing.autoRename;
    requireSubfolder.value = existing.requireSubfolder;
    fileRenamePrefix.value = existing.fileRenamePrefix || "";
  } else if (props.defaultGroup) {
    groupName.value = props.defaultGroup;
  }

  if (props.reader.volume_name !== "Offline") {
    loadPreview();
  }
});

async function loadPreview() {
  try {
    const entries = await invoke<DirectoryEntry[]>("list_directory", {
      path: props.reader.mount_point,
      autoEnterDcim: true,
    });
    const dirs = entries.filter((e) => e.is_directory);
    previewFolderCount.value = dirs.length;
    previewFileCount.value = dirs.reduce((sum, d) => sum + d.file_count, 0);
  } catch {
    previewFileCount.value = 0;
    previewFolderCount.value = 0;
  }
}

async function browseSubfolderInEvent() {
  const defaultPath = eventStore.activeEvent?.rootPath;
  const selected = await open({
    directory: true,
    multiple: false,
    defaultPath,
    title: "Select Destination Folder Inside Event Bundle",
  });
  if (typeof selected === "string") {
    if (defaultPath && selected.startsWith(defaultPath)) {
      destination.value = selected.slice(defaultPath.length).replace(/^\/+/, "");
    } else {
      destination.value = selected;
    }
  }
}

async function save() {
  const existing = ingestStore.getReaderConfig(props.reader.reader_id);
  const nextGroup = groupName.value.trim() || null;

  // Compute default sortOrder at the end of the target group if new or changing groups
  let sortOrder = existing?.sortOrder ?? 0;
  if (!existing || (existing.groupName || null) !== nextGroup) {
    const siblings = Object.values(ingestStore.readerConfigs).filter(
      (c) => (c.groupName || null) === nextGroup && c.readerId !== props.reader.reader_id
    );
    sortOrder =
      siblings.length > 0 ? Math.max(...siblings.map((s) => s.sortOrder)) + 1 : 0;
  }

  await ingestStore.saveReaderConfig({
    readerId: props.reader.reader_id,
    displayName: props.reader.display_name,
    customLabel: customLabel.value.trim() || null,
    groupName: nextGroup,
    sortOrder,
    destination: destination.value.trim().replace(/^\/+|\/+$/g, ""),
    photographer: photographer.value.trim(),
    cameraBrand: "auto",
    cameraFolderPath: "",
    renamePrefix: renamePrefix.value.trim(),
    autoRename: autoRename.value,
    fileRenamePrefix: fileRenamePrefix.value.trim() || null,
    requireSubfolder: requireSubfolder.value,
    currentOrder: existing?.currentOrder ?? 1,
  });
  emit("saved");
}

async function removeThisReader() {
  const label = customLabel.value.trim() || props.reader.display_name;
  const ok = await ask(`Remove saved configuration for "${label}" (${props.reader.reader_id})?`, {
    title: "Remove Reader Config",
    kind: "warning",
  });
  if (ok) {
    await ingestStore.deleteReaderConfig(props.reader.reader_id);
    emit("saved");
  }
}
</script>

<template>
  <div class="bg-white rounded-xl shadow-xl overflow-hidden flex flex-col max-h-[90vh]">
    <div class="px-6 py-4 border-b border-slate-100 flex justify-between items-center bg-slate-50">
      <h2 class="text-lg font-semibold text-slate-800">
        Configure {{ customLabel.trim() || reader.display_name }}
      </h2>
      <button @click="emit('close')" class="text-slate-400 hover:text-slate-600 text-xl leading-none">&times;</button>
    </div>

    <div class="p-6 overflow-y-auto space-y-4">
      <div class="bg-blue-50 p-3 rounded-lg text-xs text-blue-900 space-y-0.5">
        <div class="font-semibold">Locked Hardware Identity</div>
        <div>Device: {{ reader.display_name }} · Volume: {{ reader.volume_name }}</div>
        <div class="font-mono text-blue-700 break-all">ID: {{ reader.reader_id }}</div>
        <div class="text-blue-700/80 pt-0.5">
          Group, label, left-to-right position, and copy settings stay permanently locked to this physical reader.
        </div>
      </div>

      <!-- Label & Group Row -->
      <div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
        <div>
          <label class="block text-sm font-medium text-slate-700 mb-1">
            Reader Label (Optional)
          </label>
          <input
            v-model="customLabel"
            type="text"
            placeholder="e.g., Floor, Beam, Vault"
            class="w-full border border-slate-300 rounded-lg px-3 py-2 text-sm"
          />
          <p class="text-xs text-slate-400 mt-1">Shown as the card title.</p>
        </div>

        <div>
          <label class="block text-sm font-medium text-slate-700 mb-1">
            Group / Area (Optional)
          </label>
          <input
            v-model="groupName"
            list="pf-reader-group-suggestions"
            type="text"
            placeholder="e.g., Gym A, Gym B, Stage 1"
            class="w-full border border-slate-300 rounded-lg px-3 py-2 text-sm"
          />
          <datalist id="pf-reader-group-suggestions">
            <option v-for="g in ingestStore.groupOrder" :key="g" :value="g" />
          </datalist>
          <p class="text-xs text-slate-400 mt-1">Groups related readers side-by-side.</p>
        </div>
      </div>

      <div>
        <label class="block text-sm font-medium text-slate-700 mb-1">
          Relative Destination Inside Active Event
        </label>
        <div class="flex gap-2">
          <input
            v-model="destination"
            type="text"
            placeholder="e.g., Gym A/Session 1A/Group 1A/Floor"
            class="flex-1 border border-slate-300 rounded-lg px-3 py-2 text-sm"
          />
          <button
            v-if="eventStore.activeEvent"
            type="button"
            @click="browseSubfolderInEvent"
            class="px-3 py-2 bg-slate-200 hover:bg-slate-300 rounded-lg text-sm font-medium"
          >
            Browse
          </button>
        </div>
        <p class="text-xs text-slate-500 mt-1">
          Stored relative to the active Event Bundle so the same reader config works across meets.
        </p>
      </div>

      <div>
        <label class="block text-sm font-medium text-slate-700 mb-1">
          Photographer Initials (Optional)
        </label>
        <input
          v-model="photographer"
          type="text"
          placeholder="e.g., KDS"
          class="w-full border border-slate-300 rounded-lg px-3 py-2 text-sm"
        />
        <p class="text-xs text-slate-400 mt-1">
          Saved on this reader config and stamped onto every ingested card batch &amp; photo.
        </p>
      </div>

      <div
        v-if="previewFileCount > 0 || previewFolderCount > 0"
        class="bg-emerald-50 border border-emerald-200 rounded-lg p-3 text-sm text-emerald-800"
      >
        Detected {{ previewFileCount.toLocaleString() }} photos in {{ previewFolderCount }} folder{{ previewFolderCount === 1 ? '' : 's' }} on card (auto-detects <code>DCIM/</code>).
      </div>

      <div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
        <div>
          <label class="block text-sm font-medium text-slate-700 mb-1">File Rename Prefix (Optional)</label>
          <input
            v-model="fileRenamePrefix"
            type="text"
            placeholder="e.g., Beam"
            class="w-full border border-slate-300 rounded-lg px-3 py-2 text-sm"
          />
        </div>

        <div>
          <label class="block text-sm font-medium text-slate-700 mb-1">Folder Rename Prefix (Optional)</label>
          <input
            v-model="renamePrefix"
            type="text"
            placeholder="e.g., Gymnast"
            class="w-full border border-slate-300 rounded-lg px-3 py-2 text-sm"
          />
        </div>
      </div>

      <div class="space-y-2">
        <label class="flex items-center gap-2 cursor-pointer">
          <input v-model="autoRename" type="checkbox" :disabled="!renamePrefix" class="rounded" />
          <span class="text-sm text-slate-700">Auto-rename camera folders during copy (e.g. 105MSDCF → {{ renamePrefix || 'Gymnast' }} 05)</span>
        </label>

        <div>
          <label class="flex items-center gap-2 cursor-pointer">
            <input v-model="requireSubfolder" type="checkbox" class="rounded" />
            <span class="text-sm text-slate-700">Require sub-folder before copying</span>
          </label>
          <p class="text-xs text-slate-500 mt-0.5 ml-6">
            Prevents copying until a rotation/apparatus sub-folder is entered. Clears automatically after each copy.
          </p>
        </div>
      </div>
    </div>

    <div class="px-6 py-4 border-t border-slate-100 bg-slate-50 flex items-center justify-between gap-3">
      <button
        v-if="hasExistingConfig"
        type="button"
        @click="removeThisReader"
        class="px-3 py-2 text-xs font-medium text-red-600 hover:bg-red-50 rounded-lg"
      >
        Remove Saved Reader
      </button>
      <div v-else />
      <div class="flex items-center gap-2">
        <button @click="emit('close')" class="px-4 py-2 text-sm text-slate-600 hover:text-slate-800">
          Cancel
        </button>
        <button
          @click="save"
          class="px-4 py-2 text-sm bg-blue-600 text-white rounded-lg hover:bg-blue-700 font-medium"
        >
          Save Configuration
        </button>
      </div>
    </div>
  </div>
</template>
