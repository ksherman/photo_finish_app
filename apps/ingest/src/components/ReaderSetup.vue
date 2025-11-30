<script setup lang="ts">
import { ref, computed, onMounted, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useSessionStore } from "@/stores/session";
import { useCardReaderStore } from "@/stores/cardReader";
import type { CardReaderInfo, DirectoryEntry } from "@/types";

const props = defineProps<{
  readerId: string;
}>();

const emit = defineEmits<{
  close: [];
  saved: [];
}>();

const sessionStore = useSessionStore();
const cardReaderStore = useCardReaderStore();

const reader = computed(() => 
  cardReaderStore.cardReaders.find(r => r.reader_id === props.readerId)
);

const destination = ref("");
const photographer = ref("");
const cameraBrand = ref("");
const renamePrefix = ref("");
const autoRename = ref(false);
const fileRenamePrefix = ref("");

const cameraBrands = [
  { label: "Sony (DCIM)", value: "sony", folderPath: "DCIM" },
  { label: "Canon (DCIM)", value: "canon", folderPath: "DCIM" },
  { label: "Nikon (DCIM)", value: "nikon", folderPath: "DCIM" },
  { label: "Fujifilm (DCIM)", value: "fujifilm", folderPath: "DCIM" },
  { label: "Panasonic (DCIM)", value: "panasonic", folderPath: "DCIM" },
  { label: "Olympus (DCIM)", value: "olympus", folderPath: "DCIM" },
];

const selectedCameraBrand = computed(() => {
  return cameraBrands.find((b) => b.value === cameraBrand.value);
});

const previewFileCount = ref(0);
const previewFolderCount = ref(0);

// Initialize form data
onMounted(() => {
  if (!reader.value) return;
  
  const existing = sessionStore.getReaderMapping(props.readerId);
  if (existing) {
    destination.value = existing.destination;
    photographer.value = existing.photographer;
    cameraBrand.value = existing.cameraBrand;
    renamePrefix.value = existing.renamePrefix || "";
    autoRename.value = existing.autoRename || false;
    fileRenamePrefix.value = existing.fileRenamePrefix || "";
    loadPreview(reader.value, existing.cameraFolderPath);
  } else {
    destination.value = "";
    photographer.value = "";
    cameraBrand.value = "";
    renamePrefix.value = "";
    autoRename.value = false;
    fileRenamePrefix.value = "";
  }
});

async function loadPreview(readerInfo: CardReaderInfo, folderPath: string) {
  try {
    let basePath = readerInfo.mount_point;
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
  if (reader.value && newBrand) {
    const brand = cameraBrands.find((b) => b.value === newBrand);
    if (brand) {
      loadPreview(reader.value, brand.folderPath);
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
    !reader.value ||
    !destination.value ||
    !photographer.value ||
    !cameraBrand.value ||
    !selectedCameraBrand.value
  )
    return;

  sessionStore.setReaderMapping(
    props.readerId,
    reader.value.display_name,
    destination.value,
    photographer.value,
    cameraBrand.value,
    selectedCameraBrand.value.folderPath,
    renamePrefix.value,
    autoRename.value,
    fileRenamePrefix.value
  );

  emit("saved");
}

const isValid = computed(
  () =>
    reader.value &&
    destination.value &&
    photographer.value &&
    cameraBrand.value
);
</script>

<template>
  <div v-if="reader" class="bg-white rounded-lg shadow-xl overflow-hidden flex flex-col max-h-[90vh]">
    <div class="px-6 py-4 border-b border-gray-100 flex justify-between items-center bg-gray-50">
      <h2 class="text-lg font-semibold text-gray-800">
        Configure {{ reader.display_name }}
      </h2>
      <button @click="$emit('close')" class="text-gray-400 hover:text-gray-600">
        <svg xmlns="http://www.w3.org/2000/svg" class="h-6 w-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
        </svg>
      </button>
    </div>

    <div class="p-6 overflow-y-auto">
      <div class="space-y-4">
        <!-- Reader Info -->
        <div class="bg-blue-50 p-3 rounded-lg text-sm text-blue-900 mb-4">
           <div class="font-medium">Reader Details</div>
           <div class="text-blue-700 mt-1">Volume: {{ reader.volume_name }}</div>
           <div class="text-blue-700 font-mono text-xs">{{ reader.mount_point }}</div>
        </div>
      
        <div>
          <label class="block text-sm font-medium text-gray-700 mb-1">
            Destination Folder
          </label>
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

        <!-- Preview -->
        <div
          v-if="cameraBrand && (previewFileCount > 0 || previewFolderCount > 0)"
          class="bg-green-50 border border-green-200 rounded-lg p-3 text-sm text-green-800"
        >
          <span class="font-medium">Found:</span>
          {{ previewFileCount }} photos in {{ previewFolderCount }} folders
        </div>

        <div>
          <label class="block text-sm font-medium text-gray-700 mb-1">
            File Rename Prefix (Optional)
          </label>
          <p class="text-xs text-gray-500 mb-2">
            Renames files like "IMG_1234.jpg" → "Prefix_1234.jpg"
          </p>
          <input
            v-model="fileRenamePrefix"
            type="text"
            placeholder="e.g., Gymnast"
            class="w-full border border-gray-300 rounded-lg px-3 py-2 focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
          />
        </div>

        <div>
          <label class="block text-sm font-medium text-gray-700 mb-1">
            Folder Rename Prefix (Optional)
          </label>
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
    </div>

    <div class="px-6 py-4 border-t border-gray-100 bg-gray-50 flex justify-end gap-3">
        <button
          @click="$emit('close')"
          class="px-4 py-2 text-gray-600 hover:text-gray-800 font-medium"
        >
          Cancel
        </button>
        <button
          @click="save"
          :disabled="!isValid"
          class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:bg-gray-400 disabled:cursor-not-allowed font-medium shadow-sm"
        >
          Save Configuration
        </button>
    </div>
  </div>
</template>
