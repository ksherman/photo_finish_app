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
  eventSlug: sessionStore.eventSlug || "",
  photographer: sessionStore.photographer || "",
  gym: sessionStore.gym || "",
  sessionNumber: sessionStore.sessionNumber || "",
  destinationRoot: sessionStore.destinationRoot || "",
});

async function selectDestination() {
  const selected = await open({
    directory: true,
    multiple: false,
    title: "Select Destination Root",
  });
  if (selected) {
    form.value.destinationRoot = selected as string;
  }
}

function save() {
  sessionStore.eventSlug = form.value.eventSlug;
  sessionStore.photographer = form.value.photographer;
  sessionStore.gym = form.value.gym;
  sessionStore.sessionNumber = form.value.sessionNumber;
  sessionStore.destinationRoot = form.value.destinationRoot;
  emit("configured");
}

const isValid = () =>
  form.value.eventSlug &&
  form.value.photographer &&
  form.value.gym &&
  form.value.sessionNumber &&
  form.value.destinationRoot;
</script>

<template>
  <div class="bg-white rounded-lg shadow p-6">
    <h2 class="text-lg font-semibold text-gray-800 mb-4">Session Setup</h2>

    <div class="space-y-4">
      <div>
        <label class="block text-sm font-medium text-gray-700 mb-1"
          >Event Slug</label
        >
        <input
          v-model="form.eventSlug"
          type="text"
          placeholder="e.g., st-valentines-meet-2025"
          class="w-full border border-gray-300 rounded-lg px-3 py-2 focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
        />
      </div>

      <div class="grid grid-cols-2 gap-4">
        <div>
          <label class="block text-sm font-medium text-gray-700 mb-1"
            >Photographer</label
          >
          <input
            v-model="form.photographer"
            type="text"
            placeholder="e.g., kds"
            class="w-full border border-gray-300 rounded-lg px-3 py-2 focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
          />
        </div>
        <div>
          <label class="block text-sm font-medium text-gray-700 mb-1"
            >Gym</label
          >
          <input
            v-model="form.gym"
            type="text"
            placeholder="e.g., gym-a"
            class="w-full border border-gray-300 rounded-lg px-3 py-2 focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
          />
        </div>
      </div>

      <div>
        <label class="block text-sm font-medium text-gray-700 mb-1"
          >Session Number</label
        >
        <input
          v-model="form.sessionNumber"
          type="text"
          placeholder="e.g., session-1"
          class="w-full border border-gray-300 rounded-lg px-3 py-2 focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
        />
      </div>

      <div>
        <label class="block text-sm font-medium text-gray-700 mb-1"
          >Destination Root</label
        >
        <div class="flex gap-2">
          <input
            v-model="form.destinationRoot"
            type="text"
            readonly
            placeholder="Click Browse to select..."
            class="flex-1 border border-gray-300 rounded-lg px-3 py-2 bg-gray-50"
          />
          <button
            @click="selectDestination"
            class="px-4 py-2 bg-gray-200 hover:bg-gray-300 rounded-lg text-sm font-medium"
          >
            Browse
          </button>
        </div>
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
        Save Session
      </button>
    </div>
  </div>
</template>
