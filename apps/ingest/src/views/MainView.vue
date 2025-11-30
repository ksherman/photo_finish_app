<script setup lang="ts">
import { ref, onMounted, onUnmounted } from "vue";
import { useCardReaderStore } from "@/stores/cardReader";
import ReaderDashboardCard from "@/components/ReaderDashboardCard.vue";
import ReaderSetup from "@/components/ReaderSetup.vue";

const cardReaderStore = useCardReaderStore();

const configuringReaderId = ref<string | null>(null);

let pollInterval: ReturnType<typeof setInterval> | null = null;

onMounted(() => {
  cardReaderStore.discoverReaders();

  // Poll for volume changes
  pollInterval = setInterval(() => {
    // We can poll even during copy now, but maybe less frequently or just rely on OS events if we had them.
    // For now, keep polling.
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
        <div class="text-sm text-gray-500">
          {{ cardReaderStore.cardReaders.length }} reader(s) detected
        </div>
      </div>

      <!-- Reader Grid -->
      <div 
        v-if="cardReaderStore.cardReaders.length > 0"
        class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-6"
      >
        <ReaderDashboardCard
          v-for="reader in cardReaderStore.cardReaders"
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
