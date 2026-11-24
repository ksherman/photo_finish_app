<script setup lang="ts">
defineProps<{
  title: string;
  description: string;
  items: string[];
}>();
</script>

<template>
  <div class="max-w-3xl mx-auto p-6">
    <div class="bg-white rounded-xl border border-slate-200 shadow-sm p-8 space-y-4">
      <span class="px-2.5 py-1 text-xs font-medium rounded-full bg-slate-100 text-slate-600">
        Next Milestone
      </span>
      <h1 class="text-2xl font-bold text-slate-900">{{ title }}</h1>
      <p class="text-sm text-slate-600">{{ description }}</p>
      <ul class="list-disc pl-5 text-sm text-slate-700 space-y-1.5 pt-2">
        <li v-for="item in items" :key="item">{{ item }}</li>
      </ul>
    </div>
  </div>
</template>
