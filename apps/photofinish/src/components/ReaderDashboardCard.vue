<script setup lang="ts">
import { ref, computed, onMounted, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useEventStore } from "@/stores/event";
import { useIngestStore } from "@/stores/ingest";
import type { CardReaderInfo, DirectoryEntry } from "@/types";

const props = defineProps<{
  reader: CardReaderInfo;
}>();

const emit = defineEmits<{
  configure: [reader: CardReaderInfo];
}>();

const eventStore = useEventStore();
const ingestStore = useIngestStore();

const customSubPath = ref("");
const folderEntries = ref<DirectoryEntry[]>([]);
const folderToggles = ref<Record<string, boolean>>({});

const config = computed(() => ingestStore.getReaderConfig(props.reader.reader_id));
const copyState = computed(() => ingestStore.getCopyState(props.reader.reader_id));
const isConfigured = computed(() => Boolean(config.value));
const isOffline = computed(() => props.reader.volume_name === "Offline");

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

async function startCopy() {
  if (!canCopy.value || !config.value) return;

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
  await Promise.all([loadFolderList(), eventStore.refreshActiveEvent()]);
}

onMounted(loadFolderList);
watch(
  () => [props.reader.volume_name, props.reader.mount_point, config.value?.cameraFolderPath],
  ([newVol]) => {
    if (newVol !== "Offline") {
      loadFolderList();
    }
  }
);
</script>

<template>
  <div class="bg-white rounded-xl shadow-sm border border-slate-200 flex flex-col h-full">
    <div class="p-4 border-b border-slate-100 bg-slate-50 rounded-t-xl flex justify-between items-start">
      <div class="min-w-0">
        <h3 class="font-semibold text-slate-800 truncate" :class="{ 'text-slate-400': isOffline }">
          {{ reader.display_name }}
        </h3>
        <p v-if="!isOffline" class="text-xs text-slate-500 font-mono mt-0.5 truncate">
          {{ reader.volume_name }}
        </p>
        <p v-else class="text-xs text-red-500 font-medium mt-0.5">Reader Disconnected</p>
      </div>
      <span
        class="px-2 py-0.5 text-xs font-medium rounded-full flex-shrink-0"
        :class="
          isOffline
            ? 'bg-slate-200 text-slate-600'
            : isConfigured
              ? 'bg-emerald-100 text-emerald-700'
              : 'bg-amber-100 text-amber-700'
        "
      >
        {{ isOffline ? "Offline" : isConfigured ? "Ready" : "Setup Needed" }}
      </span>
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
              {{ reader.file_count }}
              <span class="text-slate-400 font-normal text-xs">in {{ reader.folder_count }} dirs</span>
            </span>
          </div>
        </div>

        <div v-if="!isOffline && folderEntries.length > 0">
          <div class="flex items-center justify-between text-xs mb-1">
            <span class="font-medium text-slate-700">Folders</span>
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
              class="flex items-center gap-2 px-2 py-1.5 cursor-pointer hover:bg-slate-50"
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

        <div class="bg-slate-50 p-2 rounded border border-slate-100">
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
            placeholder="e.g. Group 8B/Beam"
            class="w-full text-sm border rounded-md px-2.5 py-1.5"
            :class="subfolderRequired ? 'border-amber-400' : 'border-slate-300'"
            :disabled="copyState.isCopying"
          />
          <p v-if="subfolderRequired" class="text-xs text-amber-600 mt-1">Sub-folder required before copying</p>
        </div>
      </template>

      <div v-else class="text-center py-6 text-slate-500 text-sm">
        Configure this reader to start ingesting directly into the active event.
      </div>

      <div v-if="copyState.isCopying" class="space-y-1">
        <div class="flex justify-between text-xs">
          <span class="text-blue-600 font-medium">Copying & indexing...</span>
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
        class="bg-emerald-50 p-2 rounded text-xs text-emerald-700 border border-emerald-200 flex items-center justify-between"
      >
        <span>
          Copied {{ copyState.lastResult.copiedCount }} files ({{ copyState.lastResult.queuedCount }} queued) in
          {{ copyState.lastResult.durationSeconds.toFixed(1) }}s
        </span>
        <button @click="ingestStore.resetCopyState(reader.reader_id)" class="ml-2 text-emerald-600">&times;</button>
      </div>

      <div
        v-if="copyState.error"
        class="bg-red-50 p-2 rounded text-xs text-red-700 border border-red-200 flex items-center justify-between"
      >
        <span class="break-all">{{ copyState.error }}</span>
        <button @click="ingestStore.resetCopyState(reader.reader_id)" class="ml-2 text-red-600">&times;</button>
      </div>
    </div>

    <div class="p-4 border-t border-slate-100 bg-slate-50 rounded-b-xl flex gap-2">
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
        class="flex-1 px-3 py-2 text-sm font-medium text-white rounded-lg"
        :class="canCopy ? 'bg-blue-600 hover:bg-blue-700' : 'bg-slate-400 cursor-not-allowed'"
      >
        {{ copyState.isCopying ? "Ingesting..." : "Ingest Card" }}
      </button>
    </div>
  </div>
</template>
