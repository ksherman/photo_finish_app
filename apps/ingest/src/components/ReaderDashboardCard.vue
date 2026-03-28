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
    !subfolderRequired.value &&
    (folderEntries.value.length === 0 || Object.values(folderToggles.value).some(Boolean))
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
  () => [props.reader.volume_name, props.reader.mount_point],
  ([newVol]) => {
    if (newVol !== "Offline") {
      loadFolderList();
    }
  }
);
</script>

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
        <div v-else-if="!isOffline && isConfigured" class="mt-1 text-xs text-gray-400 italic">
          No folders found on card
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
