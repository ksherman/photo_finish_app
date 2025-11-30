<script setup lang="ts">
import { ref, computed, onMounted, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useSessionStore } from "@/stores/session";
import { useCardReaderStore } from "@/stores/cardReader";
import type { CardReaderInfo, DirectoryEntry } from "@/types";

const emit = defineEmits<{
  configured: [];
}>();

const sessionStore = useSessionStore();
const cardReaderStore = useCardReaderStore();

const selectedReader = ref<CardReaderInfo | null>(null);
const destination = ref("");
const photographer = ref("");
const cameraBrand = ref("");
const renamePrefix = ref("");
const autoRename = ref(false);

const cameraBrands = [
  { label: "Sony (DCIM)", value: "sony", folderPath: "DCIM" },
  { label: "Canon (DCIM)", value: "canon", folderPath: "DCIM" },
  { label: "Nikon (DCIM)", value: "nikon", folderPath: "DCIM" },
  { label: "Fujifilm (DCIM)", value: "fujifilm", folderPath: "DCIM" },
  { label: "Panasonic (DCIM)", value: "panasonic", folderPath: "DCIM" },
  { label: "Olympus (DCIM)", value: "olympus", folderPath: "DCIM" },
];

onMounted(async () => {
  await cardReaderStore.discoverReaders();
});

const hasExistingMapping = computed(() => {
  if (!selectedReader.value) return false;
  return !!sessionStore.getReaderMapping(selectedReader.value.reader_id);
});

const selectedCameraBrand = computed(() => {
  return cameraBrands.find((b) => b.value === cameraBrand.value);
});

const previewFileCount = ref(0);
const previewFolderCount = ref(0);

function selectReader(reader: CardReaderInfo) {
  selectedReader.value = reader;
  previewFileCount.value = 0;
  previewFolderCount.value = 0;

  // Load existing mapping if available
  const existing = sessionStore.getReaderMapping(reader.reader_id);
  if (existing) {
    destination.value = existing.destination;
    photographer.value = existing.photographer;
    cameraBrand.value = existing.cameraBrand;
    renamePrefix.value = existing.renamePrefix || "";
    autoRename.value = existing.autoRename || false;
    // Load preview for existing camera brand
    loadPreview(reader, existing.cameraFolderPath);
  } else {
    destination.value = "";
    photographer.value = "";
    cameraBrand.value = "";
    renamePrefix.value = "";
    autoRename.value = false;
  }
}

async function loadPreview(reader: CardReaderInfo, folderPath: string) {
  try {
    let basePath = reader.mount_point;
    if (folderPath) {
      basePath = `${basePath}/${folderPath}`;
    }

    const entries = await invoke<DirectoryEntry[]>("list_directory", {
      path: basePath,
    });

    const dirs = entries.filter((e) => e.is_directory);
    previewFolderCount.value = dirs.length;
    previewFileCount.value = dirs.reduce((sum, dir) => sum + dir.file_count, 0);
  } catch (error) {
    console.error("Failed to load preview:", error);
    previewFileCount.value = 0;
    previewFolderCount.value = 0;
  }
}

// Watch for camera brand changes and reload preview
watch(cameraBrand, (newBrand) => {
  if (selectedReader.value && newBrand) {
    const brand = cameraBrands.find((b) => b.value === newBrand);
    if (brand) {
      loadPreview(selectedReader.value, brand.folderPath);
    }
  }
});

async function selectDestination() {
  const selected = await open({
    directory: true,
    multiple: false,
    title: "Select Session Destination",
  });
  if (selected) {
    destination.value = selected as string;
  }
}

function save() {
  if (
    !selectedReader.value ||
    !destination.value ||
    !photographer.value ||
    !cameraBrand.value ||
    !selectedCameraBrand.value
  )
    return;

  sessionStore.setReaderMapping(
    selectedReader.value.reader_id,
    selectedReader.value.display_name,
    destination.value,
    photographer.value,
    cameraBrand.value,
    selectedCameraBrand.value.folderPath,
    renamePrefix.value,
    autoRename.value
  );

  sessionStore.setActiveReader(selectedReader.value.reader_id);
  cardReaderStore.selectReader(
    selectedReader.value,
    selectedCameraBrand.value.folderPath
  );

  emit("configured");
}

const isValid = computed(
  () =>
    selectedReader.value &&
    destination.value &&
    photographer.value &&
    cameraBrand.value
);
</script>

<template>
  <div class="bg-white rounded-lg shadow p-6">
    <h2 class="text-lg font-semibold text-gray-800 mb-4">Reader Setup</h2>

    <!-- Reader List -->
    <div class="mb-6">
      <label class="block text-sm font-medium text-gray-700 mb-2">
        Select Card Reader
      </label>

      <div v-if="cardReaderStore.cardReaders.length === 0" class="text-sm text-gray-500 p-4 bg-gray-50 rounded-lg">
        No card readers detected. Insert a memory card and wait a moment.
        <button
          @click="cardReaderStore.discoverReaders()"
          class="ml-2 text-blue-600 hover:underline"
        >
          Refresh
        </button>
      </div>

      <div v-else class="space-y-2">
        <button
          v-for="reader in cardReaderStore.cardReaders"
          :key="reader.reader_id"
          @click="selectReader(reader)"
          class="w-full text-left p-3 rounded-lg border transition-colors"
          :class="
            selectedReader?.reader_id === reader.reader_id
              ? 'border-blue-500 bg-blue-50'
              : 'border-gray-200 hover:border-gray-300'
          "
        >
          <div class="flex justify-between items-start">
            <div>
              <div class="font-medium text-gray-800">
                {{ reader.display_name }}
              </div>
              <div class="text-sm text-gray-500">
                {{ reader.volume_name }} - {{ reader.file_count }} photos
              </div>
              <div class="text-xs text-gray-400 font-mono truncate mt-1">
                {{ reader.mount_point }}
              </div>
            </div>
            <div
              v-if="sessionStore.getReaderMapping(reader.reader_id)"
              class="text-xs bg-green-100 text-green-800 px-2 py-1 rounded"
            >
              Configured
            </div>
          </div>
        </button>
      </div>
    </div>

    <!-- Configuration Form (shown when reader is selected) -->
    <template v-if="selectedReader">
      <div class="border-t pt-4 space-y-4">
        <div>
          <label class="block text-sm font-medium text-gray-700 mb-1">
            Destination Folder
          </label>
          <p class="text-xs text-gray-500 mb-2">
            Select the folder for this reader's photos (e.g.,
            /NAS/originals/event/gym-a/session-1)
          </p>
          <div class="flex gap-2">
            <input
              v-model="destination"
              type="text"
              readonly
              placeholder="Click Browse to select..."
              class="flex-1 border border-gray-300 rounded-lg px-3 py-2 bg-gray-50 text-sm"
            />
            <button
              @click="selectDestination"
              class="px-4 py-2 bg-gray-200 hover:bg-gray-300 rounded-lg text-sm font-medium"
            >
              Browse
            </button>
          </div>
        </div>

        <div>
          <label class="block text-sm font-medium text-gray-700 mb-1">
            Photographer Initials
          </label>
          <input
            v-model="photographer"
            type="text"
            placeholder="e.g., KDS"
            class="w-full border border-gray-300 rounded-lg px-3 py-2 focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
          />
        </div>

        <div>
          <label class="block text-sm font-medium text-gray-700 mb-1">
            Camera Brand
          </label>
          <p class="text-xs text-gray-500 mb-2">
            Select camera brand to determine folder structure on card
          </p>
          <select
            v-model="cameraBrand"
            class="w-full border border-gray-300 rounded-lg px-3 py-2 focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
          >
            <option value="">Select camera brand...</option>
            <option
              v-for="brand in cameraBrands"
              :key="brand.value"
              :value="brand.value"
            >
              {{ brand.label }}
            </option>
          </select>
        </div>

        <!-- Preview of files/folders found -->
        <div
          v-if="cameraBrand && (previewFileCount > 0 || previewFolderCount > 0)"
          class="bg-blue-50 border border-blue-200 rounded-lg p-3"
        >
          <div class="text-sm text-blue-800">
            <span class="font-medium">Found:</span>
            {{ previewFileCount }} photos in {{ previewFolderCount }} folders
          </div>
        </div>

        <div>
          <label class="block text-sm font-medium text-gray-700 mb-1">
            Folder Rename Prefix (Optional)
          </label>
          <p class="text-xs text-gray-500 mb-2">
            Auto-rename folders like "105MSDCF" → "Gymnast 05"
          </p>
          <input
            v-model="renamePrefix"
            type="text"
            placeholder="e.g., Gymnast"
            class="w-full border border-gray-300 rounded-lg px-3 py-2 focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
          />
          <label class="mt-3 flex items-center cursor-pointer">
            <input
              v-model="autoRename"
              type="checkbox"
              :disabled="!renamePrefix"
              class="w-4 h-4 text-blue-600 border-gray-300 rounded focus:ring-blue-500 disabled:opacity-50"
            />
            <span class="ml-2 text-sm text-gray-700">
              Auto-rename folders during copy
            </span>
          </label>
        </div>
      </div>

      <div class="mt-6 flex justify-end gap-3">
        <button
          v-if="hasExistingMapping"
          @click="sessionStore.removeReaderMapping(selectedReader.reader_id)"
          class="px-4 py-2 text-red-600 hover:text-red-800"
        >
          Remove Mapping
        </button>
        <button
          @click="save"
          :disabled="!isValid"
          class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:bg-gray-400 disabled:cursor-not-allowed"
        >
          {{ hasExistingMapping ? "Update" : "Save" }} & Start
        </button>
      </div>
    </template>
  </div>
</template>
