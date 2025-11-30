<script setup lang="ts">
import { ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { useSessionStore } from "@/stores/session";

const emit = defineEmits<{
  configured: [];
  cancel: [];
}>();

const sessionStore = useSessionStore();

const form = ref({
  destination: sessionStore.destination || "",
  photographer: sessionStore.photographer || "",
});

async function selectDestination() {
  const selected = await open({
    directory: true,
    multiple: false,
    title: "Select Session Destination",
  });
  if (selected) {
    form.value.destination = selected as string;
  }
}

function save() {
  sessionStore.setDestination(form.value.destination);
  sessionStore.photographer = form.value.photographer;
  emit("configured");
}

const isValid = () => form.value.destination && form.value.photographer;
</script>

<template>
  <div class="bg-white rounded-lg shadow p-6">
    <h2 class="text-lg font-semibold text-gray-800 mb-4">Session Setup</h2>

    <div class="space-y-4">
      <div>
        <label class="block text-sm font-medium text-gray-700 mb-1"
          >Destination Folder</label
        >
        <p class="text-xs text-gray-500 mb-2">
          Select the folder for this session (e.g.,
          /NAS/originals/event/gym-a/session-1)
        </p>
        <div class="flex gap-2">
          <input
            v-model="form.destination"
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
        <label class="block text-sm font-medium text-gray-700 mb-1"
          >Photographer Initials</label
        >
        <input
          v-model="form.photographer"
          type="text"
          placeholder="e.g., KDS"
          class="w-full border border-gray-300 rounded-lg px-3 py-2 focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
        />
      </div>
    </div>

    <div class="mt-6 flex justify-end gap-3">
      <button
        v-if="sessionStore.isConfigured"
        @click="emit('cancel')"
        class="px-4 py-2 border border-gray-300 rounded-lg hover:bg-gray-50"
      >
        Cancel
      </button>
      <button
        @click="save"
        :disabled="!isValid()"
        class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:bg-gray-400 disabled:cursor-not-allowed"
      >
        Save
      </button>
    </div>
  </div>
</template>
