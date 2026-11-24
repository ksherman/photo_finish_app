<script setup lang="ts">
import { ref, computed, watch, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { useEventStore } from "@/stores/event";
import type { CsvExportResult, EventSummary } from "@/types";

const emit = defineEmits<{
  opened: [];
}>();

const eventStore = useEventStore();

const showCreate = ref(false);
const name = ref("");
const parentDir = ref("");
const presetId = ref("gymnastics");
const subjectLabel = ref("Competitor");
const customLevelsText = ref("Gym, Session, Group, Apparatus, Subject");
const orderCode = ref("");
const stationCode = ref(eventStore.machineStationCode || "A");
const numAreas = ref(2);
const areaPrefix = ref("Gym");
const sessionsPerArea = ref(4);
const sessionPrefix = ref("Session");
const enableScaffold = ref(true);

const isRefreshingLibrary = ref(false);
const isGathering = ref(false);
const isExportingAll = ref(false);

const errorMessage = ref<string | null>(null);
const bannerMessage = ref<string | null>(null);
const gatheredResult = ref<EventSummary[] | null>(null);

watch(
  () => eventStore.machineStationCode,
  (code) => {
    if (!showCreate.value) {
      stationCode.value = code;
    }
  },
  { immediate: true }
);

const selectedPreset = computed(() =>
  eventStore.presets.find((p) => p.id === presetId.value)
);

const overviewEvents = computed(() => eventStore.libraryOverview?.events ?? []);
const overviewTotals = computed(() => eventStore.libraryOverview?.totals ?? null);
const duplicateOrderNumbers = computed(
  () => eventStore.libraryOverview?.duplicateOrderNumbers ?? []
);

function formatCents(cents: number): string {
  const sign = cents < 0 ? "-" : "";
  const abs = Math.abs(cents);
  return `${sign}$${(abs / 100).toFixed(2)}`;
}

async function refreshLibraryStatus() {
  isRefreshingLibrary.value = true;
  errorMessage.value = null;
  try {
    await eventStore.refreshLibrary();
  } catch (err) {
    errorMessage.value = String(err);
  } finally {
    isRefreshingLibrary.value = false;
  }
}

function toggleCreateForm() {
  showCreate.value = !showCreate.value;
  if (showCreate.value) {
    stationCode.value = eventStore.machineStationCode || "A";
  }
}

function onPresetChange() {
  if (!selectedPreset.value) return;
  subjectLabel.value = selectedPreset.value.subject_label;
  customLevelsText.value = selectedPreset.value.folder_levels.join(", ");
  if (selectedPreset.value.id === "gymnastics") {
    areaPrefix.value = "Gym";
    sessionPrefix.value = "Session";
    enableScaffold.value = true;
  } else if (selectedPreset.value.id === "dance") {
    areaPrefix.value = "Stage";
    sessionPrefix.value = "Session";
    enableScaffold.value = true;
  } else {
    enableScaffold.value = false;
  }
}

async function pickParentDirectory() {
  const selected = await open({
    directory: true,
    multiple: false,
    title: "Select Parent Folder for New Event Bundle",
  });
  if (typeof selected === "string") {
    parentDir.value = selected;
  }
}

async function handleCreate() {
  errorMessage.value = null;
  bannerMessage.value = null;
  if (!name.value.trim() || !parentDir.value.trim()) {
    errorMessage.value = "Please enter an event name and select a folder location.";
    return;
  }
  const safeFolderName = name.value.trim().replace(/[/\\:*?"<>|]/g, "-");
  const rootPath = `${parentDir.value.replace(/\/+$/, "")}/${safeFolderName}`;
  const folderLevels = customLevelsText.value
    .split(",")
    .map((s) => s.trim())
    .filter(Boolean);

  const chosenStation = stationCode.value.trim().toUpperCase() || eventStore.machineStationCode || "A";

  try {
    await eventStore.createEvent({
      rootPath,
      name: name.value.trim(),
      presetId: presetId.value,
      subjectLabel: subjectLabel.value.trim() || undefined,
      folderLevels: folderLevels.length > 0 ? folderLevels : undefined,
      orderCode: orderCode.value.trim() || undefined,
      stationCode: chosenStation,
      scaffold: enableScaffold.value
        ? {
            num_areas: numAreas.value,
            area_prefix: areaPrefix.value.trim() || "Area",
            sessions_per_area: sessionsPerArea.value,
            session_prefix: sessionPrefix.value.trim() || "Session",
          }
        : undefined,
    });
    if (chosenStation !== eventStore.machineStationCode) {
      await eventStore.setMachineStationCode(chosenStation);
    }
    showCreate.value = false;
    name.value = "";
    emit("opened");
  } catch (err) {
    errorMessage.value = String(err);
  }
}

async function handleOpenExisting() {
  errorMessage.value = null;
  bannerMessage.value = null;
  const selected = await open({
    directory: true,
    multiple: false,
    title: "Open Existing PhotoFinish Event Bundle Folder",
  });
  if (typeof selected === "string") {
    try {
      await eventStore.openEvent(selected);
      emit("opened");
    } catch (err) {
      errorMessage.value = String(err);
    }
  }
}

async function handleGatherFolder() {
  errorMessage.value = null;
  bannerMessage.value = null;
  gatheredResult.value = null;
  const selected = await open({
    directory: true,
    multiple: false,
    title: "Gather Multiple Event Bundles From Parent Folder (e.g. NAS / Season Folder)",
  });
  if (typeof selected === "string") {
    isGathering.value = true;
    try {
      gatheredResult.value = await eventStore.gatherEvents(selected);
    } catch (err) {
      errorMessage.value = String(err);
    } finally {
      isGathering.value = false;
    }
  }
}

async function handleExportConsolidatedCsv() {
  errorMessage.value = null;
  bannerMessage.value = null;
  const targetPath = await save({
    defaultPath: "photofinish-consolidated-orders.csv",
    filters: [{ name: "CSV Spreadsheet", extensions: ["csv"] }],
    title: "Export Consolidated Orders CSV (All Online Library Bundles)",
  });
  if (!targetPath) return;

  isExportingAll.value = true;
  try {
    const res = await invoke<CsvExportResult>("export_orders_csv", {
      outputPath: targetPath,
      allLibraryEvents: true,
    });
    const skippedNote =
      res.skippedEvents.length > 0
        ? ` (Skipped ${res.skippedEvents.length}: ${res.skippedEvents.join(", ")})`
        : "";
    bannerMessage.value = `Exported ${res.orderCount} order(s) (${res.itemRowCount} line items across ${res.eventCount} event${res.eventCount === 1 ? "" : "s"}) to ${res.outputPath}.${skippedNote}`;
  } catch (err) {
    errorMessage.value = String(err);
  } finally {
    isExportingAll.value = false;
  }
}

async function openFromLibrary(rootPath: string) {
  errorMessage.value = null;
  try {
    await eventStore.openEvent(rootPath);
    emit("opened");
  } catch (err) {
    errorMessage.value = String(err);
  }
}

onMounted(() => {
  eventStore.refreshLibrary();
});
</script>

<template>
  <div class="max-w-5xl mx-auto p-6 space-y-6">
    <div class="flex flex-wrap items-center justify-between gap-4">
      <div>
        <h1 class="text-2xl font-bold text-slate-900">Event Bundles</h1>
        <p class="text-sm text-slate-500 mt-0.5">
          Each event is a self-contained folder (<code>_photofinish/photofinish.db</code> + relative photo paths).
        </p>
      </div>
      <div class="flex flex-wrap gap-2">
        <button
          @click="handleGatherFolder"
          :disabled="isGathering"
          class="px-3.5 py-2 text-sm font-medium rounded-lg border border-slate-300 bg-white text-slate-700 hover:bg-slate-50 disabled:opacity-50"
        >
          {{ isGathering ? "Scanning Folder..." : "Gather Folder (Multi-Server)" }}
        </button>
        <button
          @click="handleOpenExisting"
          class="px-3.5 py-2 text-sm font-medium rounded-lg border border-slate-300 bg-white text-slate-700 hover:bg-slate-50"
        >
          Open Bundle
        </button>
        <button
          @click="toggleCreateForm"
          class="px-4 py-2 text-sm font-medium rounded-lg bg-blue-600 text-white hover:bg-blue-700"
        >
          {{ showCreate ? "Close Form" : "+ New Event Bundle" }}
        </button>
      </div>
    </div>

    <div v-if="errorMessage" class="p-4 rounded-lg bg-red-50 border border-red-200 text-sm text-red-700 flex items-center justify-between gap-4">
      <span>{{ errorMessage }}</span>
      <button @click="errorMessage = null" class="text-red-500 font-bold">&times;</button>
    </div>

    <div v-if="bannerMessage" class="p-4 rounded-lg bg-emerald-50 border border-emerald-200 text-sm text-emerald-800 flex items-center justify-between gap-4">
      <span>{{ bannerMessage }}</span>
      <button @click="bannerMessage = null" class="text-emerald-600 font-bold">&times;</button>
    </div>

    <div
      v-if="gatheredResult"
      class="p-4 rounded-lg bg-emerald-50 border border-emerald-200 text-sm text-emerald-800 flex items-center justify-between gap-4"
    >
      <span>
        Discovered <strong>{{ gatheredResult.length }}</strong> event bundle(s) across nested folders and added them to your library.
      </span>
      <button @click="gatheredResult = null" class="text-emerald-600 font-bold">&times;</button>
    </div>

    <div
      v-if="duplicateOrderNumbers.length > 0"
      class="p-4 rounded-lg bg-amber-50 border border-amber-300 text-sm text-amber-900"
    >
      <div class="font-semibold">Duplicate Order Numbers Detected Across Gathered Bundles</div>
      <p class="mt-1 text-xs text-amber-800">
        Multiple online bundles share the same order number(s):
        <span class="font-mono font-semibold">{{ duplicateOrderNumbers.slice(0, 8).join(", ") }}</span>
        <span v-if="duplicateOrderNumbers.length > 8"> (+{{ duplicateOrderNumbers.length - 8 }} more)</span>.
        If running a second server for the same meet, set a distinct Station Code (e.g. B) in Create Event or Orders → Pricing &amp; Tax.
      </p>
    </div>

    <!-- Consolidated Multi-Station Summary Bar -->
    <div
      v-if="overviewTotals && overviewTotals.onlineEvents > 0"
      class="bg-white rounded-xl border border-slate-200 shadow-sm p-4 flex flex-wrap items-center justify-between gap-4"
    >
      <div class="flex flex-wrap items-center gap-6">
        <div>
          <div class="text-[11px] uppercase tracking-wide font-semibold text-slate-400">Online Bundles</div>
          <div class="text-lg font-bold text-slate-900">
            {{ overviewTotals.onlineEvents }}
            <span v-if="overviewTotals.totalEvents > overviewTotals.onlineEvents" class="text-xs font-normal text-slate-500">
              / {{ overviewTotals.totalEvents }}
            </span>
          </div>
        </div>
        <div class="h-8 w-px bg-slate-200 hidden sm:block"></div>
        <div>
          <div class="text-[11px] uppercase tracking-wide font-semibold text-slate-400">Total Photos</div>
          <div class="text-lg font-bold text-slate-900 font-mono">
            {{ overviewTotals.totalPhotos.toLocaleString() }}
          </div>
        </div>
        <div class="h-8 w-px bg-slate-200 hidden sm:block"></div>
        <div>
          <div class="text-[11px] uppercase tracking-wide font-semibold text-slate-400">Combined Orders</div>
          <div class="text-lg font-bold text-slate-900">
            {{ overviewTotals.totalOrders.toLocaleString() }}
            <span v-if="overviewTotals.pendingFulfillmentOrders > 0" class="text-xs font-medium text-amber-700 ml-1">
              ({{ overviewTotals.pendingFulfillmentOrders }} open)
            </span>
          </div>
        </div>
        <div class="h-8 w-px bg-slate-200 hidden sm:block"></div>
        <div>
          <div class="text-[11px] uppercase tracking-wide font-semibold text-slate-400">Paid Revenue</div>
          <div class="text-lg font-bold text-emerald-700 font-mono">
            {{ formatCents(overviewTotals.grossPaidCents) }}
          </div>
        </div>
      </div>

      <button
        @click="handleExportConsolidatedCsv"
        :disabled="isExportingAll || overviewTotals.totalOrders === 0"
        class="px-3.5 py-2 text-xs font-semibold rounded-lg border border-slate-300 bg-slate-50 text-slate-800 hover:bg-slate-100 disabled:opacity-45"
        title="Export all orders across every online Event Bundle in your Library into one spreadsheet"
      >
        {{ isExportingAll ? "Exporting..." : "Export Consolidated Orders CSV" }}
      </button>
    </div>

    <!-- Create Event Panel -->
    <div v-if="showCreate" class="bg-white rounded-xl border border-slate-200 shadow-sm p-6 space-y-4">
      <h2 class="text-lg font-semibold text-slate-900">Create Portable Event Bundle</h2>

      <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
        <div>
          <label class="block text-sm font-medium text-slate-700 mb-1">Event Name *</label>
          <input
            v-model="name"
            type="text"
            placeholder="e.g., 2026 IL Level 3 State"
            class="w-full border border-slate-300 rounded-lg px-3 py-2 text-sm"
          />
        </div>
        <div>
          <label class="block text-sm font-medium text-slate-700 mb-1">Parent Folder (SSD / Storage) *</label>
          <div class="flex gap-2">
            <input
              v-model="parentDir"
              type="text"
              readonly
              placeholder="Choose folder..."
              class="flex-1 border border-slate-300 rounded-lg px-3 py-2 text-sm bg-slate-50"
            />
            <button
              type="button"
              @click="pickParentDirectory"
              class="px-3 py-2 bg-slate-200 hover:bg-slate-300 rounded-lg text-sm font-medium"
            >
              Browse
            </button>
          </div>
        </div>

        <div>
          <label class="block text-sm font-medium text-slate-700 mb-1">Event Structure Preset</label>
          <select
            v-model="presetId"
            @change="onPresetChange"
            class="w-full border border-slate-300 rounded-lg px-3 py-2 text-sm bg-white"
          >
            <option v-for="p in eventStore.presets" :key="p.id" :value="p.id">
              {{ p.label }} — {{ p.description }}
            </option>
          </select>
        </div>

        <div>
          <label class="block text-sm font-medium text-slate-700 mb-1">Subject Label</label>
          <input
            v-model="subjectLabel"
            type="text"
            placeholder="Competitor, Athlete, Dancer, Participant..."
            class="w-full border border-slate-300 rounded-lg px-3 py-2 text-sm"
          />
        </div>

        <div class="md:col-span-2">
          <label class="block text-sm font-medium text-slate-700 mb-1">
            Folder Levels (comma-separated, flexible)
          </label>
          <input
            v-model="customLevelsText"
            type="text"
            class="w-full border border-slate-300 rounded-lg px-3 py-2 text-sm font-mono"
          />
        </div>

        <div>
          <label class="block text-sm font-medium text-slate-700 mb-1">Order Prefix (3 letters)</label>
          <input
            v-model="orderCode"
            type="text"
            maxlength="6"
            placeholder="Auto from name (e.g. STV)"
            class="w-full border border-slate-300 rounded-lg px-3 py-2 text-sm uppercase font-mono"
          />
        </div>

        <div>
          <label class="block text-sm font-medium text-slate-700 mb-1">
            Server Station Code (prevents order # collisions on multi-server meets)
          </label>
          <input
            v-model="stationCode"
            type="text"
            maxlength="3"
            placeholder="A"
            class="w-full border border-slate-300 rounded-lg px-3 py-2 text-sm uppercase font-mono"
          />
        </div>
      </div>

      <div class="pt-3 border-t border-slate-100 space-y-3">
        <label class="flex items-center gap-2 cursor-pointer">
          <input v-model="enableScaffold" type="checkbox" class="rounded" />
          <span class="text-sm font-medium text-slate-700">Pre-create initial area / session folders on disk</span>
        </label>
        <div v-if="enableScaffold" class="grid grid-cols-2 md:grid-cols-4 gap-3 pl-6">
          <div>
            <label class="block text-xs text-slate-500 mb-1">Area Label</label>
            <input v-model="areaPrefix" type="text" class="w-full border border-slate-300 rounded px-2 py-1.5 text-sm" />
          </div>
          <div>
            <label class="block text-xs text-slate-500 mb-1">Count</label>
            <input v-model.number="numAreas" type="number" min="1" max="26" class="w-full border border-slate-300 rounded px-2 py-1.5 text-sm" />
          </div>
          <div>
            <label class="block text-xs text-slate-500 mb-1">Session Label</label>
            <input v-model="sessionPrefix" type="text" class="w-full border border-slate-300 rounded px-2 py-1.5 text-sm" />
          </div>
          <div>
            <label class="block text-xs text-slate-500 mb-1">Per Area</label>
            <input v-model.number="sessionsPerArea" type="number" min="0" max="30" class="w-full border border-slate-300 rounded px-2 py-1.5 text-sm" />
          </div>
        </div>
      </div>

      <div class="flex justify-end gap-2 pt-2">
        <button @click="showCreate = false" class="px-4 py-2 text-sm text-slate-600">Cancel</button>
        <button
          @click="handleCreate"
          class="px-5 py-2 text-sm font-medium rounded-lg bg-blue-600 text-white hover:bg-blue-700"
        >
          Create & Open Event
        </button>
      </div>
    </div>

    <!-- Library List -->
    <div class="bg-white rounded-xl border border-slate-200 shadow-sm overflow-hidden">
      <div class="px-5 py-3.5 bg-slate-50 border-b border-slate-200 flex items-center justify-between">
        <h2 class="text-sm font-semibold text-slate-700">Known Event Bundles ({{ overviewEvents.length }})</h2>
        <button
          @click="refreshLibraryStatus"
          :disabled="isRefreshingLibrary"
          class="text-xs font-medium text-slate-600 hover:text-slate-900 disabled:opacity-50"
        >
          {{ isRefreshingLibrary ? "Refreshing..." : "Refresh" }}
        </button>
      </div>
      <div v-if="overviewEvents.length === 0" class="p-10 text-center text-slate-500 text-sm">
        No event bundles yet. Create one, open an existing folder, or gather a NAS folder with multiple meets.
      </div>
      <ul v-else class="divide-y divide-slate-100">
        <li
          v-for="entry in overviewEvents"
          :key="entry.rootPath"
          class="px-5 py-4 flex items-center justify-between gap-4 hover:bg-slate-50"
        >
          <div class="min-w-0 flex-1">
            <div class="flex flex-wrap items-center gap-2">
              <span class="font-semibold text-slate-900">{{ entry.name }}</span>
              <span
                v-if="entry.orderCode && entry.stationCode"
                class="px-2 py-0.5 text-[11px] font-mono rounded-full bg-slate-100 text-slate-700 border border-slate-200 font-semibold"
              >
                {{ entry.orderCode }}-{{ entry.stationCode }}
              </span>
              <span
                v-if="eventStore.activeEvent?.rootPath === entry.rootPath"
                class="px-2 py-0.5 text-xs rounded-full bg-emerald-100 text-emerald-700 font-medium"
              >
                Active
              </span>
              <span
                v-if="!entry.existsOnDisk"
                class="px-2 py-0.5 text-xs rounded-full bg-red-100 text-red-700 font-medium"
              >
                Drive Offline
              </span>
              <span
                v-else-if="entry.inspectError"
                class="px-2 py-0.5 text-xs rounded-full bg-amber-100 text-amber-800 font-medium"
                :title="entry.inspectError"
              >
                Unreadable / Newer Schema
              </span>
            </div>
            <p class="text-xs font-mono text-slate-500 truncate mt-0.5">{{ entry.rootPath }}</p>
            <div
              v-if="entry.existsOnDisk && !entry.inspectError && entry.photoCounts"
              class="mt-1.5 flex flex-wrap items-center gap-x-4 gap-y-1 text-xs text-slate-600"
            >
              <span>
                <strong class="font-mono text-slate-800">{{ entry.photoCounts.ready.toLocaleString() }}</strong>
                <span v-if="entry.photoCounts.pending > 0" class="text-amber-700">
                  / {{ entry.photoCounts.total.toLocaleString() }}
                </span>
                photos
              </span>
              <span>
                <strong class="font-mono text-slate-800">{{ entry.subjectCount.toLocaleString() }}</strong> roster
              </span>
              <span>
                <strong class="font-mono text-slate-800">{{ entry.orderCount }}</strong>
                order{{ entry.orderCount === 1 ? "" : "s" }}
                <span v-if="entry.grossPaidCents > 0" class="text-emerald-700 font-mono font-semibold ml-1">
                  ({{ formatCents(entry.grossPaidCents) }} paid)
                </span>
              </span>
              <span v-if="entry.pendingFulfillmentCount > 0" class="text-amber-700 font-medium">
                {{ entry.pendingFulfillmentCount }} pending fulfillment
              </span>
            </div>
            <p v-else-if="entry.inspectError" class="mt-1 text-xs text-amber-700">
              {{ entry.inspectError }}
            </p>
          </div>
          <div class="flex items-center gap-2 flex-shrink-0">
            <button
              v-if="entry.existsOnDisk && !entry.inspectError"
              @click="openFromLibrary(entry.rootPath)"
              class="px-3 py-1.5 text-xs font-medium rounded-md bg-blue-600 text-white hover:bg-blue-700"
            >
              Open
            </button>
            <button
              @click="eventStore.removeLibraryEvent(entry.rootPath)"
              class="px-2.5 py-1.5 text-xs text-slate-500 hover:text-red-600"
            >
              Remove
            </button>
          </div>
        </li>
      </ul>
    </div>
  </div>
</template>
