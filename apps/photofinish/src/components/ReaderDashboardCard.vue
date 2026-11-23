<script setup lang="ts">
import { ref, computed, onMounted, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useEventStore } from "@/stores/event";
import { useIngestStore } from "@/stores/ingest";
import type { CardReaderInfo, DirectoryEntry } from "@/types";

const props = defineProps<{
  reader: CardReaderInfo;
  canMoveLeft?: boolean;
  canMoveRight?: boolean;
  isDragging?: boolean;
  dropSide?: "left" | "right" | null;
}>();

const emit = defineEmits<{
  configure: [reader: CardReaderInfo];
  dragStart: [reader: CardReaderInfo, event: PointerEvent];
  moveLeft: [reader: CardReaderInfo];
  moveRight: [reader: CardReaderInfo];
}>();

const eventStore = useEventStore();
const ingestStore = useIngestStore();

const customSubPath = ref("");
const folderEntries = ref<DirectoryEntry[]>([]);
const folderToggles = ref<Record<string, boolean>>({});
const isEjecting = ref(false);
const ejectError = ref<string | null>(null);
const lastEjected = ref(false);
const autoEjectAfterIngest = ref(
  localStorage.getItem("photofinish:autoEjectCard") !== "false"
);

watch(autoEjectAfterIngest, (val) => {
  localStorage.setItem("photofinish:autoEjectCard", String(val));
});

const config = computed(() => ingestStore.getReaderConfig(props.reader.reader_id));
const copyState = computed(() => ingestStore.getCopyState(props.reader.reader_id));
const isConfigured = computed(() => Boolean(config.value));
const isOffline = computed(() => props.reader.volume_name === "Offline");

const cardTitle = computed(
  () => config.value?.customLabel?.trim() || props.reader.display_name
);

const shortReaderId = computed(() => {
  const id = props.reader.reader_id;
  if (id === "sdxc-builtin") return "Built-in SDXC";
  if (id.length <= 12) return `ID ${id}`;
  return `SN …${id.slice(-8)}`;
});

const cleanSubPath = computed(() => customSubPath.value.trim().replace(/^\/+|\/+$/g, ""));
const subfolderRequired = computed(() => Boolean(config.value?.requireSubfolder && !cleanSubPath.value));

const destinationRelPath = computed(() => {
  const base = config.value?.destination?.trim().replace(/^\/+|\/+$/g, "") || "";
  if (base && cleanSubPath.value) return `${base}/${cleanSubPath.value}`;
  return base || cleanSubPath.value;
});

const canCopy = computed(
  () =>
    Boolean(eventStore.activeEvent) &&
    !isOffline.value &&
    isConfigured.value &&
    !copyState.value.isCopying &&
    props.reader.file_count > 0 &&
    !subfolderRequired.value &&
    (folderEntries.value.length === 0 || Object.values(folderToggles.value).some(Boolean))
);

function previewFolderName(name: string, prefix: string): string {
  const match = name.match(/(\d{2,3})/);
  if (!match) return name;
  return `${prefix} ${match[1].slice(-2)}`;
}

async function loadFolderList() {
  if (!config.value || isOffline.value) return;
  const basePath = config.value.cameraFolderPath
    ? `${props.reader.mount_point}/${config.value.cameraFolderPath}`
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

async function handleEject() {
  if (isOffline.value || !props.reader.disk_id || copyState.value.isCopying || isEjecting.value) {
    return;
  }
  isEjecting.value = true;
  ejectError.value = null;
  try {
    await ingestStore.ejectReader(props.reader.disk_id);
    lastEjected.value = true;
  } catch (err) {
    ejectError.value = String(err);
  } finally {
    isEjecting.value = false;
  }
}

async function startCopy() {
  if (!canCopy.value || !config.value) return;
  ejectError.value = null;
  lastEjected.value = false;

  const includedFolders = Object.entries(folderToggles.value)
    .filter(([, included]) => included)
    .map(([name]) => name);

  await ingestStore.ingestFromReader(
    props.reader,
    config.value,
    destinationRelPath.value,
    includedFolders.length > 0 ? includedFolders : undefined
  );

  customSubPath.value = "";
  await eventStore.refreshActiveEvent();

  if (autoEjectAfterIngest.value && props.reader.disk_id) {
    await handleEject();
  } else {
    await loadFolderList();
  }
}

onMounted(loadFolderList);
watch(
  () => [props.reader.volume_name, props.reader.mount_point, config.value?.cameraFolderPath],
  ([newVol]) => {
    if (newVol !== "Offline") {
      loadFolderList();
    } else {
      folderEntries.value = [];
      folderToggles.value = {};
    }
  }
);
</script>

<template>
  <div
    :data-drop-reader-id="reader.reader_id"
    class="relative bg-white rounded-xl shadow-sm border flex flex-col h-full transition-all"
    :class="[
      isDragging
        ? 'opacity-40 border-indigo-400 ring-2 ring-indigo-200 scale-[0.98]'
        : isOffline && isConfigured
          ? 'border-slate-200/80 bg-slate-50/40'
          : 'border-slate-200',
    ]"
  >
    <!-- Live Drop Position Indicators -->
    <div
      v-if="dropSide === 'left'"
      class="pointer-events-none absolute -left-2.5 top-2 bottom-2 w-1.5 rounded-full bg-indigo-500 shadow-sm z-20"
    />
    <div
      v-if="dropSide === 'right'"
      class="pointer-events-none absolute -right-2.5 top-2 bottom-2 w-1.5 rounded-full bg-indigo-500 shadow-sm z-20"
    />

    <!-- Card Header with Drag Handle, Order Nudge, Title, Eject & Status -->
    <div class="p-4 border-b border-slate-100 bg-slate-50 rounded-t-xl flex items-start justify-between gap-2">
      <div class="flex items-start gap-2 min-w-0 flex-1">
        <!-- Pointer Drag Handle -->
        <button
          type="button"
          @pointerdown.prevent="emit('dragStart', reader, $event)"
          title="Drag to reorder or move between groups"
          class="mt-0.5 px-1 py-1 -ml-1 rounded text-slate-400 hover:text-slate-700 hover:bg-slate-200/70 cursor-grab active:cursor-grabbing flex-shrink-0 select-none touch-none"
        >
          <svg class="w-3.5 h-4" viewBox="0 0 12 16" fill="currentColor" aria-hidden="true">
            <circle cx="3.5" cy="3" r="1.3" />
            <circle cx="8.5" cy="3" r="1.3" />
            <circle cx="3.5" cy="8" r="1.3" />
            <circle cx="8.5" cy="8" r="1.3" />
            <circle cx="3.5" cy="13" r="1.3" />
            <circle cx="8.5" cy="13" r="1.3" />
          </svg>
        </button>

        <div class="min-w-0 flex-1">
          <h3
            class="font-bold text-base leading-tight truncate"
            :class="isOffline ? 'text-slate-500' : 'text-slate-900'"
            :title="cardTitle"
          >
            {{ cardTitle }}
          </h3>
          <div class="text-xs mt-0.5 flex items-center gap-1.5 flex-wrap">
            <span v-if="!isOffline" class="font-mono text-slate-600 truncate max-w-[150px]">
              {{ reader.volume_name }}
            </span>
            <span v-else class="text-slate-400 font-medium">No card inserted</span>
            <span class="text-slate-300">·</span>
            <span class="font-mono text-[11px] text-slate-400" :title="reader.reader_id">
              {{ shortReaderId }}
            </span>
          </div>
        </div>
      </div>

      <div class="flex items-center gap-1.5 flex-shrink-0">
        <!-- Quick Left/Right Slot Nudge -->
        <div v-if="canMoveLeft || canMoveRight" class="inline-flex rounded-md border border-slate-200 bg-white overflow-hidden">
          <button
            type="button"
            @click="emit('moveLeft', reader)"
            :disabled="!canMoveLeft"
            title="Move left"
            class="px-1.5 py-0.5 text-xs text-slate-500 hover:bg-slate-100 hover:text-slate-900 disabled:opacity-30 disabled:cursor-not-allowed"
          >
            &larr;
          </button>
          <button
            type="button"
            @click="emit('moveRight', reader)"
            :disabled="!canMoveRight"
            title="Move right"
            class="px-1.5 py-0.5 text-xs text-slate-500 hover:bg-slate-100 hover:text-slate-900 border-l border-slate-200 disabled:opacity-30 disabled:cursor-not-allowed"
          >
            &rarr;
          </button>
        </div>

        <button
          v-if="!isOffline && reader.disk_id"
          @click="handleEject"
          :disabled="copyState.isCopying || isEjecting"
          title="Safely eject memory card so macOS doesn't complain when removing it"
          class="px-2 py-0.5 text-xs font-medium rounded-md border border-slate-300 bg-white text-slate-700 hover:bg-slate-100 disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
        >
          {{ isEjecting ? "Ejecting..." : "⏏ Eject" }}
        </button>

        <span
          class="px-2 py-0.5 text-xs font-medium rounded-full"
          :class="
            isOffline
              ? 'bg-slate-200 text-slate-600'
              : isConfigured
                ? 'bg-emerald-100 text-emerald-700'
                : 'bg-amber-100 text-amber-700'
          "
        >
          {{ isOffline ? "Waiting" : isConfigured ? "Ready" : "Setup" }}
        </span>
      </div>
    </div>

    <div class="p-4 flex-1 space-y-3 overflow-y-auto">
      <template v-if="isConfigured && config">
        <div class="grid grid-cols-2 gap-2 text-sm">
          <div>
            <span class="text-slate-500 block text-xs">Photographer</span>
            <span class="font-medium text-slate-900">{{ config.photographer }}</span>
          </div>
          <div v-if="!isOffline">
            <span class="text-slate-500 block text-xs">On Card</span>
            <span class="font-medium text-slate-900">
              {{ reader.file_count.toLocaleString() }}
              <span class="text-slate-400 font-normal text-xs">in {{ reader.folder_count }} dirs</span>
            </span>
          </div>
          <div v-else>
            <span class="text-slate-500 block text-xs">Status</span>
            <span class="text-xs text-slate-400">Insert SD card to ingest</span>
          </div>
        </div>

        <div v-if="!isOffline && folderEntries.length > 0">
          <div class="flex items-center justify-between text-xs mb-1">
            <span class="font-medium text-slate-700">Folders on Card</span>
            <div class="flex gap-2 text-slate-400">
              <button @click="selectAll" class="hover:text-blue-600">All</button>
              <span>·</span>
              <button @click="resetDefaults" class="hover:text-blue-600">Reset</button>
            </div>
          </div>
          <div class="overflow-y-auto max-h-40 border border-slate-200 rounded-md divide-y divide-slate-100">
            <label
              v-for="entry in folderEntries"
              :key="entry.name"
              class="flex items-center gap-2 px-2.5 py-1.5 cursor-pointer hover:bg-slate-50"
              :class="{ 'opacity-50': !folderToggles[entry.name] }"
            >
              <input
                type="checkbox"
                :checked="folderToggles[entry.name]"
                @change="folderToggles[entry.name] = ($event.target as HTMLInputElement).checked"
                :disabled="copyState.isCopying"
                class="rounded"
              />
              <span class="flex-1 text-xs font-mono truncate text-slate-800">
                {{ entry.name }}
                <span v-if="config.autoRename && config.renamePrefix" class="text-slate-400">
                  → {{ previewFolderName(entry.name, config.renamePrefix) }}
                </span>
              </span>
              <span class="text-xs font-medium" :class="entry.file_count === 0 ? 'text-amber-500' : 'text-slate-400'">
                {{ entry.file_count === 0 ? "0" : entry.file_count }}
              </span>
            </label>
          </div>
        </div>

        <div class="bg-slate-50 p-2.5 rounded-lg border border-slate-100">
          <span class="text-slate-500 block text-xs mb-0.5">Target Path (relative to event)</span>
          <code class="text-xs text-blue-600 break-all block">{{ destinationRelPath || "(event root)" }}</code>
        </div>

        <div class="pt-2 border-t border-dashed border-slate-200">
          <label class="block text-xs font-medium text-slate-700 mb-1">
            Sub-folder
            <span v-if="config.requireSubfolder" class="text-red-500">*</span>
          </label>
          <input
            v-model="customSubPath"
            type="text"
            placeholder="Scan barcode or type e.g. Group 8B/Beam"
            class="w-full text-sm border rounded-md px-2.5 py-1.5 bg-white"
            :class="subfolderRequired ? 'border-amber-400' : 'border-slate-300'"
            :disabled="copyState.isCopying"
            @keydown.enter.prevent="canCopy && startCopy()"
          />
          <p v-if="subfolderRequired" class="text-xs text-amber-600 mt-1">Sub-folder required before copying</p>
          <label class="mt-2 flex items-center gap-1.5 text-xs text-slate-600 cursor-pointer select-none">
            <input
              v-model="autoEjectAfterIngest"
              type="checkbox"
              :disabled="copyState.isCopying"
              class="rounded"
            />
            <span>Auto-eject card after ingest</span>
          </label>
        </div>
      </template>

      <div v-else class="text-center py-6 text-slate-500 text-sm space-y-1">
        <p>Configure this reader to lock its label, group, and destination path.</p>
        <p class="text-xs text-slate-400">Tip: You can also drag it directly into a Gym group above.</p>
      </div>

      <div v-if="copyState.isCopying" class="space-y-1">
        <div class="flex justify-between text-xs">
          <span class="text-blue-600 font-medium">Copying &amp; indexing...</span>
          <span class="text-slate-600">{{ Math.round(copyState.progress?.percentage || 0) }}%</span>
        </div>
        <div class="w-full bg-slate-200 rounded-full h-2">
          <div
            class="bg-blue-600 h-2 rounded-full transition-all"
            :style="{ width: `${copyState.progress?.percentage || 0}%` }"
          />
        </div>
        <p class="text-xs text-slate-500 truncate">
          {{ copyState.progress?.copied }} / {{ copyState.progress?.total }} files
        </p>
      </div>

      <div
        v-if="copyState.lastResult?.success"
        class="bg-emerald-50 p-2.5 rounded-lg text-xs text-emerald-700 border border-emerald-200 flex items-center justify-between"
      >
        <span>
          Copied {{ copyState.lastResult.copiedCount }} files ({{ copyState.lastResult.queuedCount }} queued) in
          {{ copyState.lastResult.durationSeconds.toFixed(1) }}s{{ lastEjected ? " · Safely ejected ✓" : "" }}
        </span>
        <button @click="ingestStore.resetCopyState(reader.reader_id); lastEjected = false" class="ml-2 text-emerald-600 font-bold">&times;</button>
      </div>

      <div
        v-if="ejectError"
        class="bg-amber-50 p-2.5 rounded-lg text-xs text-amber-800 border border-amber-200 flex items-center justify-between"
      >
        <span class="break-all">Eject failed: {{ ejectError }}</span>
        <button @click="ejectError = null" class="ml-2 text-amber-700 font-bold">&times;</button>
      </div>

      <div
        v-if="copyState.error"
        class="bg-red-50 p-2.5 rounded-lg text-xs text-red-700 border border-red-200 flex items-center justify-between"
      >
        <span class="break-all">{{ copyState.error }}</span>
        <button @click="ingestStore.resetCopyState(reader.reader_id)" class="ml-2 text-red-600 font-bold">&times;</button>
      </div>
    </div>

    <div class="p-3.5 border-t border-slate-100 bg-slate-50 rounded-b-xl flex gap-2">
      <button
        @click="emit('configure', reader)"
        class="flex-1 px-3 py-2 text-sm font-medium text-slate-700 bg-white border border-slate-300 rounded-lg hover:bg-slate-50"
      >
        {{ isConfigured ? "Edit Config" : "Configure" }}
      </button>
      <button
        v-if="isConfigured"
        @click="startCopy"
        :disabled="!canCopy"
        class="flex-1 px-3 py-2 text-sm font-semibold text-white rounded-lg"
        :class="canCopy ? 'bg-blue-600 hover:bg-blue-700' : 'bg-slate-400 cursor-not-allowed'"
      >
        {{ copyState.isCopying ? "Ingesting..." : "Ingest Card" }}
      </button>
    </div>
  </div>
</template>
