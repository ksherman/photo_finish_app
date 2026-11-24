<script setup lang="ts">
import { useIngestStore } from "@/stores/ingest";

defineProps<{
  title?: string;
  activeBatchId?: string | null;
  hint?: string;
  showExitAll?: boolean;
}>();

const emit = defineEmits<{
  selectBatch: [batchId: string];
  exitAll: [];
}>();

const ingestStore = useIngestStore();
</script>

<template>
  <div class="bg-white rounded-xl border border-indigo-200/80 shadow-xs px-4 py-3 space-y-2.5">
    <div class="flex flex-wrap items-center justify-between gap-2">
      <div class="flex items-center gap-2 text-xs font-semibold uppercase tracking-wide text-slate-600">
        <span>{{ title || "Card Naming Queue" }}</span>
        <span
          v-if="ingestStore.pendingNamingBatches.length > 0"
          class="px-2 py-0.5 rounded-full bg-amber-100 text-amber-800 font-semibold normal-case"
        >
          {{ ingestStore.pendingNamingBatches.length }} card{{ ingestStore.pendingNamingBatches.length === 1 ? "" : "s" }}
          {{ activeBatchId !== undefined ? "waiting" : "ready to name" }}
        </span>
        <span
          v-if="ingestStore.activeCopies.length > 0"
          class="px-2 py-0.5 rounded-full bg-blue-100 text-blue-800 font-semibold normal-case animate-pulse"
        >
          {{ ingestStore.activeCopies.length }} copying now
        </span>
      </div>

      <button
        v-if="showExitAll"
        @click="emit('exitAll')"
        class="text-xs font-medium text-indigo-700 hover:underline"
      >
        Show All Event Locations
      </button>
      <span v-else-if="hint" class="text-xs text-slate-400">{{ hint }}</span>
    </div>

    <div class="flex flex-wrap gap-2">
      <!-- Actively copying cards / folders -->
      <div
        v-for="copy in ingestStore.activeCopies"
        :key="copy.readerId"
        class="flex items-center gap-2.5 px-3 py-1.5 rounded-lg border border-blue-200 bg-blue-50/70 text-xs"
      >
        <span class="w-2 h-2 rounded-full bg-blue-500 animate-pulse shrink-0" />
        <div class="min-w-0">
          <div class="font-semibold text-blue-900 truncate max-w-[190px]">{{ copy.readerLabel }}</div>
          <div class="font-mono text-[11px] text-blue-700 truncate max-w-[230px]">
            {{ copy.destinationRelPath || "(event root)" }}
          </div>
        </div>
        <span class="font-mono font-semibold text-blue-700 pl-1">
          {{ Math.round(copy.progress?.percentage || 0) }}%
        </span>
      </div>

      <!-- Finished cards awaiting folder naming -->
      <button
        v-for="batch in ingestStore.pendingNamingBatches"
        :key="batch.id"
        @click="emit('selectBatch', batch.id)"
        class="group flex items-center gap-2.5 px-3 py-1.5 rounded-lg border text-left text-xs transition-all"
        :class="
          activeBatchId === batch.id
            ? 'border-indigo-500 bg-indigo-50 ring-2 ring-indigo-200'
            : 'border-amber-200 bg-amber-50/60 hover:bg-indigo-50 hover:border-indigo-300'
        "
      >
        <span
          class="px-1.5 py-0.5 rounded font-mono font-bold"
          :class="
            activeBatchId === batch.id
              ? 'bg-indigo-600 text-white'
              : 'bg-white border border-amber-200 text-slate-700'
          "
        >
          #{{ batch.sequenceNum }}
        </span>
        <div class="min-w-0">
          <div class="font-semibold text-slate-900 truncate max-w-[200px]">
            {{ batch.readerName || batch.volumeName || "Card" }}
          </div>
          <div class="font-mono text-[11px] text-slate-600 truncate max-w-[250px]">
            {{ batch.destinationRelPath || "(event root)" }}
          </div>
        </div>
        <div class="pl-1 text-right shrink-0">
          <div
            class="font-semibold"
            :class="
              activeBatchId === batch.id
                ? 'text-indigo-700'
                : 'text-amber-800 group-hover:text-indigo-700'
            "
          >
            {{
              activeBatchId !== undefined
                ? `${batch.unassignedFolders}/${batch.totalFolders} left`
                : `${batch.unassignedFolders} to name →`
            }}
          </div>
          <div class="text-[10px] text-slate-500">
            <template v-if="batch.pendingPhotos > 0">
              {{ batch.readyPhotos }}/{{ batch.totalPhotos }} thumbs
            </template>
            <template v-else>
              {{ batch.totalPhotos }} ready ✓
            </template>
          </div>
        </div>
      </button>
    </div>
  </div>
</template>
