<script setup lang="ts">
import { ref, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useEventStore } from "@/stores/event";
import { rankAndFilterSubjects } from "@/utils/subjects";
import type {
  AvatarMetrics,
  ImportRosterResult,
  ParsedRosterFile,
  RosterEntryInput,
  SubjectRecord,
} from "@/types";

const props = defineProps<{
  subjectLabel: string;
  groups: string[];
  allSubjects: SubjectRecord[];
  avatarBaseUrl: string;
  avatarVersion: number;
  avatarMetrics: AvatarMetrics | null;
  isGeneratingAvatars: boolean;
}>();

const emit = defineEmits<{
  refresh: [];
  autoMatch: [];
  generateAvatars: [];
  banner: [message: string];
  error: [message: string];
}>();

const eventStore = useEventStore();

const rosterSearch = ref("");
const selectedGroup = ref("");
const parsedFiles = ref<ParsedRosterFile[]>([]);
const fileGroupOverrides = ref<Record<string, string>>({});
const isImporting = ref(false);

const showAddForm = ref(false);
const newNumber = ref("");
const newFirstName = ref("");
const newLastName = ref("");
const newGroup = ref("");
const newTeam = ref("");

const totalParsedEntries = computed(() =>
  parsedFiles.value.reduce((sum, f) => sum + f.entries.length, 0)
);

const filteredRosterSubjects = computed(() => {
  const base = selectedGroup.value
    ? props.allSubjects.filter((s) => s.groupLabel === selectedGroup.value)
    : props.allSubjects;
  return rankAndFilterSubjects(base, rosterSearch.value).slice(0, 500);
});

async function pickRosterFiles() {
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
    for (const file of parsed) overrides[file.filePath] = file.defaultGroupLabel || "";
    fileGroupOverrides.value = overrides;
  } catch (err) {
    emit("error", String(err));
  }
}

async function confirmImportRoster() {
  if (parsedFiles.value.length === 0 || isImporting.value) return;
  isImporting.value = true;
  try {
    const entries: RosterEntryInput[] = [];
    for (const file of parsedFiles.value) {
      const overrideGroup = (fileGroupOverrides.value[file.filePath] || "").trim();
      for (const e of file.entries) {
        entries.push({ ...e, groupLabel: overrideGroup || e.groupLabel });
      }
    }
    const res = await invoke<ImportRosterResult>("import_roster", { entries });
    parsedFiles.value = [];
    fileGroupOverrides.value = {};
    emit(
      "banner",
      `Imported ${res.importedNew} new, updated ${res.updated} existing, and auto-matched ${res.autoMatchedPhotos.toLocaleString()} photo(s).`
    );
    emit("refresh");
  } catch (err) {
    emit("error", String(err));
  } finally {
    isImporting.value = false;
  }
}

async function handleManualAdd() {
  if (!newFirstName.value.trim()) return;
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
    emit("refresh");
  } catch (err) {
    emit("error", String(err));
  }
}
</script>

<template>
  <div class="space-y-5">
    <div class="flex flex-wrap items-center justify-between gap-3">
      <div class="flex flex-wrap items-center gap-2">
        <input
          v-model="rosterSearch"
          type="search"
          :placeholder="`Search ${subjectLabel.toLowerCase()} by # or name...`"
          class="w-64 border border-slate-300 bg-white rounded-lg px-3 py-2 text-sm"
        />
        <select v-model="selectedGroup" class="border border-slate-300 bg-white rounded-lg px-3 py-2 text-sm">
          <option value="">All Groups / Sessions ({{ groups.length }})</option>
          <option v-for="g in groups" :key="g" :value="g">{{ g }}</option>
        </select>
      </div>

      <div class="flex flex-wrap items-center gap-2">
        <button
          v-if="avatarMetrics && avatarMetrics.totalWithPhotos > 0"
          @click="emit('generateAvatars')"
          :disabled="isGeneratingAvatars"
          :title="`Engine: ${avatarMetrics.engine}. Click to generate missing square face/leotard portraits from ready previews.`"
          class="px-3.5 py-2 text-sm font-medium rounded-lg border border-slate-300 bg-white text-slate-700 hover:bg-slate-50 disabled:opacity-60 flex items-center gap-1.5"
        >
          <span v-if="isGeneratingAvatars" class="w-2 h-2 rounded-full bg-indigo-500 animate-pulse" />
          <span v-if="isGeneratingAvatars">
            Portraits ({{ avatarMetrics.cachedAvatars }}/{{ avatarMetrics.totalWithPhotos }})&hellip;
          </span>
          <span v-else-if="avatarMetrics.cachedAvatars < avatarMetrics.totalWithPhotos">
            Generate Portraits ({{ avatarMetrics.totalWithPhotos - avatarMetrics.cachedAvatars }})
          </span>
          <span v-else>Portraits Ready ({{ avatarMetrics.cachedAvatars }}) ✓</span>
        </button>
        <button
          @click="emit('autoMatch')"
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
          <button @click="parsedFiles = []" class="px-3 py-1.5 text-sm text-slate-600 hover:text-slate-900">
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
                (e.g. {{ file.entries.slice(0, 3).map((e) => `${e.number ? e.number + " " : ""}${e.firstName} ${e.lastName || ""}`.trim()).join(", ") }})
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
            <tr v-for="s in filteredRosterSubjects" :key="s.id" class="hover:bg-slate-50">
              <td class="px-4 py-2 font-mono text-slate-600">{{ s.number || "—" }}</td>
              <td class="px-4 py-2 font-medium text-slate-900">
                <div class="flex items-center gap-2.5">
                  <div class="w-9 h-9 rounded-lg overflow-hidden bg-slate-100 border border-slate-200 flex-shrink-0 flex items-center justify-center">
                    <img
                      v-if="s.photoCount > 0 && eventStore.lanStatus.running"
                      :src="`${avatarBaseUrl}/${s.id}?v=${avatarVersion}`"
                      loading="lazy"
                      class="w-full h-full object-cover object-top"
                    />
                    <span v-else class="text-[10px] font-mono text-slate-400">{{ s.number || "—" }}</span>
                  </div>
                  <span>{{ s.displayName }}</span>
                </div>
              </td>
              <td class="px-4 py-2 text-slate-600">{{ s.groupLabel || "—" }}</td>
              <td class="px-4 py-2 text-slate-500">{{ s.teamName || "—" }}</td>
              <td class="px-4 py-2 text-right font-mono">
                <span :class="s.photoCount > 0 ? 'text-emerald-700 font-semibold' : 'text-slate-400'">
                  {{ s.photoCount }}
                </span>
              </td>
            </tr>
            <tr v-if="filteredRosterSubjects.length === 0">
              <td colspan="5" class="px-4 py-10 text-center text-slate-500">
                No {{ subjectLabel.toLowerCase() }}s found. Click <strong>Import Roster Files</strong> to load your <code>Session *.txt</code> or <code>.csv</code> files.
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </div>
</template>
