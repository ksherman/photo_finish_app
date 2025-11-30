<script setup lang="ts">
import { useCardReaderStore } from "@/stores/cardReader";

const cardReaderStore = useCardReaderStore();
</script>

<template>
  <div
    v-if="cardReaderStore.copyProgress"
    class="bg-white rounded-lg shadow p-4 mb-4"
  >
    <h2 class="font-semibold text-gray-700 mb-3">
      {{ cardReaderStore.isCopying ? "Copying..." : "Copy Complete" }}
    </h2>

    <!-- Progress bar -->
    <div class="w-full bg-gray-200 rounded-full h-3 mb-3">
      <div
        class="h-3 rounded-full transition-all duration-300"
        :class="cardReaderStore.isCopying ? 'bg-blue-600' : 'bg-green-500'"
        :style="{ width: `${cardReaderStore.copyProgress.percentage}%` }"
      ></div>
    </div>

    <!-- Stats -->
    <div class="flex justify-between text-sm text-gray-600 mb-2">
      <span>
        {{ cardReaderStore.copyProgress.copied }} /
        {{ cardReaderStore.copyProgress.total }} files
      </span>
      <span> {{ Math.round(cardReaderStore.copyProgress.percentage) }}% </span>
    </div>

    <!-- Current file -->
    <div
      v-if="cardReaderStore.isCopying"
      class="text-xs text-gray-500 truncate"
    >
      {{ cardReaderStore.copyProgress.current_file }}
    </div>

    <!-- Errors -->
    <div
      v-if="cardReaderStore.copyErrors.length > 0"
      class="mt-3 p-3 bg-red-50 rounded-lg"
    >
      <p class="text-sm font-medium text-red-700 mb-1">Errors:</p>
      <ul class="text-xs text-red-600 space-y-1">
        <li v-for="(error, i) in cardReaderStore.copyErrors" :key="i">
          {{ error }}
        </li>
      </ul>
    </div>
  </div>
</template>
