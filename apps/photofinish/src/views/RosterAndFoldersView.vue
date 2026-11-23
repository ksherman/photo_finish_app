<script setup lang="ts">
import { ref, computed, onMounted, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useEventStore } from "@/stores/event";
import type {
  AssignFolderResult,
  ImportRosterResult,
  ParsedRosterFile,
  RosterEntryInput,
  SubjectRecord,
  UnassignedFolder,
  UnassignedFoldersResponse,
} from "@/types";

const eventStore = useEventStore();

type SubTab = "roster" | "folders";
const subTab = ref<SubTab>("roster");

// Roster state
const groups = ref<string[]>([]);
const subjects = ref<SubjectRecord[]>([]);
const rosterSearch = ref("");
const selectedGroup = ref("");
const parsedFiles = ref<ParsedRosterFile[]>([]);
const fileGroupOverrides = ref<Record<string, string>>({});
const isImporting = ref(false);
const bannerMessage = ref<string | null>(null);
const errorMessage = ref<string | null>(null);

// Quick manual add
const showAddForm = ref(false);
const newNumber = ref("");
const newFirstName = ref("");
const newLastName = ref("");
const newGroup = ref("");
const newTeam = ref("");

// Folder matching state
const unassignedData = ref<UnassignedFoldersResponse>({
  parents: [],
  folders: [],
  totalUnassignedPhotos: 0,
});
const selectedParent = ref("");
const selectedFolder = ref<UnassignedFolder | null>(null);
const matchSubjects = ref<SubjectRecord[]>([]);
const matchSearch = ref("");
const matchGroupFilter = ref("");
const renameOnDisk = ref(true);
const isAssigning = ref(false);

const subjectLabel = computed(() => eventStore.activeEvent?.subjectLabel || "Subject");
const thumbBaseUrl = computed(() => {
  const port = eventStore.lanStatus.port || 4000;
  return `http://127.0.0.1:${port}/photos/thumbnail`;
});

const totalParsedEntries = computed(() =>
  parsedFiles.value.reduce((sum, f) => sum + f.entries.length, 0)
);

async function loadRosterData() {
  if (!eventStore.activeEvent) return;
  try {
    const [g, s] = await Promise.all([
      invoke<string[]>("list_subject_groups"),
      invoke<SubjectRecord[]>("list_subjects", {
        query: rosterSearch.value || null,
        groupLabel: selectedGroup.value || null,
        limit: 500,
      }),
    ]);
    groups.value = g;
    subjects.value = s;
  } catch (err) {
    errorMessage.value = String(err);
  }
}

async function loadUnassignedFolders() {
  if (!eventStore.activeEvent) return;
  try {
    unassignedData.value = await invoke<UnassignedFoldersResponse>("list_unassigned_folders", {
      parentFilter: selectedParent.value || null,
    });
    if (
      selectedFolder.value &&
      !unassignedData.value.folders.some((f) => f.folderPath === selectedFolder.value?.folderPath)
    ) {
      selectedFolder.value = null;
    }
    await loadMatchSubjects();
  } catch (err) {
    errorMessage.value = String(err);
  }
}

async function loadMatchSubjects() {
  if (!eventStore.activeEvent) return;
  matchSubjects.value = await invoke<SubjectRecord[]>("list_subjects", {
    query: matchSearch.value || null,
    groupLabel: matchGroupFilter.value || null,
    limit: 300,
  });
}

async function pickRosterFiles() {
  errorMessage.value = null;
  bannerMessage.value = null;
  const selected = await open({
    multiple: true,
    filters: [{ name: "Roster Files", extensions: ["txt", "csv"] }],
    title: "Select Roster File(s) (.txt or .csv)",
  });
  if (!selected) return;
  const paths = Array.isArray(selected) ? selected : [selected];
  if (paths.length === 0) return;

  try {
    const parsed = await invoke<ParsedRosterFile[]>("parse_roster_files", { paths });
    parsedFiles.value = parsed;
    const overrides: Record<string, string> = {};
    for (const file of parsed) {
      overrides[file.filePath] = file.defaultGroupLabel || "";
    }
    fileGroupOverrides.value = overrides;
  } catch (err) {
    errorMessage.value = String(err);
  }
}

async function confirmImportRoster() {
  if (parsedFiles.value.length === 0 || isImporting.value) return;
  isImporting.value = true;
  errorMessage.value = null;
  try {
    const entries: RosterEntryInput[] = [];
    for (const file of parsedFiles.value) {
      const overrideGroup = (fileGroupOverrides.value[file.filePath] || "").trim();
      for (const e of file.entries) {
        entries.push({
          ...e,
          groupLabel: overrideGroup || e.groupLabel,
        });
      }
    }
    const res = await invoke<ImportRosterResult>("import_roster", { entries });
    parsedFiles.value = [];
    fileGroupOverrides.value = {};
    bannerMessage.value = `Imported ${res.importedNew} new, updated ${res.updated} existing, and auto-matched ${res.autoMatchedPhotos.toLocaleString()} photo(s).`;
    await Promise.all([
      loadRosterData(),
      loadUnassignedFolders(),
      eventStore.refreshActiveEvent(),
    ]);
  } catch (err) {
    errorMessage.value = String(err);
  } finally {
    isImporting.value = false;
  }
}

async function handleManualAdd() {
  if (!newFirstName.value.trim()) return;
  errorMessage.value = null;
  try {
    await invoke("add_subject", {
      number: newNumber.value.trim() || null,
      firstName: newFirstName.value.trim(),
      lastName: newLastName.value.trim() || null,
      groupLabel: newGroup.value.trim() || null,
      teamName: newTeam.value.trim() || null,
    });
    newNumber.value = "";
    newFirstName.value = "";
    newLastName.value = "";
    newTeam.value = "";
    showAddForm.value = false;
    await Promise.all([loadRosterData(), eventStore.refreshActiveEvent()]);
  } catch (err) {
    errorMessage.value = String(err);
  }
}

async function runAutoMatch() {
  errorMessage.value = null;
  try {
    const count = await invoke<number>("auto_match_photos");
    bannerMessage.value =
      count > 0
        ? `Auto-matched ${count.toLocaleString()} photo(s) to roster ${subjectLabel.value.toLowerCase()}s.`
        : "No new number-prefixed folders matched the current roster.";
    await Promise.all([loadRosterData(), loadUnassignedFolders(), eventStore.refreshActiveEvent()]);
  } catch (err) {
    errorMessage.value = String(err);
  }
}

function selectParentPath(parent: string) {
  selectedParent.value = parent;
  selectedFolder.value = null;

  // Smart convenience: if the selected parent path contains a known group label
  // (e.g. "Session 1A"), auto-narrow the right-hand subject list to that group!
  const matchingGroup = groups.value.find((g) =>
    parent.toLowerCase().includes(g.toLowerCase())
  );
  matchGroupFilter.value = matchingGroup || "";
  loadUnassignedFolders();
}

async function assignSelectedFolder(subject: SubjectRecord) {
  if (!selectedFolder.value || isAssigning.value) return;
  isAssigning.value = true;
  errorMessage.value = null;
  try {
    const res = await invoke<AssignFolderResult>("assign_folder_to_subject", {
      folderPath: selectedFolder.value.folderPath,
      subjectId: subject.id,
      renameOnDisk: renameOnDisk.value,
    });
    bannerMessage.value = `Assigned ${res.photosUpdated} photo(s) from "${selectedFolder.value.leafFolder}" → ${subject.displayName}${res.renamedOnDisk ? " (folder renamed on disk)" : ""}.`;
    selectedFolder.value = null;
    await Promise.all([loadUnassignedFolders(), loadRosterData(), eventStore.refreshActiveEvent()]);
  } catch (err) {
    errorMessage.value = String(err);
  } finally {
    isAssigning.value = false;
  }
}

onMounted(() => {
  if (eventStore.activeEvent) {
    loadRosterData();
    loadUnassignedFolders();
  }
});

watch(
  () => eventStore.activeEvent?.id,
  () => {
    selectedParent.value = "";
    selectedFolder.value = null;
    if (eventStore.activeEvent) {
      loadRosterData();
      loadUnassignedFolders();
    }
  }
);

watch([rosterSearch, selectedGroup], () => loadRosterData());
watch([matchSearch, matchGroupFilter], () => loadMatchSubjects());
</script>

<template>
  <div class="max-w-7xl mx-auto p-6 space-y-5">
    <div v-if="!eventStore.activeEvent" class="p-4 rounded-xl bg-amber-50 border border-amber-200 text-sm text-amber-800">
      Open or create an Event Bundle first to import rosters and match folders.
    </div>

    <template v-else>
      <!-- Header & Sub-tabs -->
      <div class="flex flex-wrap items-center justify-between gap-4">
        <div>
          <h1 class="text-2xl font-bold text-slate-900">Roster & Folder Matching</h1>
          <p class="text-sm text-slate-500 mt-0.5">
            Import {{ subjectLabel.toLowerCase() }} rosters (supports multi-file <code>Session *.txt</code> or <code>.csv</code>) and link photo folders.
          </p>
        </div>

        <div class="flex items-center gap-2">
          <div class="inline-flex rounded-lg bg-slate-200 p-1 text-sm font-medium">
            <button
              @click="subTab = 'roster'"
              class="px-3.5 py-1.5 rounded-md transition-colors"
              :class="subTab === 'roster' ? 'bg-white text-slate-900 shadow-sm' : 'text-slate-600 hover:text-slate-900'"
            >
              Roster ({{ eventStore.activeEvent.subjectCount.toLocaleString() }})
            </button>
            <button
              @click="subTab = 'folders'; loadUnassignedFolders()"
              class="px-3.5 py-1.5 rounded-md transition-colors flex items-center gap-1.5"
              :class="subTab === 'folders' ? 'bg-white text-slate-900 shadow-sm' : 'text-slate-600 hover:text-slate-900'"
            >
              Folder Matching
              <span
                v-if="unassignedData.totalUnassignedPhotos > 0"
                class="px-1.5 py-0.5 text-xs rounded-full bg-amber-100 text-amber-800"
              >
                {{ unassignedData.totalUnassignedPhotos.toLocaleString() }}
              </span>
            </button>
          </div>
        </div>
      </div>

      <div v-if="bannerMessage" class="p-3.5 rounded-lg bg-emerald-50 border border-emerald-200 text-sm text-emerald-800 flex items-center justify-between">
        <span>{{ bannerMessage }}</span>
        <button @click="bannerMessage = null" class="text-emerald-600 font-bold ml-4">&times;</button>
      </div>
      <div v-if="errorMessage" class="p-3.5 rounded-lg bg-red-50 border border-red-200 text-sm text-red-700 flex items-center justify-between">
        <span>{{ errorMessage }}</span>
        <button @click="errorMessage = null" class="text-red-600 font-bold ml-4">&times;</button>
      </div>

      <!-- ================= ROSTER TAB ================= -->
      <div v-if="subTab === 'roster'" class="space-y-5">
        <div class="flex flex-wrap items-center justify-between gap-3">
          <div class="flex flex-wrap items-center gap-2">
            <input
              v-model="rosterSearch"
              type="search"
              :placeholder="`Search ${subjectLabel.toLowerCase()} by # or name...`"
              class="w-64 border border-slate-300 bg-white rounded-lg px-3 py-2 text-sm"
            />
            <select
              v-model="selectedGroup"
              class="border border-slate-300 bg-white rounded-lg px-3 py-2 text-sm"
            >
              <option value="">All Groups / Sessions ({{ groups.length }})</option>
              <option v-for="g in groups" :key="g" :value="g">{{ g }}</option>
            </select>
          </div>

          <div class="flex flex-wrap items-center gap-2">
            <button
              @click="runAutoMatch"
              class="px-3.5 py-2 text-sm font-medium rounded-lg border border-slate-300 bg-white text-slate-700 hover:bg-slate-50"
            >
              Auto-Match Photos
            </button>
            <button
              @click="showAddForm = !showAddForm"
              class="px-3.5 py-2 text-sm font-medium rounded-lg border border-slate-300 bg-white text-slate-700 hover:bg-slate-50"
            >
              + Add {{ subjectLabel }}
            </button>
            <button
              @click="pickRosterFiles"
              class="px-4 py-2 text-sm font-medium rounded-lg bg-blue-600 text-white hover:bg-blue-700"
            >
              Import Roster Files (.txt / .csv)
            </button>
          </div>
        </div>

        <!-- Import Preview Panel -->
        <div v-if="parsedFiles.length > 0" class="bg-white rounded-xl border-2 border-blue-200 shadow-sm p-5 space-y-4">
          <div class="flex flex-wrap items-center justify-between gap-3">
            <div>
              <h2 class="text-base font-semibold text-slate-900">
                Ready to Import {{ totalParsedEntries.toLocaleString() }} {{ subjectLabel }}(s) from {{ parsedFiles.length }} file(s)
              </h2>
              <p class="text-xs text-slate-500 mt-0.5">
                Group/Session labels default from each filename and can be adjusted below. Any existing scanned photos matching these numbers will auto-link immediately.
              </p>
            </div>
            <div class="flex gap-2">
              <button
                @click="parsedFiles = []"
                class="px-3 py-1.5 text-sm text-slate-600 hover:text-slate-900"
              >
                Cancel
              </button>
              <button
                @click="confirmImportRoster"
                :disabled="isImporting || totalParsedEntries === 0"
                class="px-4 py-1.5 text-sm font-medium rounded-lg bg-blue-600 text-white hover:bg-blue-700 disabled:bg-slate-400"
              >
                {{ isImporting ? "Importing..." : `Import ${totalParsedEntries.toLocaleString()} & Auto-Match` }}
              </button>
            </div>
          </div>

          <div class="max-h-60 overflow-y-auto border border-slate-200 rounded-lg divide-y divide-slate-100">
            <div
              v-for="file in parsedFiles"
              :key="file.filePath"
              class="px-3.5 py-2.5 flex flex-wrap items-center justify-between gap-3 text-sm"
            >
              <div class="min-w-0">
                <div class="font-medium text-slate-800 font-mono text-xs">{{ file.filename }}</div>
                <div class="text-xs text-slate-500">
                  {{ file.entries.length }} entries
                  <span v-if="file.entries.length > 0" class="text-slate-400">
                    (e.g. {{ file.entries.slice(0, 3).map(e => `${e.number ? e.number + ' ' : ''}${e.firstName} ${e.lastName || ''}`.trim()).join(', ') }})
                  </span>
                </div>
                <div v-if="file.warnings.length > 0" class="text-xs text-amber-600 mt-0.5">
                  {{ file.warnings.length }} warning(s): {{ file.warnings[0] }}
                </div>
              </div>
              <div class="flex items-center gap-2">
                <label class="text-xs text-slate-500">Group:</label>
                <input
                  v-model="fileGroupOverrides[file.filePath]"
                  type="text"
                  placeholder="Session / Group"
                  class="w-36 border border-slate-300 rounded px-2 py-1 text-xs"
                />
              </div>
            </div>
          </div>
        </div>

        <!-- Quick Manual Add -->
        <div v-if="showAddForm" class="bg-white rounded-xl border border-slate-200 shadow-sm p-5">
          <h3 class="text-sm font-semibold text-slate-800 mb-3">Add Single {{ subjectLabel }}</h3>
          <form @submit.prevent="handleManualAdd" class="grid grid-cols-2 md:grid-cols-6 gap-3 items-end">
            <div>
              <label class="block text-xs text-slate-500 mb-1">Number / Bib</label>
              <input v-model="newNumber" type="text" placeholder="1022" class="w-full border border-slate-300 rounded px-2.5 py-1.5 text-sm font-mono" />
            </div>
            <div>
              <label class="block text-xs text-slate-500 mb-1">First Name *</label>
              <input v-model="newFirstName" type="text" required placeholder="Avery" class="w-full border border-slate-300 rounded px-2.5 py-1.5 text-sm" />
            </div>
            <div>
              <label class="block text-xs text-slate-500 mb-1">Last Name</label>
              <input v-model="newLastName" type="text" placeholder="W" class="w-full border border-slate-300 rounded px-2.5 py-1.5 text-sm" />
            </div>
            <div>
              <label class="block text-xs text-slate-500 mb-1">Group / Session</label>
              <input v-model="newGroup" type="text" placeholder="Session 1A" class="w-full border border-slate-300 rounded px-2.5 py-1.5 text-sm" />
            </div>
            <div>
              <label class="block text-xs text-slate-500 mb-1">Team / Club</label>
              <input v-model="newTeam" type="text" placeholder="Optional" class="w-full border border-slate-300 rounded px-2.5 py-1.5 text-sm" />
            </div>
            <button type="submit" class="px-4 py-1.5 bg-blue-600 text-white text-sm font-medium rounded-lg hover:bg-blue-700">
              Add
            </button>
          </form>
        </div>

        <!-- Roster Table -->
        <div class="bg-white rounded-xl border border-slate-200 shadow-sm overflow-hidden">
          <div class="overflow-x-auto">
            <table class="min-w-full divide-y divide-slate-200 text-sm">
              <thead class="bg-slate-50 text-xs uppercase text-slate-500">
                <tr>
                  <th class="px-4 py-3 text-left font-medium">Number</th>
                  <th class="px-4 py-3 text-left font-medium">{{ subjectLabel }}</th>
                  <th class="px-4 py-3 text-left font-medium">Group / Session</th>
                  <th class="px-4 py-3 text-left font-medium">Team</th>
                  <th class="px-4 py-3 text-right font-medium">Ready Photos</th>
                </tr>
              </thead>
              <tbody class="divide-y divide-slate-100">
                <tr v-for="s in subjects" :key="s.id" class="hover:bg-slate-50">
                  <td class="px-4 py-2.5 font-mono text-slate-600">{{ s.number || "—" }}</td>
                  <td class="px-4 py-2.5 font-medium text-slate-900">{{ s.displayName }}</td>
                  <td class="px-4 py-2.5 text-slate-600">{{ s.groupLabel || "—" }}</td>
                  <td class="px-4 py-2.5 text-slate-500">{{ s.teamName || "—" }}</td>
                  <td class="px-4 py-2.5 text-right font-mono">
                    <span :class="s.photoCount > 0 ? 'text-emerald-700 font-semibold' : 'text-slate-400'">
                      {{ s.photoCount }}
                    </span>
                  </td>
                </tr>
                <tr v-if="subjects.length === 0">
                  <td colspan="5" class="px-4 py-10 text-center text-slate-500">
                    No {{ subjectLabel.toLowerCase() }}s found. Click <strong>Import Roster Files</strong> to load your <code>Session *.txt</code> or <code>.csv</code> files.
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>
      </div>

      <!-- ================= FOLDER MATCHING TAB ================= -->
      <div v-else class="space-y-4">
        <div class="flex flex-wrap items-center justify-between gap-3 bg-white p-4 rounded-xl border border-slate-200">
          <div class="flex flex-wrap items-center gap-3">
            <select
              :value="selectedParent"
              @change="selectParentPath(($event.target as HTMLSelectElement).value)"
              class="border border-slate-300 rounded-lg px-3 py-2 text-sm bg-white max-w-md"
            >
              <option value="">
                All Locations ({{ unassignedData.parents.length }} locations, {{ unassignedData.totalUnassignedPhotos.toLocaleString() }} unassigned photos)
              </option>
              <option v-for="p in unassignedData.parents" :key="p.parentPath" :value="p.parentPath">
                {{ p.parentPath || "(event root)" }} — {{ p.folderCount }} folders ({{ p.photoCount }} photos)
              </option>
            </select>

            <label class="flex items-center gap-2 text-sm text-slate-700 cursor-pointer">
              <input v-model="renameOnDisk" type="checkbox" class="rounded" />
              <span>Rename folder on disk to {{ subjectLabel.toLowerCase() }} name</span>
            </label>
          </div>

          <button
            @click="runAutoMatch"
            class="px-3.5 py-2 text-sm font-medium rounded-lg bg-blue-600 text-white hover:bg-blue-700"
          >
            Auto-Match Numbered Folders
          </button>
        </div>

        <div v-if="unassignedData.folders.length === 0" class="bg-white rounded-xl border border-slate-200 p-12 text-center">
          <div class="text-lg font-semibold text-emerald-700">All folders matched!</div>
          <p class="text-sm text-slate-500 mt-1">
            There are no unassigned photo folders in this view.
          </p>
        </div>

        <div v-else class="grid grid-cols-1 lg:grid-cols-2 gap-5">
          <!-- Left: Unassigned Folders with Live Thumbnails -->
          <div class="bg-white rounded-xl border border-slate-200 overflow-hidden flex flex-col max-h-[68vh]">
            <div class="px-4 py-3 bg-slate-50 border-b border-slate-200">
              <h2 class="text-sm font-semibold text-slate-800">
                Unassigned Folders ({{ unassignedData.folders.length }})
              </h2>
              <p class="text-xs text-slate-500">Click a folder to select it, then click the matching {{ subjectLabel.toLowerCase() }} on the right.</p>
            </div>
            <div class="p-3 space-y-2.5 overflow-y-auto flex-1">
              <button
                v-for="folder in unassignedData.folders"
                :key="folder.folderPath"
                @click="selectedFolder = folder"
                class="w-full text-left p-3 rounded-lg border transition-all space-y-2"
                :class="
                  selectedFolder?.folderPath === folder.folderPath
                    ? 'border-blue-500 bg-blue-50 ring-2 ring-blue-200'
                    : 'border-slate-200 hover:border-slate-300 hover:bg-slate-50'
                "
              >
                <div class="flex items-center justify-between gap-2">
                  <div class="min-w-0">
                    <div class="font-mono text-sm font-semibold text-slate-900 truncate">{{ folder.leafFolder }}</div>
                    <div class="text-xs text-slate-400 font-mono truncate">{{ folder.parentPath }}</div>
                  </div>
                  <span class="px-2 py-0.5 text-xs rounded-full bg-slate-100 text-slate-600 font-medium flex-shrink-0">
                    {{ folder.photoCount }} photos
                  </span>
                </div>
                <div v-if="folder.samplePhotoIds.length > 0 && eventStore.lanStatus.running" class="grid grid-cols-4 gap-1.5">
                  <img
                    v-for="pid in folder.samplePhotoIds"
                    :key="pid"
                    :src="`${thumbBaseUrl}/${pid}`"
                    loading="lazy"
                    class="w-full aspect-[3/4] object-cover rounded bg-slate-200"
                  />
                </div>
              </button>
            </div>
          </div>

          <!-- Right: Matching Subjects Picker -->
          <div class="bg-white rounded-xl border border-slate-200 overflow-hidden flex flex-col max-h-[68vh]">
            <div class="px-4 py-3 bg-slate-50 border-b border-slate-200 space-y-2">
              <div class="flex items-center justify-between">
                <h2 class="text-sm font-semibold text-slate-800">
                  {{ selectedFolder ? `Assign "${selectedFolder.leafFolder}" to:` : `Select a folder on the left` }}
                </h2>
              </div>
              <div class="flex gap-2">
                <input
                  v-model="matchSearch"
                  type="search"
                  :placeholder="`Filter ${subjectLabel.toLowerCase()} by # or name...`"
                  class="flex-1 border border-slate-300 bg-white rounded-md px-2.5 py-1.5 text-sm"
                />
                <select
                  v-model="matchGroupFilter"
                  class="border border-slate-300 bg-white rounded-md px-2.5 py-1.5 text-sm"
                >
                  <option value="">All Groups</option>
                  <option v-for="g in groups" :key="g" :value="g">{{ g }}</option>
                </select>
              </div>
            </div>

            <div class="divide-y divide-slate-100 overflow-y-auto flex-1">
              <button
                v-for="s in matchSubjects"
                :key="s.id"
                @click="assignSelectedFolder(s)"
                :disabled="!selectedFolder || isAssigning"
                class="w-full px-4 py-2.5 text-left flex items-center justify-between transition-colors"
                :class="
                  selectedFolder
                    ? 'hover:bg-blue-50 cursor-pointer'
                    : 'opacity-50 cursor-not-allowed'
                "
              >
                <div class="flex items-center gap-3 min-w-0">
                  <span class="font-mono text-xs text-slate-500 w-14 flex-shrink-0">{{ s.number || "—" }}</span>
                  <div class="min-w-0">
                    <div class="text-sm font-medium text-slate-900 truncate">{{ s.displayName }}</div>
                    <div v-if="s.groupLabel" class="text-xs text-slate-400">{{ s.groupLabel }}</div>
                  </div>
                </div>
                <span class="text-xs text-slate-500 font-mono flex-shrink-0">{{ s.photoCount }} photos</span>
              </button>
              <div v-if="matchSubjects.length === 0" class="p-8 text-center text-sm text-slate-500">
                No {{ subjectLabel.toLowerCase() }}s match this filter. Import a roster first or clear the group filter.
              </div>
            </div>
          </div>
        </div>
      </div>
    </template>
  </div>
</template>
