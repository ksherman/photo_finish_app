<script setup lang="ts">
import { onMounted } from "vue";
import { useCardReaderStore } from "@/stores/cardReader";
import { useSessionStore } from "@/stores/session";

const cardReaderStore = useCardReaderStore();
const sessionStore = useSessionStore();

onMounted(() => {
  refreshWithCameraPath();
});

async function refresh() {
  await refreshWithCameraPath();
}

async function refreshWithCameraPath() {
  await cardReaderStore.discoverReaders();

  // Reload directories with camera folder path if reader is selected
  if (cardReaderStore.selectedReader && sessionStore.activeMapping) {
    await cardReaderStore.loadDirectoriesFromReader(
      sessionStore.activeMapping.cameraFolderPath
    );
  }
}
</script>

<template>
  <div class="bg-white rounded-lg shadow p-4 mb-4">
    <div class="flex items-center justify-between mb-3">
      <h2 class="font-semibold text-gray-700">Card Reader</h2>
      <button
        @click="refresh"
        class="p-2 text-gray-600 hover:text-gray-800 hover:bg-gray-100 rounded-lg"
        title="Refresh readers"
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

    <div v-if="cardReaderStore.selectedReader" class="space-y-2">
      <div class="flex items-center gap-2">
        <span class="w-3 h-3 rounded-full bg-green-500"></span>
        <span class="text-sm text-gray-600">
          {{ cardReaderStore.selectedReader.display_name }}
        </span>
      </div>
      <div class="text-sm text-gray-500">
        <span class="font-medium">{{ cardReaderStore.selectedReader.volume_name }}</span>
        - {{ cardReaderStore.totalFileCount }} photos in
        {{ cardReaderStore.directories.length }} folders
      </div>
      <div class="text-xs text-gray-400 font-mono truncate">
        {{ cardReaderStore.selectedReader.mount_point }}
      </div>
    </div>

    <div v-else class="flex items-center gap-2">
      <span class="w-3 h-3 rounded-full bg-gray-400"></span>
      <span class="text-sm text-gray-500">No card reader selected</span>
    </div>

    <!-- Folder list preview -->
    <div
      v-if="cardReaderStore.directories.length > 0"
      class="mt-3 text-xs text-gray-500 space-y-1 border-t pt-3"
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
