<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEventStore } from "@/stores/event";
import { useIngestStore } from "@/stores/ingest";
import CardNamingQueueStrip from "@/components/CardNamingQueueStrip.vue";
import StatusBanners from "@/components/StatusBanners.vue";
import RosterPanel from "@/components/roster/RosterPanel.vue";
import FolderModals, { type StagedPreviewRow } from "@/components/roster/FolderModals.vue";
import { rankAndFilterSubjects } from "@/utils/subjects";
import type {
  AvatarMetrics,
  BatchReassignResult,
  FolderItem,
  IngestBatchSummary,
  ListFoldersResponse,
  RemoveFolderMode,
  RemoveFolderResult,
  SubjectRecord,
} from "@/types";

const eventStore = useEventStore();
const ingestStore = useIngestStore();

type SubTab = "roster" | "folders";
type FolderMode = "unassigned" | "all";
type PickerScope = "group" | "session" | "all";

const subTab = ref<SubTab>("roster");
const groups = ref<string[]>([]);
const allSubjects = ref<SubjectRecord[]>([]);
const bannerMessage = ref<string | null>(null);
const errorMessage = ref<string | null>(null);

// Folder manager state
const folderMode = ref<FolderMode>("unassigned");
const activeBatchId = ref<string | null>(null);
const cardShowAllFolders = ref(false);
const nextCardToName = ref<IngestBatchSummary | null>(null);
const hideMatchedSubjects = ref(localStorage.getItem("photofinish:hideMatchedSubjects") === "true");
const locationFilterQuery = ref("");
const selectedParent = ref("");
const folderData = ref<ListFoldersResponse>({
  locations: [],
  folders: [],
  totalUnassignedPhotos: 0,
  totalUnassignedFolders: 0,
  detectedSessionGroup: null,
  detectedGroupPrefix: null,
  sameGroupSubjectIds: [],
  subjectSamplePhotos: {},
  sectionLabel: null,
  sectionMatchedSubjects: {},
});

watch(hideMatchedSubjects, (val) => {
  localStorage.setItem("photofinish:hideMatchedSubjects", String(val));
});

const selectedFolderPath = ref<string | null>(null);
const pickerScope = ref<PickerScope>("all");
const userPinnedScope = ref(false);
const pickerSearch = ref("");
const renameOnDisk = ref(true);

const stagedMap = ref<Record<string, string | null>>({});
const showPreviewModal = ref(false);
const isApplyingBatch = ref(false);

const removeTarget = ref<FolderItem | null>(null);
const removeMode = ref<RemoveFolderMode>("trash");
const isRemoving = ref(false);
const previewLightbox = ref<{ folder: FolderItem; index: number } | null>(null);

const subjectLabel = computed(() => eventStore.activeEvent?.subjectLabel || "Subject");
const lanPort = computed(() => eventStore.lanStatus.port || 4000);
const thumbBaseUrl = computed(() => `http://127.0.0.1:${lanPort.value}/photos/thumbnail`);
const previewBaseUrl = computed(() => `http://127.0.0.1:${lanPort.value}/photos/preview`);
const avatarBaseUrl = computed(() => `http://127.0.0.1:${lanPort.value}/photos/avatar`);

const avatarMetrics = ref<AvatarMetrics | null>(null);
const avatarVersion = ref(1);
const isGeneratingAvatars = computed(
  () => (avatarMetrics.value?.queued ?? 0) + (avatarMetrics.value?.inFlight ?? 0) > 0
);

const isStaged = (path: string) => Object.prototype.hasOwnProperty.call(stagedMap.value, path);

const subjectById = computed(() => new Map(allSubjects.value.map((s) => [s.id, s])));

const selectedFolder = computed<FolderItem | null>(
  () => folderData.value.folders.find((f) => f.folderPath === selectedFolderPath.value) || null
);

const availableLocations = computed(() => {
  const base =
    folderMode.value === "unassigned"
      ? folderData.value.locations.filter((l) => l.unassignedFolderCount > 0)
      : folderData.value.locations;
  const tokens = locationFilterQuery.value.trim().toLowerCase().split(/\s+/).filter(Boolean);
  if (tokens.length === 0) return base;
  return base.filter((loc) => tokens.every((t) => loc.parentPath.toLowerCase().includes(t)));
});

const shortGroupName = computed(() => {
  const prefix = folderData.value.detectedGroupPrefix;
  return prefix ? prefix.split("/").pop() || prefix : null;
});

const sameGroupIdSet = computed(() => new Set(folderData.value.sameGroupSubjectIds));
const sameGroupSubjects = computed(() =>
  sameGroupIdSet.value.size === 0
    ? []
    : allSubjects.value.filter((s) => sameGroupIdSet.value.has(s.id))
);
const sameSessionSubjects = computed(() => {
  const sess = folderData.value.detectedSessionGroup;
  return sess ? allSubjects.value.filter((s) => s.groupLabel === sess) : [];
});

const scopedPickerSubjects = computed(() => {
  const pool =
    pickerScope.value === "group" && sameGroupSubjects.value.length > 0
      ? sameGroupSubjects.value
      : pickerScope.value === "session" && sameSessionSubjects.value.length > 0
        ? sameSessionSubjects.value
        : allSubjects.value;
  return rankAndFilterSubjects(pool, pickerSearch.value).slice(0, 300);
});

const crossScopeMatchCounts = computed(() => {
  const q = pickerSearch.value.trim();
  if (!q) return { session: 0, all: 0 };
  return {
    session:
      pickerScope.value === "group" && sameSessionSubjects.value.length > 0
        ? rankAndFilterSubjects(sameSessionSubjects.value, q).length
        : 0,
    all: pickerScope.value !== "all" ? rankAndFilterSubjects(allSubjects.value, q).length : 0,
  };
});

const subjectUsageInView = computed(() => {
  const stagedTo = new Map<string, string>();
  const currentInLoc = new Map<string, string>();
  for (const f of folderData.value.folders) {
    if (f.assignedSubjectId) currentInLoc.set(f.assignedSubjectId, f.leafFolder);
    if (isStaged(f.folderPath)) {
      const targetId = stagedMap.value[f.folderPath];
      if (targetId) stagedTo.set(targetId, f.leafFolder);
    }
  }
  return { stagedTo, currentInLoc };
});

const activeBatch = computed<IngestBatchSummary | null>(() =>
  activeBatchId.value
    ? ingestStore.ingestBatches.find((b) => b.id === activeBatchId.value) || null
    : null
);

const isFixingExistingRotation = computed(
  () =>
    (!activeBatchId.value && folderMode.value === "all") ||
    (Boolean(activeBatchId.value) && cardShowAllFolders.value)
);

const classifiedPickerSubjects = computed(() => {
  const sel = selectedFolder.value;
  const selStaged = sel ? stagedMap.value[sel.folderPath] : undefined;
  const secLabel = folderData.value.sectionLabel;
  const secMatched = folderData.value.sectionMatchedSubjects || {};

  return scopedPickerSubjects.value.map((s) => {
    const isStagedHere = Boolean(sel && selStaged === s.id);
    if (isStagedHere) {
      return { subject: s, dimmed: false, badgeText: "Staged here", badgeClass: "bg-indigo-600 text-white font-semibold" };
    }
    if (subjectUsageInView.value.stagedTo.has(s.id)) {
      return { subject: s, dimmed: true, badgeText: "Staged", badgeClass: "bg-indigo-100 text-indigo-700 font-medium" };
    }
    if (sel && sel.assignedSubjectId === s.id) {
      return { subject: s, dimmed: false, badgeText: "Current", badgeClass: "bg-emerald-100 text-emerald-700 font-medium" };
    }
    if (subjectUsageInView.value.currentInLoc.has(s.id)) {
      return {
        subject: s,
        dimmed: !isFixingExistingRotation.value,
        badgeText: "Matched here",
        badgeClass: "bg-slate-200/80 text-slate-600 font-medium",
      };
    }
    const matchedElsewhereLoc = secMatched[s.id];
    if (matchedElsewhereLoc) {
      return {
        subject: s,
        dimmed: true,
        badgeText: secLabel ? `Done on ${secLabel} · ${matchedElsewhereLoc}` : `Matched · ${matchedElsewhereLoc}`,
        badgeClass: "bg-slate-200/70 text-slate-600 font-medium",
      };
    }
    return { subject: s, dimmed: false, badgeText: null, badgeClass: "" };
  });
});

const visiblePickerItems = computed(() =>
  hideMatchedSubjects.value
    ? classifiedPickerSubjects.value.filter((i) => !i.dimmed)
    : classifiedPickerSubjects.value
);
const remainingCount = computed(() => classifiedPickerSubjects.value.filter((i) => !i.dimmed).length);
const dimmedCount = computed(() => classifiedPickerSubjects.value.length - remainingCount.value);

const sanitizeFolderName = (name: string) => name.replace(/[/\\:*?"<>|]/g, "_").trim();
const stagedCount = computed(() => Object.keys(stagedMap.value).length);

const stagedPreviewRows = computed<StagedPreviewRow[]>(() => {
  const entries = Object.entries(stagedMap.value);
  if (entries.length === 0) return [];

  const folderLookup = new Map(folderData.value.folders.map((f) => [f.folderPath, f]));
  const existingPathsInView = new Set(folderLookup.keys());

  const raw = entries
    .map(([folderPath, targetSubjectId]) => {
      const folder = folderLookup.get(folderPath);
      if (!folder) return null;
      const targetSubject = targetSubjectId ? subjectById.value.get(targetSubjectId) || null : null;
      const targetLeaf =
        renameOnDisk.value && targetSubject
          ? sanitizeFolderName(targetSubject.displayName)
          : folder.leafFolder;
      const targetFolderPath =
        renameOnDisk.value && targetSubject
          ? folder.parentPath
            ? `${folder.parentPath}/${targetLeaf}`
            : targetLeaf
          : folder.folderPath;
      const willRename = renameOnDisk.value && targetSubject !== null && targetFolderPath !== folder.folderPath;
      return { folder, targetSubjectId, targetSubject, targetLeaf, targetFolderPath, willRename };
    })
    .filter((r): r is NonNullable<typeof r> => r !== null);

  const movingAway = new Set(raw.filter((r) => r.willRename).map((r) => r.folder.folderPath));
  const targetCounts = new Map<string, number>();
  for (const r of raw) {
    if (r.willRename) targetCounts.set(r.targetFolderPath, (targetCounts.get(r.targetFolderPath) || 0) + 1);
  }

  return raw.map((r) => {
    const action: StagedPreviewRow["action"] = !r.targetSubjectId
      ? "unassigned"
      : !r.willRename
        ? "reassigned"
        : (targetCounts.get(r.targetFolderPath) || 0) > 1
          ? "merged"
          : movingAway.has(r.targetFolderPath)
            ? "swapped"
            : existingPathsInView.has(r.targetFolderPath)
              ? "merged"
              : "renamed";
    return {
      folder: r.folder,
      targetSubjectId: r.targetSubjectId,
      targetSubject: r.targetSubject,
      targetLeaf: r.targetLeaf,
      targetFolderPath: r.targetFolderPath,
      action,
    };
  });
});

const stagedPhotoTotal = computed(() =>
  stagedPreviewRows.value.reduce((sum, r) => sum + r.folder.photoCount, 0)
);
const hasMergeWarning = computed(() => stagedPreviewRows.value.some((r) => r.action === "merged"));

async function loadAvatarMetrics() {
  if (!eventStore.activeEvent) return;
  try {
    avatarMetrics.value = await invoke<AvatarMetrics>("get_avatar_metrics");
  } catch {
    avatarMetrics.value = null;
  }
}

async function triggerGenerateAvatars(force = false) {
  errorMessage.value = null;
  try {
    const queued = await invoke<number>("generate_event_avatars", { force });
    await loadAvatarMetrics();
    bannerMessage.value =
      queued > 0
        ? `Queued ${queued.toLocaleString()} athlete portrait avatar(s) using ${avatarMetrics.value?.engine || "Apple Vision"} — portraits will appear automatically as they finish.`
        : "All matched athletes with ready photos already have cached portrait avatars.";
  } catch (err) {
    errorMessage.value = String(err);
  }
}

async function loadRosterData() {
  if (!eventStore.activeEvent) return;
  try {
    const [g, s] = await Promise.all([
      invoke<string[]>("list_subject_groups"),
      invoke<SubjectRecord[]>("list_subjects", { query: null, groupLabel: null, limit: 10000 }),
    ]);
    groups.value = g;
    allSubjects.value = s;
  } catch (err) {
    errorMessage.value = String(err);
  }
}

async function loadFolders(contextParentOverride?: string | null) {
  if (!eventStore.activeEvent) return;
  try {
    let res = await invoke<ListFoldersResponse>("list_folders", {
      parentFilter: activeBatchId.value ? null : selectedParent.value || null,
      onlyUnassigned: activeBatchId.value ? !cardShowAllFolders.value : folderMode.value === "unassigned",
      contextParent: contextParentOverride ?? selectedFolder.value?.parentPath ?? null,
      batchId: activeBatchId.value,
    });

    if (!activeBatchId.value && folderMode.value === "all" && !selectedParent.value && res.locations.length > 0) {
      selectedParent.value = res.locations[0].parentPath;
      res = await invoke<ListFoldersResponse>("list_folders", {
        parentFilter: selectedParent.value,
        onlyUnassigned: false,
        contextParent: selectedParent.value,
        batchId: null,
      });
    }
    folderData.value = res;

    if (!selectedFolderPath.value || !res.folders.some((f) => f.folderPath === selectedFolderPath.value)) {
      const firstUnassigned = res.folders.find((f) => f.unassignedCount > 0);
      selectedFolderPath.value = firstUnassigned?.folderPath || res.folders[0]?.folderPath || null;
    }

    if (!userPinnedScope.value) {
      pickerScope.value =
        res.sameGroupSubjectIds.length > 0 ? "group" : res.detectedSessionGroup ? "session" : "all";
    } else if (pickerScope.value === "group" && res.sameGroupSubjectIds.length === 0) {
      pickerScope.value = res.detectedSessionGroup ? "session" : "all";
    } else if (pickerScope.value === "session" && !res.detectedSessionGroup) {
      pickerScope.value = "all";
    }
  } catch (err) {
    errorMessage.value = String(err);
  }
}

async function refreshAll() {
  await Promise.all([
    loadFolders(),
    loadRosterData(),
    ingestStore.loadIngestBatches(),
    eventStore.refreshActiveEvent(),
  ]);
}

function resetFolderSelection() {
  stagedMap.value = {};
  selectedFolderPath.value = null;
  userPinnedScope.value = false;
}

function selectIngestBatch(batchId: string, showAll = false) {
  subTab.value = "folders";
  activeBatchId.value = batchId;
  cardShowAllFolders.value = showAll;
  nextCardToName.value = null;
  pickerSearch.value = "";
  resetFolderSelection();
  loadFolders();
}

function toggleCardShowAll(showAll: boolean) {
  if (cardShowAllFolders.value === showAll) return;
  cardShowAllFolders.value = showAll;
  stagedMap.value = {};
  selectedFolderPath.value = null;
  loadFolders();
}

function exitCardView() {
  activeBatchId.value = null;
  cardShowAllFolders.value = false;
  nextCardToName.value = null;
  resetFolderSelection();
  loadFolders();
}

function switchFolderMode(mode: FolderMode) {
  if (!activeBatchId.value && folderMode.value === mode) return;
  activeBatchId.value = null;
  nextCardToName.value = null;
  folderMode.value = mode;
  locationFilterQuery.value = "";
  if (mode === "unassigned") selectedParent.value = "";
  resetFolderSelection();
  loadFolders();
}

function selectParentPath(parent: string) {
  selectedParent.value = parent;
  resetFolderSelection();
  loadFolders(parent || null);
}

function selectFolder(folder: FolderItem) {
  const prevParent = selectedFolder.value?.parentPath;
  selectedFolderPath.value = folder.folderPath;
  if (!selectedParent.value && folder.parentPath && folder.parentPath !== prevParent) {
    userPinnedScope.value = false;
    loadFolders(folder.parentPath);
  }
}

function setScope(scope: PickerScope) {
  pickerScope.value = scope;
  userPinnedScope.value = true;
}

function stageFolderTarget(targetSubjectId: string | null) {
  const folder = selectedFolder.value;
  if (!folder) return;

  const next = { ...stagedMap.value };
  const matchesCurrent =
    !folder.isMixed &&
    ((targetSubjectId !== null && folder.assignedSubjectId === targetSubjectId) ||
      (targetSubjectId === null && folder.assignedSubjectId === null));
  if (matchesCurrent) delete next[folder.folderPath];
  else next[folder.folderPath] = targetSubjectId;
  stagedMap.value = next;

  const idx = folderData.value.folders.findIndex((f) => f.folderPath === folder.folderPath);
  if (idx >= 0 && idx + 1 < folderData.value.folders.length) {
    selectedFolderPath.value = folderData.value.folders[idx + 1].folderPath;
  }
}

function unstageFolder(folderPath: string) {
  const next = { ...stagedMap.value };
  delete next[folderPath];
  stagedMap.value = next;
  if (Object.keys(next).length === 0) showPreviewModal.value = false;
}

async function confirmApplyStaged() {
  if (stagedPreviewRows.value.length === 0 || isApplyingBatch.value) return;
  isApplyingBatch.value = true;
  errorMessage.value = null;
  bannerMessage.value = null;
  try {
    const res = await invoke<BatchReassignResult>("apply_folder_reassignments", {
      changes: stagedPreviewRows.value.map((r) => ({
        folderPath: r.folder.folderPath,
        targetSubjectId: r.targetSubjectId,
      })),
      renameOnDisk: renameOnDisk.value,
    });
    stagedMap.value = {};
    showPreviewModal.value = false;

    const swapCount = res.changes.filter((c) => c.action === "swapped").length;
    const suffix = swapCount > 0 ? ` (${swapCount} swapped atomically)` : res.renamedOnDisk ? " (folders renamed on disk)" : "";
    const finishedBatchId = activeBatchId.value;
    await refreshAll();

    const updatedBatch = finishedBatchId
      ? ingestStore.ingestBatches.find((b) => b.id === finishedBatchId)
      : null;
    if (updatedBatch && updatedBatch.unassignedFolders === 0) {
      nextCardToName.value = ingestStore.pendingNamingBatches.find((b) => b.id !== finishedBatchId) || null;
      bannerMessage.value = `Card #${updatedBatch.sequenceNum} complete! Updated ${res.foldersUpdated} folder(s) / ${res.photosUpdated.toLocaleString()} photo(s)${suffix}.`;
    } else {
      nextCardToName.value = null;
      bannerMessage.value = `Updated ${res.foldersUpdated} folder(s) / ${res.photosUpdated.toLocaleString()} photo(s)${suffix}.`;
    }
  } catch (err) {
    errorMessage.value = String(err);
  } finally {
    isApplyingBatch.value = false;
  }
}

async function confirmRemoveFolder() {
  if (!removeTarget.value || isRemoving.value) return;
  isRemoving.value = true;
  errorMessage.value = null;
  bannerMessage.value = null;
  const target = removeTarget.value;
  try {
    const res = await invoke<RemoveFolderResult>("remove_folder", {
      folderPath: target.folderPath,
      mode: removeMode.value,
    });
    if (isStaged(target.folderPath)) unstageFolder(target.folderPath);
    if (selectedFolderPath.value === target.folderPath) selectedFolderPath.value = null;
    removeTarget.value = null;
    bannerMessage.value =
      res.mode === "trash"
        ? `Moved "${target.leafFolder}" (${res.photosRemoved} photos) to ${res.trashedTo || "_photofinish/trash"} and removed from event.`
        : res.mode === "delete_disk"
          ? `Permanently deleted "${target.leafFolder}" (${res.photosRemoved} photos) from disk and event.`
          : `Removed "${target.leafFolder}" (${res.photosRemoved} photos) from database (files remain on disk).`;
    await refreshAll();
  } catch (err) {
    errorMessage.value = String(err);
  } finally {
    isRemoving.value = false;
  }
}

async function runAutoMatch() {
  errorMessage.value = null;
  try {
    const count = await invoke<number>("auto_match_photos");
    bannerMessage.value =
      count > 0
        ? `Auto-matched ${count.toLocaleString()} photo(s) to roster ${subjectLabel.value.toLowerCase()}s.`
        : "No new number-prefixed folders matched the current roster.";
    await refreshAll();
  } catch (err) {
    errorMessage.value = String(err);
  }
}

function onLightboxKeyDown(e: KeyboardEvent) {
  if (!previewLightbox.value) return;
  if (e.key === "Escape") previewLightbox.value = null;
  else if (e.key === "ArrowLeft" && previewLightbox.value.index > 0) {
    previewLightbox.value = { ...previewLightbox.value, index: previewLightbox.value.index - 1 };
  } else if (e.key === "ArrowRight" && previewLightbox.value.index + 1 < previewLightbox.value.folder.samplePhotoIds.length) {
    previewLightbox.value = { ...previewLightbox.value, index: previewLightbox.value.index + 1 };
  }
}

let liveRefreshTimer: ReturnType<typeof setInterval> | null = null;
let unlistenAvatars: UnlistenFn | null = null;
let avatarBumpTimer: ReturnType<typeof setTimeout> | null = null;

async function consumePendingBatch() {
  const bid = eventStore.pendingBatchToName;
  if (!bid || !eventStore.activeEvent) return;
  eventStore.clearPendingBatchToName();
  await ingestStore.loadIngestBatches();
  selectIngestBatch(bid);
}

onMounted(async () => {
  window.addEventListener("keydown", onLightboxKeyDown);
  unlistenAvatars = await listen("avatars-updated", () => {
    if (avatarBumpTimer) clearTimeout(avatarBumpTimer);
    avatarBumpTimer = setTimeout(() => {
      avatarVersion.value += 1;
      loadAvatarMetrics();
    }, 450);
  });

  if (eventStore.activeEvent) {
    await Promise.all([loadRosterData(), ingestStore.loadIngestBatches(), loadAvatarMetrics()]);
    if (eventStore.pendingBatchToName) await consumePendingBatch();
    else await loadFolders();
  }

  liveRefreshTimer = setInterval(async () => {
    if (!eventStore.activeEvent) return;
    if (isGeneratingAvatars.value) await loadAvatarMetrics();
    if (subTab.value !== "folders") return;
    if (eventStore.activeEvent.photoCounts.pending === 0 && ingestStore.activeCopies.length === 0) return;

    await ingestStore.loadIngestBatches();
    if (folderData.value.folders.some((f) => f.samplePhotoIds.length < Math.min(4, f.photoCount))) {
      await loadFolders();
    }
  }, 1500);
});

onUnmounted(() => {
  window.removeEventListener("keydown", onLightboxKeyDown);
  if (liveRefreshTimer) clearInterval(liveRefreshTimer);
  if (unlistenAvatars) unlistenAvatars();
  if (avatarBumpTimer) clearTimeout(avatarBumpTimer);
});

watch(() => eventStore.pendingBatchToName, (bid) => {
  if (bid && eventStore.activeEvent) consumePendingBatch();
});

watch(() => eventStore.activeEvent?.id, () => {
  selectedParent.value = "";
  activeBatchId.value = null;
  nextCardToName.value = null;
  resetFolderSelection();
  if (eventStore.activeEvent) {
    loadRosterData();
    ingestStore.loadIngestBatches();
    loadAvatarMetrics();
    loadFolders();
  }
});

watch(() => eventStore.activeEvent?.photoCounts.total, (next, prev) => {
  if (eventStore.activeEvent && prev !== undefined && next !== prev) {
    ingestStore.loadIngestBatches();
    loadFolders();
  }
});

watch(() => eventStore.fsSyncGeneration, (next, prev) => {
  if (eventStore.activeEvent && prev !== undefined && next !== prev) {
    refreshAll();
  }
});
</script>

<template>
  <div class="max-w-7xl mx-auto p-6 space-y-5">
    <div v-if="!eventStore.activeEvent" class="p-4 rounded-xl bg-amber-50 border border-amber-200 text-sm text-amber-800">
      Open or create an Event Bundle first to import rosters and manage folders.
    </div>

    <template v-else>
      <!-- Header & Sub-tabs -->
      <div class="flex flex-wrap items-center justify-between gap-4">
        <div>
          <h1 class="text-2xl font-bold text-slate-900">Roster &amp; Folders</h1>
          <p class="text-sm text-slate-500 mt-0.5">
            Import {{ subjectLabel.toLowerCase() }} rosters, fix out-of-order rotations, rename folders, or remove junk folders.
          </p>
        </div>

        <div class="inline-flex rounded-lg bg-slate-200 p-1 text-sm font-medium">
          <button
            @click="subTab = 'roster'"
            class="px-3.5 py-1.5 rounded-md transition-colors"
            :class="subTab === 'roster' ? 'bg-white text-slate-900 shadow-sm' : 'text-slate-600 hover:text-slate-900'"
          >
            Roster ({{ eventStore.activeEvent.subjectCount.toLocaleString() }})
          </button>
          <button
            @click="subTab = 'folders'"
            class="px-3.5 py-1.5 rounded-md transition-colors flex items-center gap-1.5"
            :class="subTab === 'folders' ? 'bg-white text-slate-900 shadow-sm' : 'text-slate-600 hover:text-slate-900'"
          >
            Folders
            <span v-if="folderData.totalUnassignedPhotos > 0" class="px-1.5 py-0.5 text-xs rounded-full bg-amber-100 text-amber-800">
              {{ folderData.totalUnassignedPhotos.toLocaleString() }}
            </span>
          </button>
        </div>
      </div>

      <StatusBanners
        :banner-message="bannerMessage"
        :error-message="errorMessage"
        @clear-banner="bannerMessage = null; nextCardToName = null"
        @clear-error="errorMessage = null"
      >
        <template #bannerAction>
          <button
            v-if="nextCardToName"
            @click="selectIngestBatch(nextCardToName.id)"
            class="px-3 py-1.5 text-xs font-semibold rounded-md bg-emerald-600 text-white hover:bg-emerald-700 transition-colors"
          >
            Next Card (#{{ nextCardToName.sequenceNum }} · {{ nextCardToName.readerName || nextCardToName.destinationRelPath.split('/').slice(-2).join('/') }}) &rarr;
          </button>
        </template>
      </StatusBanners>

      <!-- ================= ROSTER TAB ================= -->
      <RosterPanel
        v-show="subTab === 'roster'"
        :subject-label="subjectLabel"
        :groups="groups"
        :all-subjects="allSubjects"
        :avatar-base-url="avatarBaseUrl"
        :avatar-version="avatarVersion"
        :avatar-metrics="avatarMetrics"
        :is-generating-avatars="isGeneratingAvatars"
        @refresh="refreshAll"
        @auto-match="runAutoMatch"
        @generate-avatars="triggerGenerateAvatars(false)"
        @banner="bannerMessage = $event; errorMessage = null"
        @error="errorMessage = $event"
      />

      <!-- ================= FOLDERS TAB ================= -->
      <div v-show="subTab === 'folders'" class="space-y-4">
        <CardNamingQueueStrip
          v-if="ingestStore.activeCopies.length > 0 || ingestStore.pendingNamingBatches.length > 0 || activeBatch"
          title="Ingested Card Queue"
          :active-batch-id="activeBatchId"
          :show-exit-all="Boolean(activeBatchId)"
          @select-batch="selectIngestBatch($event)"
          @exit-all="exitCardView"
        />

        <!-- Toolbar: Active Card Banner OR Mode Toggle + Location Selector -->
        <div class="bg-white p-4 rounded-xl border border-slate-200 space-y-3">
          <div v-if="activeBatch" class="flex flex-wrap items-center justify-between gap-3">
            <div class="flex items-center gap-2.5 min-w-0">
              <span class="px-2 py-0.5 rounded-md bg-indigo-600 text-white font-mono text-xs font-bold shrink-0">
                Card #{{ activeBatch.sequenceNum }}
              </span>
              <div class="min-w-0">
                <div class="text-sm font-bold text-slate-900 flex items-center gap-2 flex-wrap">
                  <span>{{ activeBatch.readerName || activeBatch.volumeName || "Ingested Card" }}</span>
                  <span v-if="activeBatch.photographer" class="text-xs font-normal text-slate-500">· {{ activeBatch.photographer }}</span>
                </div>
                <div class="text-xs font-mono text-indigo-700 truncate">
                  {{ activeBatch.destinationRelPath || "(event root)" }}
                </div>
              </div>
            </div>

            <div class="flex flex-wrap items-center gap-3">
              <div class="inline-flex rounded-lg bg-slate-100 p-0.5 text-xs font-medium border border-slate-200">
                <button
                  @click="toggleCardShowAll(false)"
                  class="px-2.5 py-1 rounded-md transition-colors flex items-center gap-1.5"
                  :class="!cardShowAllFolders ? 'bg-white text-slate-900 shadow-sm' : 'text-slate-600 hover:text-slate-900'"
                >
                  Unnamed Only
                  <span v-if="activeBatch.unassignedFolders > 0" class="px-1.5 py-0.2 rounded-full bg-amber-100 text-amber-800 text-[11px]">
                    {{ activeBatch.unassignedFolders }}
                  </span>
                </button>
                <button
                  @click="toggleCardShowAll(true)"
                  class="px-2.5 py-1 rounded-md transition-colors"
                  :class="cardShowAllFolders ? 'bg-white text-slate-900 shadow-sm' : 'text-slate-600 hover:text-slate-900'"
                >
                  All on Card ({{ activeBatch.totalFolders }})
                </button>
              </div>
              <label class="flex items-center gap-2 text-xs text-slate-700 cursor-pointer">
                <input v-model="renameOnDisk" type="checkbox" class="rounded" />
                <span>Rename folders on disk</span>
              </label>
              <button @click="exitCardView" class="px-3 py-1.5 text-xs font-medium rounded-lg border border-slate-300 bg-white text-slate-700 hover:bg-slate-50">
                Exit Card View
              </button>
            </div>
          </div>

          <template v-else>
            <div class="flex flex-wrap items-center justify-between gap-3">
              <div class="flex flex-wrap items-center gap-2.5">
                <div class="inline-flex rounded-lg bg-slate-100 p-0.5 text-xs font-medium border border-slate-200">
                  <button
                    @click="switchFolderMode('unassigned')"
                    class="px-3 py-1.5 rounded-md transition-colors flex items-center gap-1.5"
                    :class="folderMode === 'unassigned' ? 'bg-white text-slate-900 shadow-sm' : 'text-slate-600 hover:text-slate-900'"
                  >
                    Unassigned Only
                    <span v-if="folderData.totalUnassignedFolders > 0" class="px-1.5 py-0.2 rounded-full bg-amber-100 text-amber-800 text-[11px]">
                      {{ folderData.totalUnassignedFolders }}
                    </span>
                  </button>
                  <button
                    @click="switchFolderMode('all')"
                    class="px-3 py-1.5 rounded-md transition-colors"
                    :class="folderMode === 'all' ? 'bg-white text-slate-900 shadow-sm' : 'text-slate-600 hover:text-slate-900'"
                  >
                    All Folders (Fix Rotations)
                  </button>
                </div>

                <label class="flex items-center gap-2 text-xs text-slate-700 cursor-pointer ml-1">
                  <input v-model="renameOnDisk" type="checkbox" class="rounded" />
                  <span>Rename folders on disk</span>
                </label>
              </div>

              <button @click="runAutoMatch" class="px-3 py-1.5 text-xs font-medium rounded-lg border border-slate-300 bg-white text-slate-700 hover:bg-slate-50">
                Auto-Match Numbered Folders
              </button>
            </div>

            <div class="flex flex-wrap items-center gap-2.5">
              <input
                v-model="locationFilterQuery"
                @keydown.enter.prevent="availableLocations.length > 0 && selectParentPath(availableLocations[0].parentPath)"
                type="search"
                placeholder="Filter locations (e.g. 10a 1a beam)..."
                class="w-60 border border-slate-300 rounded-lg px-3 py-2 text-sm bg-white"
              />
              <select
                :value="selectedParent"
                @change="selectParentPath(($event.target as HTMLSelectElement).value)"
                class="flex-1 min-w-[260px] border border-slate-300 rounded-lg px-3 py-2 text-sm bg-white font-mono"
              >
                <option v-if="folderMode === 'unassigned'" value="">
                  All Unassigned Locations ({{ availableLocations.length }} location{{ availableLocations.length === 1 ? "" : "s" }}, {{ folderData.totalUnassignedPhotos.toLocaleString() }} photos)
                </option>
                <option v-if="selectedParent && !availableLocations.some((l) => l.parentPath === selectedParent)" :value="selectedParent">
                  {{ selectedParent }} (current)
                </option>
                <option v-if="folderMode === 'all' && availableLocations.length === 0" value="">No matching locations</option>
                <option v-for="loc in availableLocations" :key="loc.parentPath" :value="loc.parentPath">
                  {{ loc.parentPath || "(event root)" }} — {{ folderMode === "unassigned" ? `${loc.unassignedFolderCount} unassigned (${loc.unassignedPhotoCount} photos)` : `${loc.folderCount} folders (${loc.photoCount} photos)` }}
                </option>
              </select>
            </div>
          </template>
        </div>

        <!-- Sticky Pending Changes Bar -->
        <div
          v-if="stagedCount > 0"
          class="sticky top-2 z-20 rounded-xl bg-indigo-600 text-white px-4 py-3 shadow-lg flex flex-wrap items-center justify-between gap-3"
        >
          <div class="flex items-center gap-3 text-sm">
            <span class="px-2.5 py-0.5 rounded-full bg-white/20 font-semibold">{{ stagedCount }} staged</span>
            <span>
              {{ stagedCount }} folder change{{ stagedCount === 1 ? "" : "s" }} pending ({{ stagedPhotoTotal.toLocaleString() }} photos)
              <span v-if="renameOnDisk" class="text-indigo-200">· will rename on disk</span>
            </span>
          </div>
          <div class="flex items-center gap-2">
            <button @click="stagedMap = {}; showPreviewModal = false" class="px-3 py-1.5 text-xs font-medium rounded-lg bg-white/15 hover:bg-white/25 transition">
              Discard All
            </button>
            <button @click="showPreviewModal = true" class="px-4 py-1.5 text-sm font-semibold rounded-lg bg-white text-indigo-700 hover:bg-indigo-50 shadow-sm transition">
              Preview &amp; Confirm Changes &rarr;
            </button>
          </div>
        </div>

        <!-- Empty / Card Completed State -->
        <div v-if="folderData.folders.length === 0" class="bg-white rounded-xl border border-slate-200 p-12 text-center space-y-3">
          <template v-if="activeBatch">
            <div class="text-lg font-semibold text-emerald-700">Card #{{ activeBatch.sequenceNum }} complete! ✓</div>
            <p class="text-sm text-slate-500">
              All <strong>{{ activeBatch.totalFolders }}</strong> folder{{ activeBatch.totalFolders === 1 ? "" : "s" }}
              ({{ activeBatch.totalPhotos.toLocaleString() }} photos) in
              <code class="text-xs font-mono text-slate-700">{{ activeBatch.destinationRelPath || "(event root)" }}</code>
              are named and matched.
            </p>
            <div class="flex flex-wrap items-center justify-center gap-2.5 pt-1">
              <button
                v-if="ingestStore.pendingNamingBatches.length > 0"
                @click="selectIngestBatch(ingestStore.pendingNamingBatches[0].id)"
                class="px-4 py-2 text-sm font-semibold rounded-lg bg-indigo-600 text-white hover:bg-indigo-700 transition-colors"
              >
                Next Card (#{{ ingestStore.pendingNamingBatches[0].sequenceNum }} · {{ ingestStore.pendingNamingBatches[0].readerName || ingestStore.pendingNamingBatches[0].destinationRelPath.split('/').slice(-2).join('/') }}) &rarr;
              </button>
              <button @click="toggleCardShowAll(true)" class="px-3.5 py-2 text-sm font-medium rounded-lg border border-slate-300 bg-white text-slate-700 hover:bg-slate-50">
                Review All {{ activeBatch.totalFolders }} Folders on Card
              </button>
              <button @click="exitCardView" class="px-3.5 py-2 text-sm font-medium rounded-lg border border-slate-300 bg-white text-slate-700 hover:bg-slate-50">
                Back to All Locations
              </button>
            </div>
          </template>
          <template v-else>
            <div class="text-lg font-semibold text-emerald-700">
              {{ folderMode === "unassigned" ? "All folders matched!" : "No folders in this location" }}
            </div>
            <p class="text-sm text-slate-500">
              <template v-if="folderMode === 'unassigned'">
                There are no unassigned photo folders. Switch to
                <button @click="switchFolderMode('all')" class="text-blue-600 font-medium underline">
                  All Folders (Fix Rotations)
                </button>
                to fix out-of-order rotations or rename existing folders.
              </template>
              <template v-else>Select another location above or clear your location filter.</template>
            </p>
          </template>
        </div>

        <!-- Two-Column Folder + Roster Picker -->
        <div v-else class="grid grid-cols-1 lg:grid-cols-2 gap-5">
          <!-- Left: Folders -->
          <div class="bg-white rounded-xl border border-slate-200 overflow-hidden flex flex-col max-h-[68vh]">
            <div class="px-4 py-3 bg-slate-50 border-b border-slate-200">
              <h2 class="text-sm font-semibold text-slate-800">
                {{
                  activeBatch
                    ? `Card #${activeBatch.sequenceNum} — ${cardShowAllFolders ? "All Folders" : "Unnamed Folders"}`
                    : folderMode === "unassigned"
                      ? "Unassigned Folders"
                      : "Rotation Folders"
                }}
                ({{ folderData.folders.length }})
              </h2>
              <p class="text-xs text-slate-500">
                Click a folder, then click its true {{ subjectLabel.toLowerCase() }} on the right to stage changes. Click any photo to zoom.
              </p>
            </div>
            <div class="p-3 space-y-2.5 overflow-y-auto flex-1">
              <div
                v-for="folder in folderData.folders"
                :key="folder.folderPath"
                @click="selectFolder(folder)"
                class="w-full text-left p-3 rounded-lg border transition-all space-y-2 cursor-pointer"
                :class="
                  selectedFolderPath === folder.folderPath
                    ? 'border-blue-500 bg-blue-50/70 ring-2 ring-blue-200'
                    : isStaged(folder.folderPath)
                      ? 'border-indigo-300 bg-indigo-50/40 hover:border-indigo-400'
                      : 'border-slate-200 hover:border-slate-300 hover:bg-slate-50'
                "
              >
                <div class="flex items-start justify-between gap-2">
                  <div class="min-w-0 flex-1">
                    <div class="flex items-center gap-2 flex-wrap">
                      <span class="font-mono text-sm font-semibold text-slate-900 truncate">{{ folder.leafFolder }}</span>
                      <span class="px-2 py-0.5 text-[11px] rounded-full bg-slate-100 text-slate-600 font-medium">
                        {{ folder.photoCount }} photos
                      </span>
                    </div>
                    <div v-if="!selectedParent && folder.parentPath" class="text-[11px] text-slate-400 font-mono truncate mt-0.5">
                      {{ folder.parentPath }}
                    </div>

                    <div class="flex items-center flex-wrap gap-1.5 mt-1.5">
                      <span v-if="folder.assignedSubjectName" class="px-2 py-0.5 text-[11px] rounded bg-emerald-50 text-emerald-700 border border-emerald-200 font-medium">
                        Current: {{ folder.assignedSubjectName }}
                      </span>
                      <span v-else-if="folder.isMixed" class="px-2 py-0.5 text-[11px] rounded bg-orange-50 text-orange-700 border border-orange-200 font-medium">
                        Mixed assignment
                      </span>
                      <span v-else class="px-2 py-0.5 text-[11px] rounded bg-amber-50 text-amber-800 border border-amber-200 font-medium">
                        Unassigned
                      </span>

                      <span
                        v-if="isStaged(folder.folderPath)"
                        class="inline-flex items-center gap-1 px-2 py-0.5 text-[11px] rounded bg-indigo-600 text-white font-semibold"
                      >
                        <span>
                          &rarr;
                          {{ stagedMap[folder.folderPath] ? subjectById.get(stagedMap[folder.folderPath]!)?.displayName || "Selected" : "Unassign" }}
                        </span>
                        <button @click.stop="unstageFolder(folder.folderPath)" title="Undo staged change" class="ml-0.5 hover:text-indigo-200 font-bold">&times;</button>
                      </span>
                    </div>
                  </div>

                  <button
                    @click.stop="removeTarget = folder; removeMode = 'trash'"
                    title="Remove or delete this folder"
                    class="px-2 py-1 text-xs font-medium rounded-md text-slate-500 hover:text-red-700 hover:bg-red-50 border border-transparent hover:border-red-200 transition-colors flex-shrink-0"
                  >
                    Remove
                  </button>
                </div>

                <div v-if="folder.samplePhotoIds.length > 0 && eventStore.lanStatus.running" class="grid grid-cols-4 gap-1.5">
                  <button
                    v-for="(pid, idx) in folder.samplePhotoIds"
                    :key="pid"
                    type="button"
                    @click.stop="previewLightbox = { folder, index: idx }"
                    title="Click to inspect full preview"
                    class="group relative overflow-hidden rounded bg-slate-200 focus:outline-none focus:ring-2 focus:ring-blue-400"
                  >
                    <img :src="`${thumbBaseUrl}/${pid}`" loading="lazy" class="w-full aspect-[3/4] object-cover group-hover:scale-105 transition-transform" />
                  </button>
                </div>
              </div>
            </div>
          </div>

          <!-- Right: Smart Scoped Roster Picker -->
          <div class="bg-white rounded-xl border border-slate-200 overflow-hidden flex flex-col max-h-[68vh]">
            <div class="px-4 py-3 bg-slate-50 border-b border-slate-200 space-y-2.5">
              <div class="flex items-center justify-between gap-2">
                <h2 class="text-sm font-semibold text-slate-800 truncate">
                  <template v-if="selectedFolder">
                    {{ selectedFolder.assignedSubjectId ? "Reassign" : "Assign" }}
                    <span class="font-mono text-blue-700">"{{ selectedFolder.leafFolder }}"</span> &rarr;
                  </template>
                  <template v-else>Select a folder on the left</template>
                </h2>
                <button
                  v-if="selectedFolder && (selectedFolder.assignedSubjectId || selectedFolder.isMixed)"
                  @click="stageFolderTarget(null)"
                  class="px-2.5 py-1 text-xs font-medium rounded-md border border-slate-300 bg-white text-slate-600 hover:bg-slate-100 flex-shrink-0"
                >
                  Stage Unassign
                </button>
              </div>

              <div class="flex flex-wrap items-center justify-between gap-2 text-xs">
                <div class="flex flex-wrap items-center gap-1.5">
                  <button
                    v-if="sameGroupSubjects.length > 0"
                    @click="setScope('group')"
                    class="px-2.5 py-1 rounded-full font-medium transition-colors border"
                    :class="pickerScope === 'group' ? 'bg-indigo-600 text-white border-indigo-600' : 'bg-white text-slate-700 border-slate-300 hover:bg-slate-100'"
                  >
                    {{ shortGroupName || "Same Group" }} ({{ sameGroupSubjects.length }})
                  </button>
                  <button
                    v-if="folderData.detectedSessionGroup && sameSessionSubjects.length > 0"
                    @click="setScope('session')"
                    class="px-2.5 py-1 rounded-full font-medium transition-colors border"
                    :class="pickerScope === 'session' ? 'bg-indigo-600 text-white border-indigo-600' : 'bg-white text-slate-700 border-slate-300 hover:bg-slate-100'"
                  >
                    {{ folderData.detectedSessionGroup }} ({{ sameSessionSubjects.length }})
                  </button>
                  <button
                    @click="setScope('all')"
                    class="px-2.5 py-1 rounded-full font-medium transition-colors border"
                    :class="pickerScope === 'all' ? 'bg-indigo-600 text-white border-indigo-600' : 'bg-white text-slate-700 border-slate-300 hover:bg-slate-100'"
                  >
                    All Roster ({{ allSubjects.length.toLocaleString() }})
                  </button>
                </div>

                <label v-if="dimmedCount > 0" class="flex items-center gap-1.5 text-slate-600 cursor-pointer select-none" title="Hide already-matched athletes instead of greying them out in place">
                  <input v-model="hideMatchedSubjects" type="checkbox" class="rounded" />
                  <span>Hide matched ({{ dimmedCount }})</span>
                </label>
              </div>

              <div class="text-xs text-slate-500">
                <strong class="text-slate-800">{{ remainingCount }}</strong> remaining
                <template v-if="dimmedCount > 0">
                  · {{ dimmedCount }} already matched{{ folderData.sectionLabel ? ` on ${folderData.sectionLabel}` : "" }}
                </template>
              </div>

              <input
                v-model="pickerSearch"
                type="search"
                :placeholder="`Filter ${subjectLabel.toLowerCase()} by # or name...`"
                class="w-full border border-slate-300 bg-white rounded-md px-3 py-1.5 text-sm"
              />

              <div
                v-if="pickerSearch.trim() && (crossScopeMatchCounts.session > 0 || crossScopeMatchCounts.all > scopedPickerSubjects.length)"
                class="text-xs text-slate-600 flex flex-wrap items-center gap-2 bg-blue-50/80 border border-blue-200 rounded-md px-2.5 py-1.5"
              >
                <span>Late / different group?</span>
                <button v-if="pickerScope === 'group' && crossScopeMatchCounts.session > 0" @click="setScope('session')" class="text-blue-700 font-semibold underline">
                  Show {{ crossScopeMatchCounts.session }} in {{ folderData.detectedSessionGroup }}
                </button>
                <button v-if="pickerScope !== 'all' && crossScopeMatchCounts.all > 0" @click="setScope('all')" class="text-blue-700 font-semibold underline">
                  Show {{ crossScopeMatchCounts.all }} in All Roster
                </button>
              </div>
            </div>

            <div class="divide-y divide-slate-100 overflow-y-auto flex-1">
              <button
                v-for="item in visiblePickerItems"
                :key="item.subject.id"
                @click="stageFolderTarget(item.subject.id)"
                :disabled="!selectedFolder"
                class="w-full px-3.5 py-2 text-left flex items-center justify-between gap-3 transition-all"
                :class="[
                  selectedFolder ? 'cursor-pointer' : 'opacity-50 cursor-not-allowed',
                  selectedFolder && stagedMap[selectedFolder.folderPath] === item.subject.id
                    ? 'bg-indigo-50/90'
                    : item.dimmed
                      ? 'bg-slate-50/80 opacity-45 hover:opacity-85 hover:bg-slate-100/80'
                      : selectedFolder
                        ? 'hover:bg-indigo-50'
                        : '',
                ]"
              >
                <div class="flex items-center gap-3 min-w-0">
                  <div
                    class="w-11 h-11 rounded-lg overflow-hidden bg-slate-100 border border-slate-200 flex-shrink-0 flex items-center justify-center"
                    :class="item.dimmed ? 'grayscale-[70%]' : ''"
                  >
                    <img
                      v-if="(item.subject.photoCount > 0 || folderData.subjectSamplePhotos[item.subject.id]) && eventStore.lanStatus.running"
                      :src="`${avatarBaseUrl}/${item.subject.id}?v=${avatarVersion}`"
                      loading="lazy"
                      class="w-full h-full object-cover object-top"
                    />
                    <span v-else class="text-[10px] text-slate-400 font-mono">{{ item.subject.number || "—" }}</span>
                  </div>

                  <div class="min-w-0">
                    <div class="flex items-center gap-2">
                      <span
                        v-if="item.subject.number"
                        class="font-mono text-xs px-1.5 py-0.5 rounded"
                        :class="item.dimmed ? 'font-medium text-slate-500 bg-slate-200/70 line-through decoration-slate-400' : 'font-semibold text-slate-700 bg-slate-100'"
                      >
                        {{ item.subject.number }}
                      </span>
                      <span class="text-sm truncate" :class="item.dimmed ? 'font-normal text-slate-500' : 'font-medium text-slate-900'">
                        {{ item.subject.firstName }} {{ item.subject.lastName || "" }}
                      </span>
                    </div>
                    <div class="text-xs text-slate-400 flex items-center gap-1.5 mt-0.5">
                      <span v-if="item.subject.groupLabel">{{ item.subject.groupLabel }}</span>
                      <span>· {{ item.subject.photoCount }} photos</span>
                    </div>
                  </div>
                </div>

                <span v-if="item.badgeText" class="px-2 py-0.5 text-[11px] rounded-full flex-shrink-0" :class="item.badgeClass">
                  {{ item.badgeText }}
                </span>
              </button>

              <div v-if="visiblePickerItems.length === 0" class="p-8 text-center text-sm text-slate-500 space-y-2">
                <div v-if="hideMatchedSubjects && dimmedCount > 0">
                  All {{ dimmedCount }} matching {{ subjectLabel.toLowerCase() }}(s) in this scope are already matched.
                </div>
                <div v-else>No {{ subjectLabel.toLowerCase() }}s match this filter.</div>
                <div class="flex justify-center gap-2">
                  <button v-if="hideMatchedSubjects && dimmedCount > 0" @click="hideMatchedSubjects = false" class="px-3 py-1 text-xs rounded-md bg-slate-100 hover:bg-slate-200 text-slate-700 font-medium">
                    Show Matched ({{ dimmedCount }})
                  </button>
                  <button v-if="pickerScope !== 'session' && folderData.detectedSessionGroup" @click="setScope('session')" class="px-3 py-1 text-xs rounded-md bg-slate-100 hover:bg-slate-200 text-slate-700 font-medium">
                    Search {{ folderData.detectedSessionGroup }}
                  </button>
                  <button v-if="pickerScope !== 'all'" @click="setScope('all')" class="px-3 py-1 text-xs rounded-md bg-blue-50 hover:bg-blue-100 text-blue-700 font-medium">
                    Search All Roster
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </template>

    <FolderModals
      :subject-label="subjectLabel"
      :lan-running="eventStore.lanStatus.running"
      :thumb-base-url="thumbBaseUrl"
      :preview-base-url="previewBaseUrl"
      :show-preview-modal="showPreviewModal"
      :rename-on-disk="renameOnDisk"
      :staged-preview-rows="stagedPreviewRows"
      :staged-photo-total="stagedPhotoTotal"
      :has-merge-warning="hasMergeWarning"
      :is-applying-batch="isApplyingBatch"
      :remove-target="removeTarget"
      :remove-mode="removeMode"
      :is-removing="isRemoving"
      :preview-lightbox="previewLightbox"
      @update:show-preview-modal="showPreviewModal = $event"
      @update:rename-on-disk="renameOnDisk = $event"
      @update:remove-mode="removeMode = $event"
      @unstage="unstageFolder"
      @confirm-apply="confirmApplyStaged"
      @close-remove="removeTarget = null"
      @confirm-remove="confirmRemoveFolder"
      @close-lightbox="previewLightbox = null"
      @set-lightbox-index="previewLightbox && (previewLightbox = { ...previewLightbox, index: $event })"
    />
  </div>
</template>
