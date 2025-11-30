<script setup lang="ts">
import { useSessionStore } from "@/stores/session";

const sessionStore = useSessionStore();
</script>

<template>
  <div class="bg-white rounded-lg shadow p-4 mb-4">
    <h2 class="font-semibold text-gray-700 mb-2">Copy To</h2>

    <div v-if="sessionStore.destinationPath" class="space-y-2">
      <code
        class="block text-sm bg-gray-100 p-3 rounded-lg break-all text-gray-700"
      >
        {{ sessionStore.destinationPath }}/
      </code>

      <div class="flex items-center justify-between text-sm">
        <span class="text-gray-500">
          Batch #{{ String(sessionStore.currentOrderNumber).padStart(4, "0") }}
        </span>
        <div class="space-x-3">
          <button
            @click="sessionStore.resetOrder()"
            class="text-gray-500 hover:text-gray-700"
          >
            Reset to 0001
          </button>
          <button
            @click="sessionStore.incrementOrder()"
            class="text-blue-600 hover:text-blue-800"
          >
            Skip to next
          </button>
        </div>
      </div>
    </div>

    <div v-else class="text-sm text-gray-500">
      Configure session to see destination path
    </div>
  </div>
</template>
