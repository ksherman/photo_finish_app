<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from "vue";
import { ask, open } from "@tauri-apps/plugin-dialog";
import { useEventStore } from "@/stores/event";
import { useIngestStore } from "@/stores/ingest";
import ReaderDashboardCard from "@/components/ReaderDashboardCard.vue";
import ReaderSetupModal from "@/components/ReaderSetupModal.vue";
import type { CardReaderInfo, ReaderLayoutUpdate } from "@/types";

const eventStore = useEventStore();
const ingestStore = useIngestStore();

const configuringReader = ref<CardReaderInfo | null>(null);
const configuringDefaultGroup = ref<string | null>(null);
const isEjectingAll = ref(false);
let pollTimer: ReturnType<typeof setInterval> | null = null;

// Optional Local Folder Ingest modal (great for thumb drives / AirDrop drops OR testing multi-card queues)
const showFolderIngestModal = ref(false);
const localSourcePath = ref("");
const localDestRelPath = ref("");
const localLabel = ref("");
const localPhotographer = ref("");
const localAutoRename = ref(true);
const localRenamePrefix = ref("Gymnast");
const localIngestError = ref<string | null>(null);

// Group creation & inline rename state
const showNewGroupInput = ref(false);
const newGroupName = ref("");
const renamingGroup = ref<string | null>(null);
const renameGroupDraft = ref("");

// Pointer-based drag-and-drop state (works reliably in macOS WKWebView regardless of Tauri OS file-drop)
const draggingReader = ref<CardReaderInfo | null>(null);
const dragX = ref(0);
const dragY = ref(0);
const hoverGroup = ref<string | null>(null);
const hoverTargetReaderId = ref<string | null>(null);
const hoverDropSide = ref<"left" | "right" | null>(null);

const onlineReaders = computed(() =>
  ingestStore.cardReaders.filter((r) => r.volume_name !== "Offline" && Boolean(r.disk_id))
);

const anyReaderCopying = computed(() =>
  onlineReaders.value.some((r) => ingestStore.getCopyState(r.reader_id).isCopying)
);

/// Merge live online readers with persisted offline reader configs, preserving stable
/// `(groupName, sortOrder)` slots so cards NEVER jump around when pulling an SD card out!
const allReaders = computed<CardReaderInfo[]>(() => {
  const readers = [...ingestStore.cardReaders];
  for (const [readerId, cfg] of Object.entries(ingestStore.readerConfigs)) {
    if (!readers.some((r) => r.reader_id === readerId)) {
      readers.push({
        reader_id: readerId,
        display_name: cfg.displayName,
        mount_point: "Not Connected",
        volume_name: "Offline",
        bus_protocol: "Unknown",
        is_internal: false,
        disk_id: "",
        file_count: 0,
        folder_count: 0,
      });
    }
  }
  return readers;
});

function sortReadersInBucket(list: CardReaderInfo[]): CardReaderInfo[] {
  return [...list].sort((a, b) => {
    const cfgA = ingestStore.readerConfigs[a.reader_id];
    const cfgB = ingestStore.readerConfigs[b.reader_id];
    // Configured readers come first in their exact locked sortOrder; unconfigured follow
    if (Boolean(cfgA) !== Boolean(cfgB)) return cfgA ? -1 : 1;
    if (cfgA && cfgB && cfgA.sortOrder !== cfgB.sortOrder) {
      return cfgA.sortOrder - cfgB.sortOrder;
    }
    const labelA = cfgA?.customLabel || a.display_name;
    const labelB = cfgB?.customLabel || b.display_name;
    return labelA.localeCompare(labelB, undefined, { numeric: true, sensitivity: "base" });
  });
}

interface ReaderGroupSection {
  name: string;
  readers: CardReaderInfo[];
  onlineCount: number;
}

const namedGroups = computed<ReaderGroupSection[]>(() => {
  return ingestStore.groupOrder.map((groupName) => {
    const bucket = sortReadersInBucket(
      allReaders.value.filter((r) => {
        const cfg = ingestStore.readerConfigs[r.reader_id];
        return (cfg?.groupName || null) === groupName;
      })
    );
    const onlineCount = bucket.filter((r) => r.volume_name !== "Offline").length;
    return { name: groupName, readers: bucket, onlineCount };
  });
});

const ungroupedReaders = computed<CardReaderInfo[]>(() => {
  return sortReadersInBucket(
    allReaders.value.filter((r) => {
      const cfg = ingestStore.readerConfigs[r.reader_id];
      const g = cfg?.groupName?.trim() || "";
      return !g || !ingestStore.groupOrder.includes(g);
    })
  );
});

const draggingLabel = computed(() => {
  if (!draggingReader.value) return "";
  const cfg = ingestStore.readerConfigs[draggingReader.value.reader_id];
  return cfg?.customLabel?.trim() || draggingReader.value.display_name;
});

async function createGroup() {
  const name = newGroupName.value.trim();
  if (!name) return;
  await ingestStore.addGroup(name);
  newGroupName.value = "";
  showNewGroupInput.value = false;
}

function startRenameGroup(name: string) {
  renamingGroup.value = name;
  renameGroupDraft.value = name;
}

async function commitRenameGroup(oldName: string) {
  const next = renameGroupDraft.value.trim();
  renamingGroup.value = null;
  if (next && next !== oldName) {
    await ingestStore.renameGroup(oldName, next);
  }
}

async function confirmRemoveGroup(name: string) {
  const sec = namedGroups.value.find((g) => g.name === name);
  const count = sec?.readers.length || 0;
  if (count > 0) {
    const ok = await ask(
      `Remove group "${name}"? Its ${count} reader(s) will stay saved and move to Ungrouped.`,
      { title: "Remove Group", kind: "warning" }
    );
    if (!ok) return;
  }
  await ingestStore.removeGroup(name);
}

async function nudgeReaderInList(
  bucket: CardReaderInfo[],
  groupName: string | null,
  readerId: string,
  delta: -1 | 1
) {
  const configured = bucket.filter((r) => Boolean(ingestStore.readerConfigs[r.reader_id]));
  const idx = configured.findIndex((r) => r.reader_id === readerId);
  const target = idx + delta;
  if (idx < 0 || target < 0 || target >= configured.length) return;
  const reordered = [...configured];
  const [moved] = reordered.splice(idx, 1);
  reordered.splice(target, 0, moved);

  const updates: ReaderLayoutUpdate[] = reordered.map((r, i) => ({
    readerId: r.reader_id,
    groupName,
    sortOrder: i,
  }));
  await ingestStore.saveReaderLayout(updates);
}

// =========================================================================
// Pointer Drag-and-Drop (Reorder Left-to-Right & Move Across Groups)
// =========================================================================

function startDrag(reader: CardReaderInfo, event: PointerEvent) {
  draggingReader.value = reader;
  dragX.value = event.clientX;
  dragY.value = event.clientY;
  hoverGroup.value = null;
  hoverTargetReaderId.value = null;
  hoverDropSide.value = null;
  window.addEventListener("pointermove", onPointerMove);
  window.addEventListener("pointerup", onPointerUp);
  window.addEventListener("pointercancel", cleanupDrag);
}

function onPointerMove(e: PointerEvent) {
  if (!draggingReader.value) return;
  dragX.value = e.clientX;
  dragY.value = e.clientY;

  const els = document.elementsFromPoint(e.clientX, e.clientY);
  let foundGroup: string | null = null;
  let foundCardEl: HTMLElement | null = null;

  for (const el of els) {
    if (!foundCardEl && el instanceof HTMLElement && el.dataset.dropReaderId) {
      if (el.dataset.dropReaderId !== draggingReader.value.reader_id) {
        foundCardEl = el;
      }
    }
    if (foundGroup === null && el instanceof HTMLElement && el.dataset.dropGroup !== undefined) {
      foundGroup = el.dataset.dropGroup;
    }
  }

  hoverGroup.value = foundGroup;
  if (foundCardEl && foundCardEl.dataset.dropReaderId) {
    const rect = foundCardEl.getBoundingClientRect();
    hoverTargetReaderId.value = foundCardEl.dataset.dropReaderId;
    hoverDropSide.value = e.clientX < rect.left + rect.width / 2 ? "left" : "right";
  } else {
    hoverTargetReaderId.value = null;
    hoverDropSide.value = null;
  }
}

async function onPointerUp() {
  const dragged = draggingReader.value;
  const targetGroupRaw = hoverGroup.value;
  const targetCardId = hoverTargetReaderId.value;
  const dropSide = hoverDropSide.value;
  cleanupDrag();

  if (!dragged || targetGroupRaw === null) return;

  const targetGroup: string | null = targetGroupRaw === "" ? null : targetGroupRaw;
  const draggedCfg = ingestStore.readerConfigs[dragged.reader_id];

  // If dragging an unconfigured reader into a named group, open setup pre-filled with that group!
  if (!draggedCfg) {
    configuringDefaultGroup.value = targetGroup;
    configuringReader.value = dragged;
    return;
  }

  const sourceGroup: string | null = draggedCfg.groupName?.trim() || null;

  // Build ordered configured reader IDs for the destination bucket (excluding dragged)
  const destBucketIds = sortReadersInBucket(
    allReaders.value.filter((r) => {
      const cfg = ingestStore.readerConfigs[r.reader_id];
      if (!cfg || r.reader_id === dragged.reader_id) return false;
      const g = cfg.groupName?.trim() || null;
      return g === targetGroup;
    })
  ).map((r) => r.reader_id);

  let insertIndex = destBucketIds.length;
  if (targetCardId) {
    const idx = destBucketIds.indexOf(targetCardId);
    if (idx >= 0) {
      insertIndex = dropSide === "left" ? idx : idx + 1;
    }
  }
  destBucketIds.splice(insertIndex, 0, dragged.reader_id);

  const updates: ReaderLayoutUpdate[] = destBucketIds.map((id, idx) => ({
    readerId: id,
    groupName: targetGroup,
    sortOrder: idx,
  }));

  // Re-index remaining readers in source group if moved across groups
  if (sourceGroup !== targetGroup) {
    const srcBucketIds = sortReadersInBucket(
      allReaders.value.filter((r) => {
        const cfg = ingestStore.readerConfigs[r.reader_id];
        if (!cfg || r.reader_id === dragged.reader_id) return false;
        const g = cfg.groupName?.trim() || null;
        return g === sourceGroup;
      })
    ).map((r) => r.reader_id);
    srcBucketIds.forEach((id, idx) => {
      updates.push({ readerId: id, groupName: sourceGroup, sortOrder: idx });
    });
  }

  await ingestStore.saveReaderLayout(updates);
}

function cleanupDrag() {
  draggingReader.value = null;
  hoverGroup.value = null;
  hoverTargetReaderId.value = null;
  hoverDropSide.value = null;
  window.removeEventListener("pointermove", onPointerMove);
  window.removeEventListener("pointerup", onPointerUp);
  window.removeEventListener("pointercancel", cleanupDrag);
}

function openConfigure(reader: CardReaderInfo, defaultGroup: string | null = null) {
  configuringDefaultGroup.value = defaultGroup;
  configuringReader.value = reader;
}

async function ejectAllCards() {
  if (isEjectingAll.value || anyReaderCopying.value) return;
  isEjectingAll.value = true;
  try {
    for (const r of onlineReaders.value) {
      try {
        await ingestStore.ejectReader(r.disk_id);
      } catch (err) {
        console.error(`Failed to eject ${r.volume_name}:`, err);
      }
    }
  } finally {
    isEjectingAll.value = false;
  }
}

async function pickLocalSourceFolder() {
  const selected = await open({
    directory: true,
    multiple: false,
    title: "Select Source Folder (e.g. DCIM or Camera Card Folder)",
  });
  if (typeof selected === "string") {
    localSourcePath.value = selected;
    if (!localLabel.value.trim()) {
      localLabel.value = selected.split("/").filter(Boolean).pop() || "Manual Folder";
    }
  }
}

async function startLocalFolderIngest() {
  localIngestError.value = null;
  if (!localSourcePath.value.trim()) {
    localIngestError.value = "Choose a source folder to ingest.";
    return;
  }
  const params = {
    sourcePath: localSourcePath.value.trim(),
    destinationRelPath: localDestRelPath.value.trim().replace(/^\/+|\/+$/g, ""),
    label: localLabel.value.trim() || "Folder Ingest",
    photographer: localPhotographer.value.trim() || "Manual",
    autoRename: localAutoRename.value,
    renamePrefix: localRenamePrefix.value.trim() || "Gymnast",
  };
  showFolderIngestModal.value = false;
  localSourcePath.value = "";
  localDestRelPath.value = "";
  localLabel.value = "";
  // Fire non-blocking so operators can queue multiple folders concurrently!
  ingestStore
    .ingestFromLocalFolder(params)
    .then(() => eventStore.refreshActiveEvent())
    .catch((err) => {
      console.error("Local folder ingest failed:", err);
    });
}

onMounted(async () => {
  await Promise.all([
    ingestStore.loadReaderConfigs(),
    ingestStore.discoverReaders(),
    eventStore.activeEvent ? ingestStore.loadIngestBatches() : Promise.resolve(),
  ]);
  pollTimer = setInterval(async () => {
    await ingestStore.discoverReaders();
    if (eventStore.activeEvent) {
      if (eventStore.activeEvent.photoCounts.pending > 0) {
        await Promise.all([
          eventStore.refreshActiveEvent(),
          ingestStore.loadIngestBatches(),
        ]);
      }
    }
  }, 3000);
});

onUnmounted(() => {
  if (pollTimer) clearInterval(pollTimer);
  cleanupDrag();
});

async function confirmResetConfigs() {
  const ok = await ask("Clear all persisted card reader configurations and groups?", {
    title: "Reset Reader Configs",
    kind: "warning",
  });
  if (ok) {
    await ingestStore.clearAllConfigs();
  }
}
</script>

<template>
  <div class="max-w-[1600px] mx-auto p-6 space-y-6">
    <!-- Top Header -->
    <div class="flex flex-wrap items-center justify-between gap-4">
      <div>
        <h1 class="text-2xl font-bold text-slate-900">Card Ingest</h1>
        <p class="text-sm text-slate-500 mt-0.5">
          Organize card readers by group (e.g. Gym A: Floor → Beam) via drag-and-drop. Layouts stay locked to each physical USB reader ID.
        </p>
      </div>
      <div class="flex flex-wrap items-center gap-2.5">
        <div v-if="showNewGroupInput" class="flex items-center gap-1.5 bg-white border border-blue-300 rounded-lg p-1 shadow-xs">
          <input
            v-model="newGroupName"
            @keydown.enter.prevent="createGroup"
            @keydown.escape="showNewGroupInput = false"
            type="text"
            placeholder="e.g. Gym A"
            autofocus
            class="w-36 px-2 py-1 text-sm focus:outline-none"
          />
          <button
            @click="createGroup"
            class="px-2.5 py-1 text-xs font-semibold rounded bg-blue-600 text-white hover:bg-blue-700"
          >
            Add
          </button>
          <button
            @click="showNewGroupInput = false"
            class="px-1.5 py-1 text-xs text-slate-400 hover:text-slate-600"
          >
            &times;
          </button>
        </div>
        <button
          v-else
          @click="showNewGroupInput = true"
          class="px-3 py-1.5 text-sm font-medium rounded-lg border border-blue-200 bg-blue-50 text-blue-700 hover:bg-blue-100"
        >
          + New Group
        </button>

        <button
          v-if="eventStore.activeEvent"
          @click="showFolderIngestModal = true; localIngestError = null"
          title="Ingest photos from a local folder or thumb drive into the active event"
          class="px-3 py-1.5 text-sm font-medium rounded-lg border border-slate-300 bg-white text-slate-700 hover:bg-slate-50"
        >
          Ingest Folder&hellip;
        </button>

        <button
          v-if="onlineReaders.length > 1"
          @click="ejectAllCards"
          :disabled="isEjectingAll || anyReaderCopying"
          class="px-3 py-1.5 text-sm font-medium rounded-lg border border-slate-300 bg-white text-slate-700 hover:bg-slate-50 disabled:opacity-40"
        >
          {{ isEjectingAll ? "Ejecting..." : `⏏ Eject All (${onlineReaders.length})` }}
        </button>
        <button
          @click="ingestStore.discoverReaders()"
          class="px-3 py-1.5 text-sm rounded-lg border border-slate-300 bg-white text-slate-700 hover:bg-slate-50"
        >
          Refresh Readers
        </button>
        <button
          v-if="Object.keys(ingestStore.readerConfigs).length > 0 || ingestStore.groupOrder.length > 0"
          @click="confirmResetConfigs"
          class="px-3 py-1.5 text-sm text-red-600 hover:underline"
        >
          Reset All
        </button>
      </div>
    </div>

    <div
      v-if="!eventStore.activeEvent"
      class="p-4 rounded-xl bg-amber-50 border border-amber-200 text-sm text-amber-800"
    >
      Open or create an Event Bundle in the <strong>Events</strong> tab before ingesting memory cards.
    </div>

    <!-- Live Card Naming Queue Strip (Active Downloads + Finished Cards Awaiting Naming) -->
    <div
      v-if="eventStore.activeEvent && (ingestStore.activeCopies.length > 0 || ingestStore.pendingNamingBatches.length > 0)"
      class="bg-white rounded-xl border border-indigo-200/80 shadow-xs px-4 py-3 space-y-2.5"
    >
      <div class="flex flex-wrap items-center justify-between gap-2">
        <div class="flex items-center gap-2 text-xs font-semibold uppercase tracking-wide text-slate-600">
          <span>Card Naming Queue</span>
          <span
            v-if="ingestStore.pendingNamingBatches.length > 0"
            class="px-2 py-0.5 rounded-full bg-amber-100 text-amber-800 font-semibold normal-case"
          >
            {{ ingestStore.pendingNamingBatches.length }} card{{ ingestStore.pendingNamingBatches.length === 1 ? '' : 's' }} ready to name
          </span>
          <span
            v-if="ingestStore.activeCopies.length > 0"
            class="px-2 py-0.5 rounded-full bg-blue-100 text-blue-800 font-semibold normal-case animate-pulse"
          >
            {{ ingestStore.activeCopies.length }} copying now
          </span>
        </div>
        <span class="text-xs text-slate-400">
          Click any finished card to jump straight to naming its folders
        </span>
      </div>

      <div class="flex flex-wrap gap-2">
        <!-- Actively copying cards / folders -->
        <div
          v-for="copy in ingestStore.activeCopies"
          :key="copy.readerId"
          class="flex items-center gap-2.5 px-3 py-2 rounded-lg border border-blue-200 bg-blue-50/70 text-xs"
        >
          <span class="w-2 h-2 rounded-full bg-blue-500 animate-pulse shrink-0" />
          <div class="min-w-0">
            <div class="font-semibold text-blue-900 truncate max-w-[200px]">{{ copy.readerLabel }}</div>
            <div class="font-mono text-[11px] text-blue-700 truncate max-w-[240px]">
              {{ copy.destinationRelPath || "(event root)" }}
            </div>
          </div>
          <span class="font-mono font-semibold text-blue-700 pl-1">
            {{ Math.round(copy.progress?.percentage || 0) }}%
          </span>
        </div>

        <!-- Finished cards with unnamed folders -->
        <button
          v-for="batch in ingestStore.pendingNamingBatches"
          :key="batch.id"
          @click="eventStore.openBatchForNaming(batch.id)"
          class="group flex items-center gap-2.5 px-3 py-2 rounded-lg border border-amber-200 bg-amber-50/70 hover:bg-indigo-50 hover:border-indigo-300 transition-colors text-left text-xs"
        >
          <span class="px-1.5 py-0.5 rounded bg-white border border-amber-200 font-mono font-bold text-slate-700">
            #{{ batch.sequenceNum }}
          </span>
          <div class="min-w-0">
            <div class="font-semibold text-slate-900 truncate max-w-[220px]">
              {{ batch.readerName || batch.volumeName || "Card" }}
            </div>
            <div class="font-mono text-[11px] text-slate-600 truncate max-w-[260px]">
              {{ batch.destinationRelPath || "(event root)" }}
            </div>
          </div>
          <div class="pl-1 text-right shrink-0">
            <div class="font-semibold text-amber-800 group-hover:text-indigo-700">
              {{ batch.unassignedFolders }} to name &rarr;
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

    <!-- ================= NAMED GROUPS (e.g. Gym A, Gym B) ================= -->
    <div v-if="namedGroups.length > 0" class="space-y-5">
      <section
        v-for="(group, gIdx) in namedGroups"
        :key="group.name"
        :data-drop-group="group.name"
        class="rounded-2xl border-2 p-4 space-y-3.5 transition-all"
        :class="
          draggingReader && hoverGroup === group.name
            ? 'border-indigo-500 bg-indigo-50/50 ring-2 ring-indigo-200'
            : 'border-slate-200/90 bg-slate-50/60'
        "
      >
        <!-- Group Header -->
        <div class="flex flex-wrap items-center justify-between gap-2">
          <div class="flex items-center gap-2.5">
            <template v-if="renamingGroup === group.name">
              <input
                v-model="renameGroupDraft"
                @keydown.enter.prevent="commitRenameGroup(group.name)"
                @keydown.escape="renamingGroup = null"
                @blur="commitRenameGroup(group.name)"
                type="text"
                autofocus
                class="border border-blue-400 rounded-md px-2.5 py-0.5 text-base font-bold text-slate-900 bg-white"
              />
            </template>
            <template v-else>
              <h2 class="text-base font-bold text-slate-800">{{ group.name }}</h2>
              <button
                @click="startRenameGroup(group.name)"
                title="Rename group"
                class="text-xs text-slate-400 hover:text-slate-700 px-1.5 py-0.5 rounded hover:bg-slate-200/60"
              >
                Rename
              </button>
            </template>

            <span class="px-2 py-0.5 text-xs font-medium rounded-full bg-white border border-slate-200 text-slate-600">
              {{ group.onlineCount }}/{{ group.readers.length }} card{{ group.readers.length === 1 ? '' : 's' }} ready
            </span>
          </div>

          <div class="flex items-center gap-1">
            <button
              v-if="namedGroups.length > 1"
              @click="ingestStore.moveGroup(group.name, -1)"
              :disabled="gIdx === 0"
              title="Move group up"
              class="px-2 py-1 text-xs rounded border border-slate-200 bg-white text-slate-600 hover:bg-slate-100 disabled:opacity-30"
            >
              &uarr;
            </button>
            <button
              v-if="namedGroups.length > 1"
              @click="ingestStore.moveGroup(group.name, 1)"
              :disabled="gIdx === namedGroups.length - 1"
              title="Move group down"
              class="px-2 py-1 text-xs rounded border border-slate-200 bg-white text-slate-600 hover:bg-slate-100 disabled:opacity-30"
            >
              &darr;
            </button>
            <button
              @click="confirmRemoveGroup(group.name)"
              title="Remove group"
              class="px-2 py-1 text-xs rounded text-slate-400 hover:text-red-600 hover:bg-red-50"
            >
              &times;
            </button>
          </div>
        </div>

        <!-- Reader Cards Grid (generous >= 340px card width!) -->
        <div
          v-if="group.readers.length > 0"
          class="grid gap-5"
          style="grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));"
        >
          <ReaderDashboardCard
            v-for="(reader, rIdx) in group.readers"
            :key="reader.reader_id"
            :reader="reader"
            :can-move-left="Boolean(ingestStore.readerConfigs[reader.reader_id]) && rIdx > 0"
            :can-move-right="
              Boolean(ingestStore.readerConfigs[reader.reader_id]) &&
              rIdx + 1 < group.readers.filter((x) => Boolean(ingestStore.readerConfigs[x.reader_id])).length
            "
            :is-dragging="draggingReader?.reader_id === reader.reader_id"
            :drop-side="hoverTargetReaderId === reader.reader_id ? hoverDropSide : null"
            @configure="openConfigure($event, group.name)"
            @drag-start="startDrag"
            @move-left="nudgeReaderInList(group.readers, group.name, $event.reader_id, -1)"
            @move-right="nudgeReaderInList(group.readers, group.name, $event.reader_id, 1)"
          />
        </div>

        <div
          v-else
          class="rounded-xl border border-dashed border-slate-300 bg-white/70 p-6 text-center text-xs text-slate-500"
        >
          Drag a card reader here (using its <strong>⋮⋮</strong> grip) or assign <strong>{{ group.name }}</strong> in Edit Config.
        </div>
      </section>
    </div>

    <!-- ================= UNGROUPED / NEW READERS ================= -->
    <section
      v-if="ungroupedReaders.length > 0 || (draggingReader && namedGroups.length > 0)"
      data-drop-group=""
      class="rounded-2xl transition-all"
      :class="
        namedGroups.length > 0
          ? draggingReader && hoverGroup === ''
            ? 'border-2 border-indigo-500 bg-indigo-50/50 ring-2 ring-indigo-200 p-4 space-y-3.5'
            : 'border-2 border-dashed border-slate-200 p-4 space-y-3.5'
          : 'space-y-3.5'
      "
    >
      <div v-if="namedGroups.length > 0" class="flex items-center justify-between">
        <h2 class="text-sm font-semibold text-slate-600">
          Ungrouped / New Readers ({{ ungroupedReaders.length }})
        </h2>
        <span class="text-xs text-slate-400">Drag into any group above or configure directly</span>
      </div>

      <div
        v-if="ungroupedReaders.length > 0"
        class="grid gap-5"
        style="grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));"
      >
        <ReaderDashboardCard
          v-for="(reader, rIdx) in ungroupedReaders"
          :key="reader.reader_id"
          :reader="reader"
          :can-move-left="Boolean(ingestStore.readerConfigs[reader.reader_id]) && rIdx > 0"
          :can-move-right="
            Boolean(ingestStore.readerConfigs[reader.reader_id]) &&
            rIdx + 1 < ungroupedReaders.filter((x) => Boolean(ingestStore.readerConfigs[x.reader_id])).length
          "
          :is-dragging="draggingReader?.reader_id === reader.reader_id"
          :drop-side="hoverTargetReaderId === reader.reader_id ? hoverDropSide : null"
          @configure="openConfigure($event, null)"
          @drag-start="startDrag"
          @move-left="nudgeReaderInList(ungroupedReaders, null, $event.reader_id, -1)"
          @move-right="nudgeReaderInList(ungroupedReaders, null, $event.reader_id, 1)"
        />
      </div>
      <div v-else class="py-4 text-center text-xs text-slate-400">
        Drop here to remove reader from its group
      </div>
    </section>

    <!-- Empty State -->
    <div
      v-if="allReaders.length === 0 && namedGroups.length === 0"
      class="bg-white rounded-xl border border-slate-200 p-14 text-center"
    >
      <h3 class="text-lg font-medium text-slate-900">No Card Readers Detected</h3>
      <p class="text-sm text-slate-500 mt-1">
        Connect a USB card reader or insert an SD card to begin ingesting, or click <strong>+ New Group</strong> to pre-create Gym groups.
      </p>
      <button
        @click="ingestStore.discoverReaders()"
        class="mt-4 px-4 py-2 text-sm font-medium bg-blue-50 text-blue-600 rounded-lg hover:bg-blue-100"
      >
        Refresh Now
      </button>
    </div>

    <!-- Floating Drag Preview Pill -->
    <div
      v-if="draggingReader"
      class="fixed z-50 pointer-events-none px-3.5 py-2 rounded-xl bg-indigo-600 text-white text-sm font-semibold shadow-2xl flex items-center gap-2 -translate-x-1/2 -translate-y-1/2"
      :style="{ left: `${dragX}px`, top: `${dragY}px` }"
    >
      <span>⋮⋮</span>
      <span>{{ draggingLabel }}</span>
      <span v-if="hoverGroup" class="text-xs px-1.5 py-0.5 rounded bg-white/20">
        &rarr; {{ hoverGroup }}
      </span>
    </div>

    <!-- Local Folder Ingest Modal -->
    <div
      v-if="showFolderIngestModal"
      class="fixed inset-0 bg-black/50 flex items-center justify-center p-4 z-50 backdrop-blur-sm"
      @click="showFolderIngestModal = false"
    >
      <div class="w-full max-w-lg bg-white rounded-xl shadow-xl border border-slate-200 overflow-hidden" @click.stop>
        <div class="px-5 py-4 border-b border-slate-200 flex items-center justify-between">
          <div>
            <h2 class="text-base font-bold text-slate-900">Ingest Local Folder</h2>
            <p class="text-xs text-slate-500 mt-0.5">
              Copy photos from a folder or thumb drive through the card ingest queue.
            </p>
          </div>
          <button @click="showFolderIngestModal = false" class="text-slate-400 hover:text-slate-700 text-xl leading-none">&times;</button>
        </div>

        <form @submit.prevent="startLocalFolderIngest" class="p-5 space-y-4">
          <div v-if="localIngestError" class="p-2.5 rounded-lg bg-red-50 border border-red-200 text-xs text-red-700">
            {{ localIngestError }}
          </div>

          <div>
            <label class="block text-xs font-medium text-slate-700 mb-1">Source Folder *</label>
            <div class="flex gap-2">
              <input
                v-model="localSourcePath"
                type="text"
                readonly
                placeholder="Choose folder (e.g. DCIM or card dump)..."
                class="flex-1 border border-slate-300 rounded-lg px-3 py-2 text-sm bg-slate-50 font-mono"
              />
              <button
                type="button"
                @click="pickLocalSourceFolder"
                class="px-3 py-2 bg-slate-200 hover:bg-slate-300 rounded-lg text-sm font-medium"
              >
                Browse
              </button>
            </div>
          </div>

          <div>
            <label class="block text-xs font-medium text-slate-700 mb-1">
              Destination Path (relative to event)
            </label>
            <input
              v-model="localDestRelPath"
              type="text"
              placeholder="e.g. Gym A/Session 10A/Group 2A/Beam"
              class="w-full border border-slate-300 rounded-lg px-3 py-2 text-sm font-mono"
            />
          </div>

          <div class="grid grid-cols-2 gap-3">
            <div>
              <label class="block text-xs font-medium text-slate-700 mb-1">Card / Station Label</label>
              <input
                v-model="localLabel"
                type="text"
                placeholder="e.g. Gym A · Beam"
                class="w-full border border-slate-300 rounded-lg px-3 py-2 text-sm"
              />
            </div>
            <div>
              <label class="block text-xs font-medium text-slate-700 mb-1">Photographer</label>
              <input
                v-model="localPhotographer"
                type="text"
                placeholder="Initials or name"
                class="w-full border border-slate-300 rounded-lg px-3 py-2 text-sm"
              />
            </div>
          </div>

          <div class="flex items-center justify-between gap-3 pt-1">
            <label class="flex items-center gap-2 text-sm text-slate-700 cursor-pointer">
              <input v-model="localAutoRename" type="checkbox" class="rounded" />
              <span>Auto-rename numbered subfolders</span>
            </label>
            <input
              v-if="localAutoRename"
              v-model="localRenamePrefix"
              type="text"
              placeholder="Gymnast"
              class="w-32 border border-slate-300 rounded-md px-2.5 py-1 text-sm"
            />
          </div>

          <div class="flex justify-end gap-2 pt-2 border-t border-slate-100">
            <button
              type="button"
              @click="showFolderIngestModal = false"
              class="px-4 py-2 text-sm text-slate-600 hover:text-slate-900"
            >
              Cancel
            </button>
            <button
              type="submit"
              class="px-4 py-2 text-sm font-semibold rounded-lg bg-blue-600 text-white hover:bg-blue-700"
            >
              Start Ingest
            </button>
          </div>
        </form>
      </div>
    </div>

    <!-- Reader Setup Modal -->
    <div
      v-if="configuringReader"
      class="fixed inset-0 bg-black/50 flex items-center justify-center p-4 z-50 backdrop-blur-sm"
      @click="configuringReader = null"
    >
      <div class="w-full max-w-lg" @click.stop>
        <ReaderSetupModal
          :reader="configuringReader"
          :default-group="configuringDefaultGroup"
          @close="configuringReader = null"
          @saved="configuringReader = null"
        />
      </div>
    </div>
  </div>
</template>
