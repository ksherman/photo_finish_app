<script setup lang="ts">
import { useEventStore } from "@/stores/event";

const eventStore = useEventStore();
</script>

<template>
  <header class="bg-white border-b border-slate-200 px-6 py-3 flex flex-wrap items-center justify-between gap-4">
    <div class="flex items-center gap-3 min-w-0">
      <div v-if="eventStore.activeEvent" class="min-w-0">
        <div class="flex items-center gap-2">
          <span class="font-semibold text-slate-900 truncate">{{ eventStore.activeEvent.name }}</span>
          <span class="px-2 py-0.5 text-xs font-mono rounded bg-slate-100 text-slate-600">
            {{ eventStore.activeEvent.orderCode }}-{{ eventStore.activeEvent.stationCode }}
          </span>
        </div>
        <div class="text-xs text-slate-500 flex items-center gap-3 mt-0.5">
          <span class="text-emerald-600 font-medium">
            {{ eventStore.activeEvent.photoCounts.ready }} ready
          </span>
          <span v-if="eventStore.activeEvent.photoCounts.pending > 0" class="text-blue-600 font-medium">
            {{ eventStore.activeEvent.photoCounts.pending }} processing...
          </span>
          <span v-if="eventStore.activeEvent.photoCounts.error > 0" class="text-red-600 font-medium">
            {{ eventStore.activeEvent.photoCounts.error }} errors
          </span>
          <span class="text-slate-400">•</span>
          <span>{{ eventStore.activeEvent.subjectCount }} {{ eventStore.activeEvent.subjectLabel.toLowerCase() }}s</span>
        </div>
      </div>
      <div v-else class="text-sm text-amber-700 bg-amber-50 border border-amber-200 px-3 py-1 rounded-md">
        No event bundle open — create or open an event to begin
      </div>
    </div>

    <div class="flex items-center gap-3">
      <button
        v-if="eventStore.activeEvent"
        @click="eventStore.scanActiveEvent()"
        :disabled="eventStore.isScanning"
        class="px-3 py-1.5 text-xs font-medium rounded-md border border-slate-300 bg-white text-slate-700 hover:bg-slate-50 disabled:opacity-50"
      >
        {{ eventStore.isScanning ? "Scanning..." : "Scan Folder" }}
      </button>

      <div class="flex items-center gap-2 pl-3 border-l border-slate-200">
        <span
          class="inline-block w-2.5 h-2.5 rounded-full"
          :class="eventStore.lanStatus.running ? 'bg-emerald-500' : 'bg-slate-300'"
        />
        <div class="text-xs">
          <div class="font-medium text-slate-700">
            Kiosk LAN {{ eventStore.lanStatus.running ? "Online" : "Stopped" }}
          </div>
          <div v-if="eventStore.lanStatus.running" class="font-mono text-blue-600">
            {{ eventStore.lanStatus.lanUrl || `http://localhost:${eventStore.lanStatus.port}` }}
          </div>
        </div>
        <button
          @click="eventStore.toggleLanServer()"
          class="ml-1 px-2.5 py-1 text-xs font-medium rounded-md border transition-colors"
          :class="
            eventStore.lanStatus.running
              ? 'border-slate-300 text-slate-600 hover:bg-slate-50'
              : 'border-emerald-600 bg-emerald-600 text-white hover:bg-emerald-700'
          "
        >
          {{ eventStore.lanStatus.running ? "Stop" : "Start" }}
        </button>
      </div>
    </div>
  </header>
</template>
