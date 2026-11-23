<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from "vue";
import { ask } from "@tauri-apps/plugin-dialog";
import { useEventStore } from "@/stores/event";
import { useIngestStore } from "@/stores/ingest";
import ReaderDashboardCard from "@/components/ReaderDashboardCard.vue";
import ReaderSetupModal from "@/components/ReaderSetupModal.vue";
import type { CardReaderInfo } from "@/types";

const eventStore = useEventStore();
const ingestStore = useIngestStore();

const configuringReader = ref<CardReaderInfo | null>(null);
const isEjectingAll = ref(false);
let pollTimer: ReturnType<typeof setInterval> | null = null;

const onlineReaders = computed(() =>
  ingestStore.cardReaders.filter((r) => r.volume_name !== "Offline" && Boolean(r.disk_id))
);

const anyReaderCopying = computed(() =>
  onlineReaders.value.some((r) => ingestStore.getCopyState(r.reader_id).isCopying)
);

async function ejectAllCards() {
  if (isEjectingAll.value || anyReaderCopying.value) return;
  isEjectingAll.value = true;
  try {
    for (const r of onlineReaders.value) {
      try {
        await ingestStore.ejectReader(r.disk_id);
      } catch (err) {
        console.error(`Failed to eject ${r.volume_name}:`, err);
      }
    }
  } finally {
    isEjectingAll.value = false;
  }
}

const displayedReaders = computed<CardReaderInfo[]>(() => {
  const readers = [...ingestStore.cardReaders];
  for (const [readerId, cfg] of Object.entries(ingestStore.readerConfigs)) {
    if (!readers.some((r) => r.reader_id === readerId)) {
      readers.push({
        reader_id: readerId,
        display_name: cfg.displayName,
        mount_point: "Not Connected",
        volume_name: "Offline",
        bus_protocol: "Unknown",
        is_internal: false,
        disk_id: "",
        file_count: 0,
        folder_count: 0,
      });
    }
  }
  return readers.sort((a, b) => {
    const aOnline = a.volume_name !== "Offline";
    const bOnline = b.volume_name !== "Offline";
    if (aOnline !== bOnline) return aOnline ? -1 : 1;
    return a.display_name.localeCompare(b.display_name);
  });
});

onMounted(async () => {
  await Promise.all([ingestStore.loadReaderConfigs(), ingestStore.discoverReaders()]);
  pollTimer = setInterval(async () => {
    await ingestStore.discoverReaders();
    if (eventStore.activeEvent && eventStore.activeEvent.photoCounts.pending > 0) {
      await eventStore.refreshActiveEvent();
    }
  }, 3000);
});

onUnmounted(() => {
  if (pollTimer) clearInterval(pollTimer);
});

async function confirmResetConfigs() {
  const ok = await ask("Clear all persisted card reader configurations?", {
    title: "Reset Reader Configs",
    kind: "warning",
  });
  if (ok) {
    await ingestStore.clearAllConfigs();
  }
}
</script>

<template>
  <div class="max-w-7xl mx-auto p-6 space-y-6">
    <div class="flex flex-wrap items-center justify-between gap-4">
      <div>
        <h1 class="text-2xl font-bold text-slate-900">Card Ingest</h1>
        <p class="text-sm text-slate-500 mt-0.5">
          Copies JPEGs straight into the active Event Bundle, logs the card batch, and queues thumbnails immediately.
        </p>
      </div>
      <div class="flex items-center gap-3">
        <button
          v-if="onlineReaders.length > 1"
          @click="ejectAllCards"
          :disabled="isEjectingAll || anyReaderCopying"
          class="px-3 py-1.5 text-sm font-medium rounded-lg border border-slate-300 bg-white text-slate-700 hover:bg-slate-50 disabled:opacity-40"
        >
          {{ isEjectingAll ? "Ejecting..." : `⏏ Eject All (${onlineReaders.length})` }}
        </button>
        <button
          @click="ingestStore.discoverReaders()"
          class="px-3 py-1.5 text-sm rounded-lg border border-slate-300 bg-white text-slate-700 hover:bg-slate-50"
        >
          Refresh Readers
        </button>
        <button
          v-if="Object.keys(ingestStore.readerConfigs).length > 0"
          @click="confirmResetConfigs"
          class="px-3 py-1.5 text-sm text-red-600 hover:underline"
        >
          Reset Configs
        </button>
      </div>
    </div>

    <div
      v-if="!eventStore.activeEvent"
      class="p-4 rounded-xl bg-amber-50 border border-amber-200 text-sm text-amber-800"
    >
      Open or create an Event Bundle in the <strong>Events</strong> tab before ingesting memory cards.
    </div>

    <div
      v-if="displayedReaders.length > 0"
      class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-5"
    >
      <ReaderDashboardCard
        v-for="reader in displayedReaders"
        :key="reader.reader_id"
        :reader="reader"
        @configure="configuringReader = $event"
      />
    </div>

    <div v-else class="bg-white rounded-xl border border-slate-200 p-14 text-center">
      <h3 class="text-lg font-medium text-slate-900">No Card Readers Detected</h3>
      <p class="text-sm text-slate-500 mt-1">
        Connect a USB card reader or insert an SD card to begin ingesting.
      </p>
      <button
        @click="ingestStore.discoverReaders()"
        class="mt-4 px-4 py-2 text-sm font-medium bg-blue-50 text-blue-600 rounded-lg hover:bg-blue-100"
      >
        Refresh Now
      </button>
    </div>

    <div
      v-if="configuringReader"
      class="fixed inset-0 bg-black/50 flex items-center justify-center p-4 z-50 backdrop-blur-sm"
      @click="configuringReader = null"
    >
      <div class="w-full max-w-lg" @click.stop>
        <ReaderSetupModal
          :reader="configuringReader"
          @close="configuringReader = null"
          @saved="configuringReader = null"
        />
      </div>
    </div>
  </div>
</template>
