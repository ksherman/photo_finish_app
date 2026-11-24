<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import { formatCents, useKioskStore } from "../store";
import BrandMark from "@/components/BrandMark.vue";
import { rankAndFilterSubjects } from "@/utils/subjects";
import type { KioskSubject } from "../types";

const store = useKioskStore();

const query = ref("");
const selectedGroup = ref("");
const searchInputRef = ref<HTMLInputElement | null>(null);

const groups = computed(() => {
  const set = new Set<string>();
  for (const s of store.subjects) {
    if (s.groupLabel) set.add(s.groupLabel);
  }
  return Array.from(set).sort((a, b) =>
    a.localeCompare(b, undefined, { numeric: true, sensitivity: "base" })
  );
});

const marqueeRows = computed(() => {
  const ids = store.showcasePhotoIds;
  if (ids.length === 0) return [];
  const buckets: [string[], string[], string[]] = [[], [], []];
  ids.forEach((id, i) => buckets[i % 3].push(id));

  const buildTrack = (src: string[]) => {
    const base = src.length > 0 ? src : ids;
    const half: string[] = [];
    while (half.length < 12) half.push(...base);
    return [...half, ...half];
  };

  return [
    { key: "r1", heightClass: "h-[22vh]", trackClass: "kiosk-marquee-left", pathPrefix: "/photos/thumbnail", ids: buildTrack(buckets[0]) },
    { key: "r2", heightClass: "h-[56vh]", trackClass: "kiosk-marquee-right", pathPrefix: "/photos/preview", ids: buildTrack(buckets[1]) },
    { key: "r3", heightClass: "h-[22vh]", trackClass: "kiosk-marquee-left-slow", pathPrefix: "/photos/thumbnail", ids: buildTrack(buckets[2]) },
  ];
});

const filteredSubjects = computed<KioskSubject[]>(() => {
  const raw = query.value.trim();
  const group = selectedGroup.value;
  if (!raw && !group) return [];
  const pool = group ? store.subjects.filter((s) => s.groupLabel === group) : store.subjects;
  return rankAndFilterSubjects(pool, raw).slice(0, 60);
});

const subjectLabelLower = computed(() =>
  (store.activeEvent?.subjectLabel || "Competitor").toLowerCase()
);

onMounted(() => {
  if (store.activeEvent) {
    store.loadRoster();
    if (store.showcasePhotoIds.length === 0) {
      store.loadShowcase();
    }
  }
});
</script>

<template>
  <div class="min-h-screen bg-slate-950 relative overflow-hidden flex flex-col">
    <!-- Ambient 3-Row Photo Showcase Marquee -->
    <div
      v-if="marqueeRows.length > 0"
      class="absolute inset-0 flex flex-col gap-1.5 opacity-45 pointer-events-none overflow-hidden"
    >
      <div v-for="row in marqueeRows" :key="row.key" :class="row.heightClass" class="w-full overflow-hidden">
        <div :class="row.trackClass" class="flex gap-1.5 h-full w-max">
          <img
            v-for="(pid, idx) in row.ids"
            :key="`${row.key}-${idx}`"
            :src="`${row.pathPrefix}/${pid}`"
            decoding="async"
            class="h-full aspect-[2/3] shrink-0 object-cover bg-slate-800"
          />
        </div>
      </div>
    </div>

    <!-- Top Floating Cart Button -->
    <div v-if="store.cartCount > 0" class="relative z-20 flex justify-end p-5">
      <button
        @click="store.navigate({ name: 'cart' })"
        class="inline-flex items-center gap-2.5 px-5 py-3 rounded-full bg-indigo-600 text-white font-semibold shadow-xl hover:bg-indigo-500 active:scale-95 transition"
      >
        <span>View Cart ({{ store.cartCount }})</span>
        <span class="px-2 py-0.5 rounded-full bg-white/20 text-sm font-mono">
          {{ formatCents(store.subtotalCents) }}
        </span>
      </button>
    </div>

    <!-- Center Search Card -->
    <div class="relative z-10 flex-1 flex items-center justify-center px-4 py-8">
      <div class="w-full max-w-2xl bg-white/95 backdrop-blur-md rounded-3xl shadow-2xl p-6 sm:p-8 space-y-5">
        <div class="text-center space-y-1.5">
          <BrandMark class="w-12 h-12 mx-auto mb-2 drop-shadow-sm" />
          <h1 class="text-2xl sm:text-3xl font-extrabold text-slate-900 tracking-tight">
            {{ store.activeEvent?.name || "PhotoFinish" }}
          </h1>
          <p class="text-slate-500 text-sm sm:text-base">
            Find your photos — search by {{ subjectLabelLower }} number or name
          </p>
        </div>

        <div class="flex flex-col sm:flex-row gap-2.5">
          <div class="relative flex-1">
            <input
              ref="searchInputRef"
              v-model="query"
              type="search"
              autocomplete="off"
              autocorrect="off"
              spellcheck="false"
              placeholder="Type # or name (e.g. 105 or Julia)..."
              class="w-full pl-5 pr-11 py-4 text-lg rounded-2xl border-2 border-slate-200 bg-white text-slate-900 focus:border-indigo-500 focus:outline-none"
            />
            <button
              v-if="query"
              @click="query = ''; searchInputRef?.focus()"
              class="absolute right-3 top-1/2 -translate-y-1/2 w-8 h-8 rounded-full bg-slate-100 text-slate-500 hover:bg-slate-200 flex items-center justify-center text-lg"
            >
              &times;
            </button>
          </div>

          <select
            v-if="groups.length > 1"
            v-model="selectedGroup"
            class="px-4 py-4 text-base rounded-2xl border-2 border-slate-200 bg-white text-slate-700 focus:border-indigo-500 focus:outline-none sm:w-44"
          >
            <option value="">All Sessions</option>
            <option v-for="g in groups" :key="g" :value="g">{{ g }}</option>
          </select>
        </div>

        <div
          v-if="query.trim() || selectedGroup"
          class="rounded-2xl border border-slate-200 overflow-hidden bg-white max-h-[46vh] overflow-y-auto divide-y divide-slate-100"
        >
          <div v-if="filteredSubjects.length === 0" class="p-8 text-center text-slate-500">
            No {{ subjectLabelLower }}s found matching "{{ query || selectedGroup }}"
          </div>

          <button
            v-for="s in filteredSubjects"
            :key="s.id"
            @click="store.navigate({ name: 'subject', subjectId: s.id })"
            class="w-full px-5 py-3.5 text-left flex items-center justify-between gap-4 hover:bg-indigo-50 active:bg-indigo-100 transition-colors"
          >
            <div class="flex items-center gap-3.5 min-w-0">
              <div
                v-if="s.photoCount > 0"
                class="w-12 h-12 rounded-xl overflow-hidden bg-slate-100 border border-slate-200 flex-shrink-0"
              >
                <img
                  :src="`/photos/avatar/${s.id}`"
                  loading="lazy"
                  decoding="async"
                  class="w-full h-full object-cover object-top"
                />
              </div>
              <span
                v-if="s.number"
                class="px-2.5 py-1 rounded-lg bg-slate-100 font-mono text-base font-bold text-slate-800 flex-shrink-0 min-w-[3.5rem] text-center"
              >
                {{ s.number }}
              </span>
              <div class="min-w-0">
                <div class="text-lg font-semibold text-slate-900 truncate">
                  {{ s.firstName }} {{ s.lastName || "" }}
                </div>
                <div class="text-xs text-slate-500 flex items-center gap-2">
                  <span v-if="s.groupLabel">{{ s.groupLabel }}</span>
                  <span v-if="s.teamName">· {{ s.teamName }}</span>
                </div>
              </div>
            </div>

            <div class="flex items-center gap-2 flex-shrink-0">
              <span
                class="px-3 py-1 rounded-full text-sm font-semibold"
                :class="
                  s.photoCount > 0 ? 'bg-emerald-50 text-emerald-700' : 'bg-slate-100 text-slate-400'
                "
              >
                {{ s.photoCount }} photos
              </span>
              <span class="text-slate-400 text-xl">&rsaquo;</span>
            </div>
          </button>
        </div>

        <div v-else class="text-center text-xs text-slate-400 pt-1">
          {{ store.activeEvent?.photoCounts.ready.toLocaleString() || 0 }} photos ready across
          {{ store.subjects.length.toLocaleString() }} {{ subjectLabelLower }}s
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
@keyframes kiosk-left {
  0% { transform: translate3d(0, 0, 0); }
  100% { transform: translate3d(-50%, 0, 0); }
}
@keyframes kiosk-right {
  0% { transform: translate3d(-50%, 0, 0); }
  100% { transform: translate3d(0, 0, 0); }
}
.kiosk-marquee-left {
  will-change: transform;
  animation: kiosk-left 280s linear infinite;
}
.kiosk-marquee-right {
  will-change: transform;
  animation: kiosk-right 800s linear infinite;
}
.kiosk-marquee-left-slow {
  will-change: transform;
  animation: kiosk-left 340s linear infinite;
}
</style>
