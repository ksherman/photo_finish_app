<script setup lang="ts">
import { ref, watch, onMounted } from "vue";
import { useCardReaderStore } from "@/stores/cardReader";

const cardReaderStore = useCardReaderStore();
const selectedPath = ref("");

watch(
  () => cardReaderStore.selectedVolume,
  (vol) => {
    selectedPath.value = vol?.path || "";
  }
);

onMounted(() => {
  cardReaderStore.refreshVolumes();
});

function onVolumeSelect() {
  const volume = cardReaderStore.volumes.find(
    (v) => v.path === selectedPath.value
  );
  if (volume) {
    cardReaderStore.selectVolume(volume);
  }
}

function refresh() {
  cardReaderStore.refreshVolumes();
}
</script>

<template>
  <div class="bg-white rounded-lg shadow p-4 mb-4">
    <h2 class="font-semibold text-gray-700 mb-3">Card Reader</h2>

    <div class="flex items-center gap-3 mb-3">
      <select
        v-model="selectedPath"
        @change="onVolumeSelect"
        class="flex-1 border border-gray-300 rounded-lg px-3 py-2 focus:ring-2 focus:ring-blue-500"
      >
        <option value="">Select volume...</option>
        <option
          v-for="vol in cardReaderStore.volumes"
          :key="vol.path"
          :value="vol.path"
        >
          {{ vol.name }} ({{ vol.path }})
        </option>
      </select>
      <button
        @click="refresh"
        class="p-2 text-gray-600 hover:text-gray-800 hover:bg-gray-100 rounded-lg"
        title="Refresh volumes"
      >
        <svg
          class="w-5 h-5"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"
          />
        </svg>
      </button>
    </div>

    <div v-if="cardReaderStore.selectedVolume" class="flex items-center gap-2">
      <span class="w-3 h-3 rounded-full bg-green-500"></span>
      <span class="text-sm text-gray-600">
        Card Ready - {{ cardReaderStore.totalFileCount }} photos in
        {{ cardReaderStore.directories.length }} folders
      </span>
    </div>
    <div v-else class="flex items-center gap-2">
      <span class="w-3 h-3 rounded-full bg-gray-400"></span>
      <span class="text-sm text-gray-500">No card selected</span>
    </div>

    <!-- Folder list preview -->
    <div
      v-if="cardReaderStore.directories.length > 0"
      class="mt-3 text-xs text-gray-500 space-y-1"
    >
      <div
        v-for="dir in cardReaderStore.directories.slice(0, 5)"
        :key="dir.path"
        class="flex justify-between"
      >
        <span>{{ dir.name }}</span>
        <span>{{ dir.file_count }} photos</span>
      </div>
      <div v-if="cardReaderStore.directories.length > 5" class="text-gray-400">
        +{{ cardReaderStore.directories.length - 5 }} more folders...
      </div>
    </div>
  </div>
</template>
