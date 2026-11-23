<script setup lang="ts">
import { ref, onMounted } from "vue";
import { useEventStore } from "@/stores/event";
import ServerStatusBar from "@/components/ServerStatusBar.vue";
import BrandMark from "@/components/BrandMark.vue";
import EventsView from "@/views/EventsView.vue";
import IngestView from "@/views/IngestView.vue";
import RosterAndFoldersView from "@/views/RosterAndFoldersView.vue";
import PlaceholderView from "@/views/PlaceholderView.vue";

type Tab = "events" | "ingest" | "roster" | "orders" | "shootproof";

const activeTab = ref<Tab>("events");
const eventStore = useEventStore();

const navItems: { id: Tab; label: string }[] = [
  { id: "events", label: "Events" },
  { id: "ingest", label: "Card Ingest" },
  { id: "roster", label: "Roster & Folders" },
  { id: "orders", label: "Orders & USB" },
  { id: "shootproof", label: "ShootProof Sync" },
];

onMounted(() => {
  eventStore.bootstrap();
});
</script>

<template>
  <div class="min-h-screen flex bg-slate-100">
    <!-- Sidebar -->
    <aside class="w-56 bg-slate-900 text-slate-200 flex flex-col flex-shrink-0">
      <div class="px-4 py-4 border-b border-slate-800 flex items-center gap-3">
        <BrandMark class="w-9 h-9 shrink-0 drop-shadow-sm" />
        <div class="text-lg font-bold tracking-tight text-white truncate">PhotoFinish</div>
      </div>

      <nav class="p-3 space-y-1 flex-1">
        <button
          v-for="item in navItems"
          :key="item.id"
          @click="activeTab = item.id"
          class="w-full text-left px-3 py-2.5 rounded-lg text-sm font-medium transition-colors"
          :class="
            activeTab === item.id
              ? 'bg-blue-600 text-white'
              : 'text-slate-300 hover:bg-slate-800 hover:text-white'
          "
        >
          {{ item.label }}
        </button>
      </nav>
    </aside>

    <!-- Main Area -->
    <div class="flex-1 flex flex-col min-w-0">
      <ServerStatusBar />

      <main class="flex-1 overflow-y-auto">
        <EventsView v-if="activeTab === 'events'" @opened="activeTab = 'ingest'" />
        <IngestView v-else-if="activeTab === 'ingest'" />
        <RosterAndFoldersView v-else-if="activeTab === 'roster'" />
        <PlaceholderView
          v-else-if="activeTab === 'orders'"
          title="Orders & Instant USB-C/A Fulfillment"
          description="Manage on-site kiosk orders, Square QR / Terminal payments, and one-click USB drive copying."
          :items="[
            'v3 Pricing Ladder: Single Digital ($25) with 3-photo tipping-point upgrade → Full Digital + Prints + +$10 Dual USB-C/A',
            'Offline Square POS QR checkout + optional Square Terminal API',
            'Native high-speed USB drive detection & one-click order copy + eject',
            'Multi-event consolidated CSV export across gathered event bundles',
          ]"
        />
        <PlaceholderView
          v-else
          title="ShootProof Studio API Sync"
          description="Pivot on-site apparatus folders into subject-sorted online galleries and automate digital zip & lab print fulfillment."
          :items="[
            'Auto-create Event → Area → Subject albums via ShootProof Studio API',
            '20–50 concurrent S3 photo upload workers with resumable progress in photofinish.db',
            'Automated zip-bundle delivery for paid digital orders',
            'Push on-site print orders directly to WHCC / Bay Photo drop-ship',
          ]"
        />
      </main>
    </div>
  </div>
</template>
