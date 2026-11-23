<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import { formatCents, useKioskStore } from "../store";
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

const showcaseRows = computed(() => {
  const ids = store.showcasePhotoIds;
  if (ids.length === 0) return { row1: [] as string[], row2: [] as string[], row3: [] as string[] };
  const q1 = Math.max(1, Math.floor(ids.length / 4));
  const q3 = Math.max(1, Math.floor(ids.length / 4));
  const q2 = Math.max(1, ids.length - q1 - q3);
  const r1 = ids.slice(0, q1);
  const r2 = ids.slice(q1, q1 + q2);
  const r3 = ids.slice(q1 + q2);
  const repeat = (arr: string[]) => (arr.length ? [...arr, ...arr, ...arr, ...arr] : []);
  return {
    row1: repeat(r1),
    row2: repeat(r2.length ? r2 : r1),
    row3: repeat(r3.length ? r3 : r1),
  };
});

function bibNum(s: KioskSubject): number {
  if (!s.number) return Number.MAX_SAFE_INTEGER;
  const n = parseInt(s.number, 10);
  return Number.isNaN(n) ? Number.MAX_SAFE_INTEGER : n;
}

/// Instant 0ms client-side search with exact-bib -> bib-prefix -> name-prefix -> substring ranking
/// and proper numeric bib ordering (105 before 1000!).
const filteredSubjects = computed<KioskSubject[]>(() => {
  const raw = query.value.trim().toLowerCase();
  const group = selectedGroup.value;
  if (!raw && !group) return [];

  const matches: Array<{ subject: KioskSubject; rank: number; bib: number }> = [];
  for (const s of store.subjects) {
    if (group && s.groupLabel !== group) continue;
    if (!raw) {
      matches.push({ subject: s, rank: 0, bib: bibNum(s) });
      continue;
    }

    const num = (s.number || "").toLowerCase();
    const first = s.firstName.toLowerCase();
    const last = (s.lastName || "").toLowerCase();
    const disp = s.displayName.toLowerCase();
    const team = (s.teamName || "").toLowerCase();

    let rank = -1;
    if (num === raw) rank = 0;
    else if (num.startsWith(raw)) rank = 1;
    else if (first.startsWith(raw) || last.startsWith(raw)) rank = 2;
    else if (disp.includes(raw) || team.includes(raw)) rank = 3;

    if (rank >= 0) {
      matches.push({ subject: s, rank, bib: bibNum(s) });
    }
  }

  matches.sort((a, b) => {
    if (a.rank !== b.rank) return a.rank - b.rank;
    if (a.bib !== b.bib) return a.bib - b.bib;
    return a.subject.displayName.localeCompare(b.subject.displayName, undefined, {
      numeric: true,
      sensitivity: "base",
    });
  });

  return matches.slice(0, 60).map((m) => m.subject);
});

const subjectLabelLower = computed(() =>
  (store.activeEvent?.subjectLabel || "Competitor").toLowerCase()
);

onMounted(() => {
  // Refresh roster counts in background whenever returning to Home
  if (store.activeEvent) {
    store.loadRoster();
  }
});
</script>

<template>
  <div class="min-h-screen bg-slate-950 relative overflow-hidden flex flex-col">
    <!-- Ambient 3-Row Photo Showcase Marquee -->
    <div
      v-if="showcaseRows.row1.length > 0"
      class="absolute inset-0 flex flex-col gap-1.5 opacity-45 pointer-events-none"
    >
      <div class="h-[22vh] overflow-hidden">
        <div class="kiosk-marquee-left flex gap-1.5 h-full w-max">
          <img
            v-for="(pid, idx) in showcaseRows.row1"
            :key="`r1-${idx}`"
            :src="`/photos/thumbnail/${pid}`"
            class="h-full w-auto object-cover"
          />
        </div>
      </div>
      <div class="h-[56vh] overflow-hidden">
        <div class="kiosk-marquee-right flex gap-1.5 h-full w-max">
          <img
            v-for="(pid, idx) in showcaseRows.row2"
            :key="`r2-${idx}`"
            :src="`/photos/preview/${pid}`"
            class="h-full w-auto object-cover"
          />
        </div>
      </div>
      <div class="h-[22vh] overflow-hidden">
        <div class="kiosk-marquee-left-slow flex gap-1.5 h-full w-max">
          <img
            v-for="(pid, idx) in showcaseRows.row3"
            :key="`r3-${idx}`"
            :src="`/photos/thumbnail/${pid}`"
            class="h-full w-auto object-cover"
          />
        </div>
      </div>
    </div>

    <!-- Top Floating Cart Button (visible if family already started an order for sibling #1) -->
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
        <div class="text-center space-y-1">
          <h1 class="text-2xl sm:text-3xl font-extrabold text-slate-900 tracking-tight">
            {{ store.activeEvent?.name || "PhotoFinish" }}
          </h1>
          <p class="text-slate-500 text-sm sm:text-base">
            Find your photos — search by {{ subjectLabelLower }} number or name
          </p>
        </div>

        <!-- Search + Session Filter -->
        <div class="flex flex-col sm:flex-row gap-2.5">
          <div class="relative flex-1">
            <input
              ref="searchInputRef"
              v-model="query"
              type="search"
              autocomplete="off"
              autocorrect="off"
              spellcheck="false"
              :placeholder="`Type # or name (e.g. 105 or Julia)...`"
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

        <!-- Live Search Results -->
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
            class="w-full px-5 py-4 text-left flex items-center justify-between gap-4 hover:bg-indigo-50 active:bg-indigo-100 transition-colors"
          >
            <div class="flex items-center gap-3.5 min-w-0">
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
                  s.photoCount > 0
                    ? 'bg-emerald-50 text-emerald-700'
                    : 'bg-slate-100 text-slate-400'
                "
              >
                {{ s.photoCount }} photos
              </span>
              <span class="text-slate-400 text-xl">&rsaquo;</span>
            </div>
          </button>
        </div>

        <!-- Subtle footer stats when idle -->
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
  from {
    transform: translateX(0);
  }
  to {
    transform: translateX(-50%);
  }
}
@keyframes kiosk-right {
  from {
    transform: translateX(-50%);
  }
  to {
    transform: translateX(0);
  }
}
.kiosk-marquee-left {
  animation: kiosk-left 180s linear infinite;
}
.kiosk-marquee-right {
  animation: kiosk-right 260s linear infinite;
}
.kiosk-marquee-left-slow {
  animation: kiosk-left 220s linear infinite;
}
</style>
