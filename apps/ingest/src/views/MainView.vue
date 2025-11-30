<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from "vue";
import { useSessionStore } from "@/stores/session";
import { useCardReaderStore } from "@/stores/cardReader";
import SessionConfig from "@/components/SessionConfig.vue";
import CardReaderStatus from "@/components/CardReaderStatus.vue";
import DestinationPreview from "@/components/DestinationPreview.vue";
import CopyProgress from "@/components/CopyProgress.vue";

const emit = defineEmits<{
  navigate: [view: string];
}>();

const sessionStore = useSessionStore();
const cardReaderStore = useCardReaderStore();

const showConfig = ref(false);
const rotation = ref("");
const apparatus = ref("");
const lastCopyResult = ref<{ count: number; success: boolean } | null>(null);

let pollInterval: ReturnType<typeof setInterval> | null = null;

const canCopy = computed(
  () =>
    sessionStore.isConfigured &&
    cardReaderStore.selectedVolume &&
    cardReaderStore.totalFileCount > 0 &&
    !cardReaderStore.isCopying
);

onMounted(() => {
  cardReaderStore.refreshVolumes();

  // Poll for volume changes
  pollInterval = setInterval(() => {
    if (!cardReaderStore.isCopying) {
      cardReaderStore.refreshVolumes();
    }
  }, 3000);
});

onUnmounted(() => {
  if (pollInterval) {
    clearInterval(pollInterval);
  }
});

function onSessionConfigured() {
  showConfig.value = false;
}

async function startCopy() {
  if (!canCopy.value) return;

  lastCopyResult.value = null;

  try {
    const copiedCount = await cardReaderStore.copyFiles(
      sessionStore.destinationPath
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
          <button
            v-if="cardReaderStore.directories.length > 0"
            @click="emit('navigate', 'rename')"
            class="text-sm text-blue-600 hover:underline"
          >
            Rename Folders
          </button>
        </div>
      </div>

      <!-- Session Config (if not configured or editing) -->
      <SessionConfig
        v-if="!sessionStore.isConfigured || showConfig"
        @configured="onSessionConfigured"
        @cancel="showConfig = false"
      />

      <template v-else>
        <!-- Current Session Summary -->
        <div class="bg-white rounded-lg shadow p-4 mb-4">
          <div class="flex justify-between items-center mb-2">
            <h2 class="font-semibold text-gray-700">Session</h2>
            <button
              @click="showConfig = true"
              class="text-sm text-blue-600 hover:underline"
            >
              Edit
            </button>
          </div>
          <div class="grid grid-cols-2 gap-2 text-sm">
            <div>
              <span class="text-gray-500">Event:</span>
              {{ sessionStore.eventSlug }}
            </div>
            <div>
              <span class="text-gray-500">Photographer:</span>
              {{ sessionStore.photographer }}
            </div>
            <div>
              <span class="text-gray-500">Gym:</span> {{ sessionStore.gym }}
            </div>
            <div>
              <span class="text-gray-500">Session:</span>
              {{ sessionStore.sessionNumber }}
            </div>
          </div>
        </div>

        <!-- Card Reader Status -->
        <CardReaderStatus />

        <!-- Destination Preview -->
        <DestinationPreview />

        <!-- Rotation/Apparatus (Manual for V1) -->
        <div class="bg-white rounded-lg shadow p-4 mb-4">
          <h2 class="font-semibold text-gray-700 mb-3">
            Rotation Info (optional)
          </h2>
          <div class="grid grid-cols-2 gap-4">
            <div>
              <label class="block text-sm text-gray-600 mb-1"
                >Rotation/Group</label
              >
              <input
                v-model="rotation"
                type="text"
                class="w-full border border-gray-300 rounded-lg px-3 py-2"
                placeholder="e.g., Group 1A"
              />
            </div>
            <div>
              <label class="block text-sm text-gray-600 mb-1">Apparatus</label>
              <select
                v-model="apparatus"
                class="w-full border border-gray-300 rounded-lg px-3 py-2"
              >
                <option value="">Select...</option>
                <option value="vault">Vault</option>
                <option value="bars">Bars</option>
                <option value="beam">Beam</option>
                <option value="floor">Floor</option>
              </select>
            </div>
          </div>
        </div>

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
              String(sessionStore.currentOrder).padStart(4, "0")
            }}
          </p>
        </div>
      </template>
    </div>
  </div>
</template>
