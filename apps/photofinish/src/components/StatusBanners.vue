<script setup lang="ts">
defineProps<{
  bannerMessage?: string | null;
  errorMessage?: string | null;
}>();

const emit = defineEmits<{
  clearBanner: [];
  clearError: [];
}>();
</script>

<template>
  <div
    v-if="bannerMessage"
    class="p-3.5 rounded-lg bg-emerald-50 border border-emerald-200 text-sm text-emerald-800 flex flex-wrap items-center justify-between gap-3"
  >
    <span>{{ bannerMessage }}</span>
    <div class="flex items-center gap-2.5 ml-auto">
      <slot name="bannerAction" />
      <button @click="emit('clearBanner')" class="text-emerald-600 font-bold">&times;</button>
    </div>
  </div>

  <div
    v-if="errorMessage"
    class="p-3.5 rounded-lg bg-red-50 border border-red-200 text-sm text-red-700 flex items-center justify-between gap-4"
  >
    <span>{{ errorMessage }}</span>
    <button @click="emit('clearError')" class="text-red-500 font-bold">&times;</button>
  </div>
</template>
