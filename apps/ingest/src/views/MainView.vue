<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from "vue";
import { useSessionStore } from "@/stores/session";
import { useCardReaderStore } from "@/stores/cardReader";
import ReaderSetup from "@/components/ReaderSetup.vue";
import CardReaderStatus from "@/components/CardReaderStatus.vue";
import DestinationPreview from "@/components/DestinationPreview.vue";
import CopyProgress from "@/components/CopyProgress.vue";

const emit = defineEmits<{
  navigate: [view: string];
}>();

const sessionStore = useSessionStore();
const cardReaderStore = useCardReaderStore();

const showSetup = ref(false);
const lastCopyResult = ref<{ count: number; success: boolean } | null>(null);

let pollInterval: ReturnType<typeof setInterval> | null = null;

const canCopy = computed(
  () =>
    sessionStore.isConfigured &&
    cardReaderStore.selectedReader &&
    cardReaderStore.totalFileCount > 0 &&
    !cardReaderStore.isCopying
);

const cameraBrands = [
  { label: "Sony (DCIM)", value: "sony" },
  { label: "Canon (DCIM)", value: "canon" },
  { label: "Nikon (DCIM)", value: "nikon" },
  { label: "Fujifilm (DCIM)", value: "fujifilm" },
  { label: "Panasonic (DCIM)", value: "panasonic" },
  { label: "Olympus (DCIM)", value: "olympus" },
];

const cameraBrandLabel = computed(() => {
  const brandValue = sessionStore.activeMapping?.cameraBrand;
  return cameraBrands.find((b) => b.value === brandValue)?.label || "Unknown";
});

onMounted(() => {
  cardReaderStore.discoverReaders();

  // Poll for volume changes
  pollInterval = setInterval(() => {
    if (!cardReaderStore.isCopying) {
      cardReaderStore.discoverReaders();
    }
  }, 3000);

  // Auto-activate reader from existing mapping
  if (cardReaderStore.cardReaders.length > 0) {
    for (const reader of cardReaderStore.cardReaders) {
      const mapping = sessionStore.getReaderMapping(reader.reader_id);
      if (mapping) {
        sessionStore.setActiveReader(reader.reader_id);
        cardReaderStore.selectReader(reader, mapping.cameraFolderPath);
        break;
      }
    }
  }
});

onUnmounted(() => {
  if (pollInterval) {
    clearInterval(pollInterval);
  }
});

function onReaderConfigured() {
  showSetup.value = false;
}

async function startCopy() {
  if (!canCopy.value) return;

  lastCopyResult.value = null;

  try {
    const cameraFolderPath = sessionStore.activeMapping?.cameraFolderPath || "";
    const renamePrefix = sessionStore.activeMapping?.renamePrefix || "";
    const autoRename = sessionStore.activeMapping?.autoRename || false;

    const copiedCount = await cardReaderStore.copyFiles(
      sessionStore.destinationPath,
      cameraFolderPath,
      renamePrefix,
      autoRename
    );

    lastCopyResult.value = { count: copiedCount, success: true };

    // Increment order for next batch
    sessionStore.incrementOrder();
  } catch (error) {
    lastCopyResult.value = { count: 0, success: false };
    console.error("Copy failed:", error);
  }
}
</script>

<template>
  <div class="min-h-screen bg-gray-100 p-6">
    <div class="max-w-2xl mx-auto">
      <!-- Header -->
      <div class="bg-white rounded-lg shadow p-4 mb-4">
        <div class="flex justify-between items-center">
          <div>
            <h1 class="text-xl font-bold text-gray-800">PhotoFinish Ingest</h1>
            <p class="text-sm text-gray-500">
              Copy photos from memory card to server
            </p>
          </div>
        </div>
      </div>

      <!-- Reader Setup (if not configured or editing) -->
      <ReaderSetup
        v-if="!sessionStore.isConfigured || showSetup"
        @configured="onReaderConfigured"
      />

      <template v-else>
        <!-- Current Session Summary -->
        <div class="bg-white rounded-lg shadow p-4 mb-4">
          <div class="flex justify-between items-center mb-2">
            <h2 class="font-semibold text-gray-700">Session</h2>
            <button
              @click="showSetup = true"
              class="text-sm text-blue-600 hover:underline"
            >
              Change
            </button>
          </div>
          <div class="text-sm space-y-1">
            <div>
              <span class="text-gray-500">Reader:</span>
              {{ sessionStore.activeMapping?.displayName || "Unknown" }}
            </div>
            <div>
              <span class="text-gray-500">Camera:</span>
              {{ cameraBrandLabel }}
            </div>
            <div>
              <span class="text-gray-500">Photographer:</span>
              {{ sessionStore.currentPhotographer }}
            </div>
            <div class="text-xs text-gray-400 truncate">
              {{ sessionStore.currentDestination }}
            </div>
          </div>
        </div>

        <!-- Card Reader Status -->
        <CardReaderStatus />

        <!-- Destination Preview -->
        <DestinationPreview />

        <!-- Copy Button -->
        <div class="bg-white rounded-lg shadow p-4 mb-4">
          <button
            @click="startCopy"
            :disabled="!canCopy"
            class="w-full py-3 rounded-lg font-semibold text-white transition-colors"
            :class="
              canCopy
                ? 'bg-blue-600 hover:bg-blue-700'
                : 'bg-gray-400 cursor-not-allowed'
            "
          >
            {{
              cardReaderStore.isCopying
                ? "Copying..."
                : `Copy ${cardReaderStore.totalFileCount} Files to Server`
            }}
          </button>
        </div>

        <!-- Progress -->
        <CopyProgress />

        <!-- Success Message -->
        <div
          v-if="lastCopyResult?.success"
          class="bg-green-50 border border-green-200 rounded-lg p-4 mb-4"
        >
          <p class="text-green-800 font-medium">
            Successfully copied {{ lastCopyResult.count }} files!
          </p>
          <p class="text-sm text-green-600 mt-1">
            Order incremented to #{{
              String(sessionStore.currentOrderNumber).padStart(4, "0")
            }}
          </p>
        </div>
      </template>
    </div>
  </div>
</template>
