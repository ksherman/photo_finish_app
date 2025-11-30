<script setup lang="ts">
import { computed } from "vue";
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

const mapping = computed(() =>
  sessionStore.getReaderMapping(props.reader.reader_id)
);

const copyState = computed(() =>
  cardReaderStore.getCopyState(props.reader.reader_id)
);

const isConfigured = computed(() => !!mapping.value);

const canCopy = computed(
  () =>
    isConfigured.value &&
    !copyState.value.isCopying &&
    props.reader.file_count > 0
);

const destinationDisplay = computed(() => {
  if (!mapping.value) return "";
  const orderPadded = String(mapping.value.currentOrder).padStart(4, "0");
  // Show last part of path for brevity
  const parts = mapping.value.destination.split("/");
  const shortPath = parts.slice(-2).join("/");
  return `.../${shortPath}/${orderPadded}`;
});

async function startCopy() {
  if (!canCopy.value || !mapping.value) return;

  try {
    await cardReaderStore.copyFiles(
      props.reader.reader_id,
      mapping.value.destination, // Base destination
      mapping.value.cameraFolderPath,
      mapping.value.renamePrefix,
      mapping.value.autoRename
    );

    // Increment order on success
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
          <h3 class="font-semibold text-gray-800">{{ reader.display_name }}</h3>
          <p class="text-xs text-gray-500 font-mono mt-1">{{ reader.volume_name }}</p>
        </div>
        <span
          class="px-2 py-1 text-xs font-medium rounded-full"
          :class="isConfigured ? 'bg-green-100 text-green-700' : 'bg-yellow-100 text-yellow-700'"
        >
          {{ isConfigured ? 'Ready' : 'Setup Needed' }}
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
          <div>
            <span class="text-gray-500 block text-xs">Camera</span>
            <span class="font-medium text-gray-900">{{ mapping.cameraBrand }}</span>
          </div>
        </div>
        
        <div class="text-sm bg-gray-50 p-2 rounded border border-gray-100">
          <span class="text-gray-500 block text-xs mb-1">Destination</span>
          <code class="text-xs text-blue-600 break-all">{{ destinationDisplay }}</code>
        </div>
        
        <div class="mt-3">
             <span class="text-gray-500 text-xs">Files on card:</span>
             <span class="font-bold text-gray-900 ml-1">{{ reader.file_count }}</span>
        </div>
      </div>

      <div v-else class="text-center py-6 text-gray-500 text-sm">
        <p>Configure this reader to start ingesting photos.</p>
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
        ✅ Copied {{ copyState.lastResult.count }} files
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
