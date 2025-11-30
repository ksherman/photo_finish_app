<script setup lang="ts">
import { ref, computed } from "vue";
import { useSessionStore } from "@/stores/session";
import { useCardReaderStore } from "@/stores/cardReader";
import type { CardReaderInfo } from "@/types";

const props = defineProps<{
  reader: CardReaderInfo;
}>();

const emit = defineEmits<{
  configure: [readerId: string];
}>();

const sessionStore = useSessionStore();
const cardReaderStore = useCardReaderStore();
const customSubPath = ref("");

const mapping = computed(() =>
  sessionStore.getReaderMapping(props.reader.reader_id)
);

const copyState = computed(() =>
  cardReaderStore.getCopyState(props.reader.reader_id)
);

const isConfigured = computed(() => !!mapping.value);
const isOffline = computed(() => props.reader.volume_name === "Offline");

const canCopy = computed(
  () =>
    !isOffline.value &&
    isConfigured.value &&
    !copyState.value.isCopying &&
    props.reader.file_count > 0
);

const destinationDisplay = computed(() => {
  if (!mapping.value) return "";
  
  let display = mapping.value.destination;
  
  if (customSubPath.value.trim()) {
    display += `/${customSubPath.value.trim().replace(/^\/+|\/+$/g, '')}`;
  }
  
  return display;
});

async function startCopy() {
  if (!canCopy.value || !mapping.value) return;

  let fullPath = `${mapping.value.destination}`;

  if (customSubPath.value.trim()) {
    const cleanSubPath = customSubPath.value.trim().replace(/^\/+|\/+$/g, '');
    fullPath = `${fullPath}/${cleanSubPath}`;
  }

  try {
    await cardReaderStore.copyFiles(
      props.reader.reader_id,
      fullPath,
      mapping.value.cameraFolderPath,
      mapping.value.renamePrefix,
      mapping.value.autoRename,
      mapping.value.fileRenamePrefix
    );

    // Increment order on success (still tracking it in background in case needed later)
    sessionStore.incrementOrderForReader(props.reader.reader_id);
    
  } catch (error) {
    console.error("Copy failed:", error);
  }
}
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
    <div class="p-4 flex-1 space-y-3">
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
      </div>

      <div v-else class="text-center py-6 text-gray-500 text-sm">
        <p v-if="!isOffline">Configure this reader to start ingesting photos.</p>
        <p v-else>Reader is offline but can be configured.</p>
      </div>

      <div v-if="isConfigured" class="mt-auto space-y-3">
        <!-- Destination Preview -->
        <div class="text-sm bg-gray-50 p-2 rounded border border-gray-100">
          <span class="text-gray-500 block text-xs mb-1">Destination</span>
          <code class="text-xs text-blue-600 break-all block leading-tight">{{ destinationDisplay }}</code>
        </div>

        <!-- Sub-Folder Input -->
        <div class="pt-2 border-t border-dashed border-gray-200">
          <label class="block text-xs font-medium text-gray-700 mb-1">
            Optional Sub-folder
          </label>
          <input
              v-model="customSubPath"
              type="text"
              placeholder="e.g. Group 8B/Floor"
              class="w-full text-sm border border-gray-300 rounded-md px-2 py-1.5 focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
              :disabled="copyState.isCopying"
          />
        </div>
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

      <div v-if="copyState.lastResult?.success" class="mt-4 bg-green-50 p-2 rounded text-xs text-green-700 border border-green-100">
        ✅ Copied {{ copyState.lastResult.count }} files in {{ copyState.lastResult.durationSeconds.toFixed(1) }}s
      </div>
      
      <div v-if="copyState.error" class="mt-4 bg-red-50 p-2 rounded text-xs text-red-700 border border-red-100">
        ❌ Error: {{ copyState.error }}
      </div>
    </div>

    <!-- Footer Actions -->
    <div class="p-4 border-t border-gray-100 bg-gray-50 rounded-b-lg flex gap-2">
      <button
        @click="emit('configure', reader.reader_id)"
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
