<script setup lang="ts">
import { ref, onMounted, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useCardReaderStore } from "@/stores/cardReader";
import { useRosterStore } from "@/stores/roster";
import { useSessionStore } from "@/stores/session";
import type { FolderRename, Competitor } from "@/types";

const emit = defineEmits<{
  back: [];
}>();

const cardReaderStore = useCardReaderStore();
const rosterStore = useRosterStore();
const sessionStore = useSessionStore();

const folders = ref<FolderRename[]>([]);
const isApplying = ref(false);

onMounted(() => {
  // Initialize folders from card reader directories
  folders.value = cardReaderStore.directories.map((dir) => ({
    originalName: dir.name,
    originalPath: dir.path,
    newName: "",
    photoCount: dir.file_count,
    competitorId: undefined,
  }));
});

function selectCompetitor(folder: FolderRename, competitor: Competitor | null) {
  if (competitor) {
    folder.competitorId = competitor.id;
    folder.newName = competitor.displayName;
  } else {
    folder.competitorId = undefined;
    folder.newName = "";
  }
}

function autoAssign() {
  const sorted = rosterStore.sortedCompetitors;
  folders.value.forEach((folder, index) => {
    if (index < sorted.length && folder.photoCount > 0) {
      const competitor = sorted[index];
      folder.competitorId = competitor.id;
      folder.newName = competitor.displayName;
    }
  });
}

function clearAll() {
  folders.value.forEach((folder) => {
    folder.competitorId = undefined;
    folder.newName = "";
  });
}

const renamePrefix = computed(() => {
  return sessionStore.activeMapping?.renamePrefix || "";
});

function autoRenameWithPrefix() {
  if (!renamePrefix.value) {
    alert("No rename prefix configured for this reader");
    return;
  }

  folders.value.forEach((folder) => {
    // Extract number from folder name (e.g., "105MSDCF" → "05", "101CANON" → "01")
    const match = folder.originalName.match(/(\d{2,3})/);
    if (match) {
      const number = match[1].slice(-2); // Take last 2 digits
      folder.newName = `${renamePrefix.value} ${number}`;
      folder.competitorId = undefined;
    }
  });
}

const hasChanges = () =>
  folders.value.some((f) => f.newName && f.newName !== f.originalName);

async function applyRenames() {
  const toRename = folders.value.filter(
    (f) => f.newName && f.newName !== f.originalName
  );

  if (toRename.length === 0) return;

  isApplying.value = true;

  try {
    for (const folder of toRename) {
      await invoke("rename_folder", {
        sourcePath: folder.originalPath,
        newName: folder.newName,
      });
    }

    // Refresh directories
    await cardReaderStore.loadDirectories();

    emit("back");
  } catch (error) {
    console.error("Failed to rename folders:", error);
    alert(`Rename failed: ${error}`);
  } finally {
    isApplying.value = false;
  }
}
</script>

<template>
  <div class="min-h-screen bg-gray-100 p-6">
    <div class="max-w-3xl mx-auto">
      <!-- Header -->
      <div class="bg-white rounded-lg shadow p-4 mb-4">
        <div class="flex justify-between items-center">
          <div>
            <h1 class="text-xl font-bold text-gray-800">Rename Folders</h1>
            <p class="text-sm text-gray-500">
              Assign competitor names to camera folders
            </p>
          </div>
          <button @click="emit('back')" class="text-blue-600 hover:underline">
            Back to Main
          </button>
        </div>
      </div>

      <!-- Folder List -->
      <div class="bg-white rounded-lg shadow">
        <!-- Actions -->
        <div class="p-4 border-b flex justify-between items-center">
          <span class="text-sm text-gray-600">
            {{ folders.length }} folders
          </span>
          <div class="space-x-2">
            <button
              v-if="renamePrefix"
              @click="autoRenameWithPrefix"
              class="text-sm bg-blue-600 hover:bg-blue-700 text-white px-3 py-1.5 rounded-lg"
            >
              Auto-Rename ({{ renamePrefix }})
            </button>
            <button
              @click="autoAssign"
              class="text-sm bg-gray-200 hover:bg-gray-300 px-3 py-1.5 rounded-lg"
            >
              Auto-Assign Sequential
            </button>
            <button
              @click="clearAll"
              class="text-sm text-gray-600 hover:text-gray-800 px-3 py-1.5"
            >
              Clear All
            </button>
          </div>
        </div>

        <!-- Folder rows -->
        <div class="divide-y max-h-[60vh] overflow-y-auto">
          <div
            v-for="folder in folders"
            :key="folder.originalPath"
            class="p-4 flex items-center gap-4"
          >
            <!-- Original name -->
            <div class="w-32 text-sm font-mono text-gray-600">
              {{ folder.originalName }}
            </div>

            <!-- Arrow -->
            <svg
              class="w-5 h-5 text-gray-400"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M14 5l7 7m0 0l-7 7m7-7H3"
              />
            </svg>

            <!-- Competitor select -->
            <select
              :value="folder.competitorId || ''"
              @change="
                selectCompetitor(
                  folder,
                  rosterStore.competitors.find(
                    (c) => c.id === ($event.target as HTMLSelectElement).value
                  ) || null
                )
              "
              class="flex-1 border border-gray-300 rounded-lg px-3 py-2 text-sm"
            >
              <option value="">-- Select Competitor --</option>
              <option
                v-for="competitor in rosterStore.sortedCompetitors"
                :key="competitor.id"
                :value="competitor.id"
              >
                {{ competitor.displayName }} ({{ competitor.teamName }})
              </option>
            </select>

            <!-- Photo count -->
            <div class="w-24 text-right text-sm text-gray-500">
              {{ folder.photoCount }} photos
            </div>
          </div>
        </div>

        <!-- Apply button -->
        <div class="p-4 border-t bg-gray-50 flex justify-end gap-3">
          <button
            @click="emit('back')"
            class="px-4 py-2 border border-gray-300 rounded-lg hover:bg-gray-100"
          >
            Cancel
          </button>
          <button
            @click="applyRenames"
            :disabled="!hasChanges() || isApplying"
            class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:bg-gray-400 disabled:cursor-not-allowed"
          >
            {{ isApplying ? "Applying..." : "Apply Renames" }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
