<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed } from "vue";
import { useCardReaderStore } from "@/stores/cardReader";
import { useSessionStore } from "@/stores/session";
import ReaderDashboardCard from "@/components/ReaderDashboardCard.vue";
import ReaderSetup from "@/components/ReaderSetup.vue";
import type { CardReaderInfo } from "@/types";
import { ask } from "@tauri-apps/plugin-dialog";

const cardReaderStore = useCardReaderStore();
const sessionStore = useSessionStore();

const configuringReaderId = ref<string | null>(null);

let pollInterval: ReturnType<typeof setInterval> | null = null;

// Combine detected readers with persisted configurations that are currently offline
const displayedReaders = computed<CardReaderInfo[]>(() => {
  const readers = [...cardReaderStore.cardReaders];
  
  // Find configured readers that aren't in the detected list
  for (const [readerId, mapping] of Object.entries(sessionStore.readerMappings)) {
    if (!readers.some(r => r.reader_id === readerId)) {
      readers.push({
        reader_id: readerId,
        display_name: mapping.displayName,
        mount_point: "Not Connected",
        volume_name: "Offline",
        bus_protocol: "Unknown",
        is_internal: false,
        disk_id: "",
        file_count: 0,
        folder_count: 0
      });
    }
  }
  
  // Sort: Connected first, then by name
  return readers.sort((a, b) => {
    const aConnected = a.volume_name !== "Offline";
    const bConnected = b.volume_name !== "Offline";
    if (aConnected !== bConnected) return aConnected ? -1 : 1;
    return a.display_name.localeCompare(b.display_name);
  });
});

onMounted(() => {
  cardReaderStore.discoverReaders();

  // Poll for volume changes
  pollInterval = setInterval(() => {
    cardReaderStore.discoverReaders();
  }, 3000);
});

onUnmounted(() => {
  if (pollInterval) {
    clearInterval(pollInterval);
  }
});

function onConfigureReader(readerId: string) {
  configuringReaderId.value = readerId;
}

function onSetupClose() {
  configuringReaderId.value = null;
}

function onSetupSaved() {
  configuringReaderId.value = null;
}

async function confirmReset() {
  const confirmed = await ask("Are you sure you want to clear all reader configurations?", {
    title: "Reset Configuration",
    kind: "warning"
  });
  
  if (confirmed) {
    sessionStore.clearAllMappings();
  }
}
</script>

<template>
  <div class="min-h-screen bg-gray-100 p-6">
    <div class="max-w-7xl mx-auto">
      <!-- Header -->
      <div class="bg-white rounded-lg shadow p-4 mb-6 flex justify-between items-center">
        <div>
          <h1 class="text-xl font-bold text-gray-800">PhotoFinish Ingest</h1>
          <p class="text-sm text-gray-500">
            Copy photos from memory cards to server
          </p>
        </div>
        <div class="flex items-center gap-4">
          <div class="text-sm text-gray-500">
            {{ displayedReaders.length }} reader(s) known
          </div>
          <button
            @click="confirmReset"
            class="text-sm text-red-600 hover:text-red-800 hover:underline px-3 py-1 rounded"
            v-if="Object.keys(sessionStore.readerMappings).length > 0"
          >
            Reset All Configs
          </button>
        </div>
      </div>

      <!-- Reader Grid -->
      <div 
        v-if="displayedReaders.length > 0"
        class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-6"
      >
        <ReaderDashboardCard
          v-for="reader in displayedReaders"
          :key="reader.reader_id"
          :reader="reader"
          @configure="onConfigureReader"
        />
      </div>

      <!-- Empty State -->
      <div v-else class="text-center py-20 bg-white rounded-lg shadow">
        <svg xmlns="http://www.w3.org/2000/svg" class="h-16 w-16 mx-auto text-gray-300 mb-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 3v2m6-2v2M9 19v2m6-2v2M5 9H3m2 6H3m18-6h-2m2 6h-2M7 19h10a2 2 0 002-2V7a2 2 0 00-2-2H7a2 2 0 00-2 2v10a2 2 0 002 2zM9 9h6v6H9V9z" />
        </svg>
        <h3 class="text-lg font-medium text-gray-900">No Card Readers Detected</h3>
        <p class="text-gray-500 mt-2">Connect a card reader or insert a memory card to begin.</p>
        <button
          @click="cardReaderStore.discoverReaders()"
          class="mt-4 px-4 py-2 bg-blue-50 text-blue-600 rounded-lg hover:bg-blue-100 font-medium"
        >
          Refresh Now
        </button>
      </div>

      <!-- Setup Modal -->
      <div 
        v-if="configuringReaderId"
        class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center p-4 z-50 backdrop-blur-sm"
      >
        <div class="w-full max-w-lg bg-white rounded-lg shadow-xl" @click.stop>
          <ReaderSetup
            :reader-id="configuringReaderId"
            @close="onSetupClose"
            @saved="onSetupSaved"
          />
        </div>
      </div>

    </div>
  </div>
</template>
