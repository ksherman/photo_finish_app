<script setup lang="ts">
import { ref, computed } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { useEventStore } from "@/stores/event";
import type { EventSummary } from "@/types";

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
const stationCode = ref("A");
const numAreas = ref(2);
const areaPrefix = ref("Gym");
const sessionsPerArea = ref(4);
const sessionPrefix = ref("Session");
const enableScaffold = ref(true);

const errorMessage = ref<string | null>(null);
const gatheredResult = ref<EventSummary[] | null>(null);

const selectedPreset = computed(() =>
  eventStore.presets.find((p) => p.id === presetId.value)
);

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

  try {
    await eventStore.createEvent({
      rootPath,
      name: name.value.trim(),
      presetId: presetId.value,
      subjectLabel: subjectLabel.value.trim() || undefined,
      folderLevels: folderLevels.length > 0 ? folderLevels : undefined,
      orderCode: orderCode.value.trim() || undefined,
      stationCode: stationCode.value.trim() || "A",
      scaffold: enableScaffold.value
        ? {
            num_areas: numAreas.value,
            area_prefix: areaPrefix.value.trim() || "Area",
            sessions_per_area: sessionsPerArea.value,
            session_prefix: sessionPrefix.value.trim() || "Session",
          }
        : undefined,
    });
    showCreate.value = false;
    name.value = "";
    emit("opened");
  } catch (err) {
    errorMessage.value = String(err);
  }
}

async function handleOpenExisting() {
  errorMessage.value = null;
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
  gatheredResult.value = null;
  const selected = await open({
    directory: true,
    multiple: false,
    title: "Gather Multiple Event Bundles From Parent Folder (e.g. NAS / Season Folder)",
  });
  if (typeof selected === "string") {
    try {
      gatheredResult.value = await eventStore.gatherEvents(selected);
    } catch (err) {
      errorMessage.value = String(err);
    }
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
          class="px-3.5 py-2 text-sm font-medium rounded-lg border border-slate-300 bg-white text-slate-700 hover:bg-slate-50"
        >
          Gather Folder (Multi-Server)
        </button>
        <button
          @click="handleOpenExisting"
          class="px-3.5 py-2 text-sm font-medium rounded-lg border border-slate-300 bg-white text-slate-700 hover:bg-slate-50"
        >
          Open Bundle
        </button>
        <button
          @click="showCreate = !showCreate"
          class="px-4 py-2 text-sm font-medium rounded-lg bg-blue-600 text-white hover:bg-blue-700"
        >
          {{ showCreate ? "Close Form" : "+ New Event Bundle" }}
        </button>
      </div>
    </div>

    <div v-if="errorMessage" class="p-4 rounded-lg bg-red-50 border border-red-200 text-sm text-red-700">
      {{ errorMessage }}
    </div>

    <div
      v-if="gatheredResult"
      class="p-4 rounded-lg bg-emerald-50 border border-emerald-200 text-sm text-emerald-800"
    >
      Discovered {{ gatheredResult.length }} event bundle(s) and added them to your library.
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
        <h2 class="text-sm font-semibold text-slate-700">Known Event Bundles ({{ eventStore.library.length }})</h2>
      </div>
      <div v-if="eventStore.library.length === 0" class="p-10 text-center text-slate-500 text-sm">
        No event bundles yet. Create one, open an existing folder, or gather a NAS folder with multiple meets.
      </div>
      <ul v-else class="divide-y divide-slate-100">
        <li
          v-for="entry in eventStore.library"
          :key="entry.root_path"
          class="px-5 py-4 flex items-center justify-between gap-4 hover:bg-slate-50"
        >
          <div class="min-w-0">
            <div class="flex items-center gap-2">
              <span class="font-semibold text-slate-900">{{ entry.name }}</span>
              <span
                v-if="eventStore.activeEvent?.rootPath === entry.root_path"
                class="px-2 py-0.5 text-xs rounded-full bg-emerald-100 text-emerald-700 font-medium"
              >
                Active
              </span>
              <span
                v-if="!entry.exists_on_disk"
                class="px-2 py-0.5 text-xs rounded-full bg-red-100 text-red-700 font-medium"
              >
                Drive Offline
              </span>
            </div>
            <p class="text-xs font-mono text-slate-500 truncate mt-0.5">{{ entry.root_path }}</p>
          </div>
          <div class="flex items-center gap-2 flex-shrink-0">
            <button
              v-if="entry.exists_on_disk"
              @click="openFromLibrary(entry.root_path)"
              class="px-3 py-1.5 text-xs font-medium rounded-md bg-blue-600 text-white hover:bg-blue-700"
            >
              Open
            </button>
            <button
              @click="eventStore.removeLibraryEvent(entry.root_path)"
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
