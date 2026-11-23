<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useEventStore } from "@/stores/event";
import type {
  BatchReassignResult,
  FolderItem,
  ImportRosterResult,
  ListFoldersResponse,
  ParsedRosterFile,
  RemoveFolderMode,
  RemoveFolderResult,
  RosterEntryInput,
  SubjectRecord,
} from "@/types";

const eventStore = useEventStore();

type SubTab = "roster" | "folders";
type FolderMode = "unassigned" | "all";
type PickerScope = "group" | "session" | "all";

const subTab = ref<SubTab>("roster");

// Roster state
const groups = ref<string[]>([]);
const allSubjects = ref<SubjectRecord[]>([]);
const rosterSearch = ref("");
const selectedGroup = ref("");
const parsedFiles = ref<ParsedRosterFile[]>([]);
const fileGroupOverrides = ref<Record<string, string>>({});
const isImporting = ref(false);
const bannerMessage = ref<string | null>(null);
const errorMessage = ref<string | null>(null);

// Quick manual add
const showAddForm = ref(false);
const newNumber = ref("");
const newFirstName = ref("");
const newLastName = ref("");
const newGroup = ref("");
const newTeam = ref("");

// Folder manager state
const folderMode = ref<FolderMode>("unassigned");
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
});
const selectedFolderPath = ref<string | null>(null);
const pickerScope = ref<PickerScope>("all");
const userPinnedScope = ref(false);
const pickerSearch = ref("");
const renameOnDisk = ref(true);

// Staged batch changes: folderPath -> targetSubjectId (or null for explicit unassign)
const stagedMap = ref<Record<string, string | null>>({});
const showPreviewModal = ref(false);
const isApplyingBatch = ref(false);

// Remove folder modal state
const removeTarget = ref<FolderItem | null>(null);
const removeMode = ref<RemoveFolderMode>("trash");
const isRemoving = ref(false);

// Quick photo preview lightbox state (for reading small bib numbers / confirming athlete)
const previewLightbox = ref<{ folder: FolderItem; index: number } | null>(null);

const subjectLabel = computed(() => eventStore.activeEvent?.subjectLabel || "Subject");
const thumbBaseUrl = computed(() => {
  const port = eventStore.lanStatus.port || 4000;
  return `http://127.0.0.1:${port}/photos/thumbnail`;
});
const previewBaseUrl = computed(() => {
  const port = eventStore.lanStatus.port || 4000;
  return `http://127.0.0.1:${port}/photos/preview`;
});

const totalParsedEntries = computed(() =>
  parsedFiles.value.reduce((sum, f) => sum + f.entries.length, 0)
);

const subjectById = computed(() => {
  const map = new Map<string, SubjectRecord>();
  for (const s of allSubjects.value) {
    map.set(s.id, s);
  }
  return map;
});

function bibNum(s: SubjectRecord): number {
  if (!s.number) return Number.MAX_SAFE_INTEGER;
  const n = parseInt(s.number, 10);
  return Number.isNaN(n) ? Number.MAX_SAFE_INTEGER : n;
}

function rankAndFilterSubjects(list: SubjectRecord[], query: string): SubjectRecord[] {
  const raw = query.trim().toLowerCase();
  if (!raw) return list;

  const matches: Array<{ subject: SubjectRecord; rank: number; bib: number }> = [];
  for (const s of list) {
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

  return matches.map((m) => m.subject);
}

// Roster tab filtered table (instant 0ms client-side filter over full roster)
const filteredRosterSubjects = computed(() => {
  const base = selectedGroup.value
    ? allSubjects.value.filter((s) => s.groupLabel === selectedGroup.value)
    : allSubjects.value;
  return rankAndFilterSubjects(base, rosterSearch.value).slice(0, 500);
});

const selectedFolder = computed<FolderItem | null>(() => {
  if (!selectedFolderPath.value) return null;
  return folderData.value.folders.find((f) => f.folderPath === selectedFolderPath.value) || null;
});

// Location dropdown filtered by mode & multi-word search (e.g. "10a 1a beam")
const availableLocations = computed(() => {
  const base =
    folderMode.value === "unassigned"
      ? folderData.value.locations.filter((l) => l.unassignedFolderCount > 0)
      : folderData.value.locations;
  const tokens = locationFilterQuery.value
    .trim()
    .toLowerCase()
    .split(/\s+/)
    .filter(Boolean);
  if (tokens.length === 0) return base;
  return base.filter((loc) => {
    const hay = loc.parentPath.toLowerCase();
    return tokens.every((t) => hay.includes(t));
  });
});

const shortGroupName = computed(() => {
  const prefix = folderData.value.detectedGroupPrefix;
  if (!prefix) return null;
  const parts = prefix.split("/");
  return parts[parts.length - 1] || prefix;
});

const sameGroupIdSet = computed(() => new Set(folderData.value.sameGroupSubjectIds));

const sameGroupSubjects = computed(() => {
  if (sameGroupIdSet.value.size === 0) return [];
  return allSubjects.value.filter((s) => sameGroupIdSet.value.has(s.id));
});

const sameSessionSubjects = computed(() => {
  const sess = folderData.value.detectedSessionGroup;
  if (!sess) return [];
  return allSubjects.value.filter((s) => s.groupLabel === sess);
});

const scopedPickerSubjects = computed(() => {
  let pool: SubjectRecord[];
  if (pickerScope.value === "group" && sameGroupSubjects.value.length > 0) {
    pool = sameGroupSubjects.value;
  } else if (pickerScope.value === "session" && sameSessionSubjects.value.length > 0) {
    pool = sameSessionSubjects.value;
  } else {
    pool = allSubjects.value;
  }
  return rankAndFilterSubjects(pool, pickerSearch.value).slice(0, 300);
});

// Cross-scope fallback counts when typing a search query (e.g. late athlete from another group!)
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

// Track which subjects are already assigned in this location or staged to another folder
const subjectUsageInView = computed(() => {
  const stagedTo = new Map<string, string>(); // subjectId -> leafFolder
  const currentInLoc = new Map<string, string>(); // subjectId -> leafFolder
  for (const f of folderData.value.folders) {
    if (f.assignedSubjectId) {
      currentInLoc.set(f.assignedSubjectId, f.leafFolder);
    }
    if (Object.prototype.hasOwnProperty.call(stagedMap.value, f.folderPath)) {
      const targetId = stagedMap.value[f.folderPath];
      if (targetId) stagedTo.set(targetId, f.leafFolder);
    }
  }
  return { stagedTo, currentInLoc };
});

function sanitizeFolderName(name: string): string {
  return name.replace(/[/\\:*?"<>|]/g, "_").trim();
}

interface PreviewRow {
  folder: FolderItem;
  targetSubjectId: string | null;
  targetSubject: SubjectRecord | null;
  targetLeaf: string;
  targetFolderPath: string;
  action: "swapped" | "renamed" | "merged" | "reassigned" | "unassigned";
}

const stagedCount = computed(() => Object.keys(stagedMap.value).length);

const stagedPreviewRows = computed<PreviewRow[]>(() => {
  const entries = Object.entries(stagedMap.value);
  if (entries.length === 0) return [];

  const folderLookup = new Map<string, FolderItem>();
  for (const f of folderData.value.folders) {
    folderLookup.set(f.folderPath, f);
  }
  const existingPathsInView = new Set(folderData.value.folders.map((f) => f.folderPath));

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
    if (r.willRename) {
      targetCounts.set(r.targetFolderPath, (targetCounts.get(r.targetFolderPath) || 0) + 1);
    }
  }

  return raw.map((r) => {
    let action: PreviewRow["action"];
    if (!r.targetSubjectId) {
      action = "unassigned";
    } else if (!r.willRename) {
      action = "reassigned";
    } else if ((targetCounts.get(r.targetFolderPath) || 0) > 1) {
      action = "merged";
    } else if (movingAway.has(r.targetFolderPath)) {
      action = "swapped";
    } else if (existingPathsInView.has(r.targetFolderPath)) {
      action = "merged";
    } else {
      action = "renamed";
    }
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

const hasMergeWarning = computed(() =>
  stagedPreviewRows.value.some((r) => r.action === "merged")
);

async function loadRosterData() {
  if (!eventStore.activeEvent) return;
  try {
    const [g, s] = await Promise.all([
      invoke<string[]>("list_subject_groups"),
      invoke<SubjectRecord[]>("list_subjects", {
        query: null,
        groupLabel: null,
        limit: 10000,
      }),
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
    const res = await invoke<ListFoldersResponse>("list_folders", {
      parentFilter: selectedParent.value || null,
      onlyUnassigned: folderMode.value === "unassigned",
      contextParent: contextParentOverride ?? selectedFolder.value?.parentPath ?? null,
    });
    folderData.value = res;

    // In "All Folders" mode, auto-select the first location if none is selected yet
    // so we show a single clean rotation rather than 3,000+ folders.
    if (
      folderMode.value === "all" &&
      !selectedParent.value &&
      res.locations.length > 0
    ) {
      selectedParent.value = res.locations[0].parentPath;
      const locRes = await invoke<ListFoldersResponse>("list_folders", {
        parentFilter: selectedParent.value,
        onlyUnassigned: false,
        contextParent: selectedParent.value,
      });
      folderData.value = locRes;
    }

    // Keep or pick a sensible selected folder
    if (
      selectedFolderPath.value &&
      !folderData.value.folders.some((f) => f.folderPath === selectedFolderPath.value)
    ) {
      selectedFolderPath.value = folderData.value.folders[0]?.folderPath || null;
    } else if (!selectedFolderPath.value && folderData.value.folders.length > 0) {
      selectedFolderPath.value = folderData.value.folders[0].folderPath;
    }

    // Auto-select tightest smart scope unless user explicitly pinned another scope
    if (!userPinnedScope.value) {
      if (folderData.value.sameGroupSubjectIds.length > 0) {
        pickerScope.value = "group";
      } else if (folderData.value.detectedSessionGroup) {
        pickerScope.value = "session";
      } else {
        pickerScope.value = "all";
      }
    } else {
      if (pickerScope.value === "group" && folderData.value.sameGroupSubjectIds.length === 0) {
        pickerScope.value = folderData.value.detectedSessionGroup ? "session" : "all";
      } else if (pickerScope.value === "session" && !folderData.value.detectedSessionGroup) {
        pickerScope.value = "all";
      }
    }
  } catch (err) {
    errorMessage.value = String(err);
  }
}

function switchFolderMode(mode: FolderMode) {
  if (folderMode.value === mode) return;
  folderMode.value = mode;
  stagedMap.value = {};
  selectedFolderPath.value = null;
  locationFilterQuery.value = "";
  userPinnedScope.value = false;
  if (mode === "unassigned") {
    selectedParent.value = "";
  }
  loadFolders();
}

function jumpToFirstFilteredLocation() {
  if (availableLocations.value.length > 0) {
    selectParentPath(availableLocations.value[0].parentPath);
  }
}

function selectParentPath(parent: string) {
  selectedParent.value = parent;
  stagedMap.value = {};
  selectedFolderPath.value = null;
  userPinnedScope.value = false;
  loadFolders(parent || null);
}

function selectFolder(folder: FolderItem) {
  const prevParent = selectedFolder.value?.parentPath;
  selectedFolderPath.value = folder.folderPath;
  // When viewing "All Unassigned Locations" across multiple parents, refresh smart
  // group/session context whenever clicking a folder in a different parent path!
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
  // Clicking the folder's existing assignment cancels any staged change for it
  if (
    targetSubjectId !== null &&
    folder.assignedSubjectId === targetSubjectId &&
    !folder.isMixed
  ) {
    delete next[folder.folderPath];
  } else if (targetSubjectId === null && folder.assignedSubjectId === null && !folder.isMixed) {
    delete next[folder.folderPath];
  } else {
    next[folder.folderPath] = targetSubjectId;
  }
  stagedMap.value = next;

  // Auto-advance to the next folder so fixing an out-of-order rotation is click-click-click!
  const idx = folderData.value.folders.findIndex((f) => f.folderPath === folder.folderPath);
  if (idx >= 0 && idx + 1 < folderData.value.folders.length) {
    selectedFolderPath.value = folderData.value.folders[idx + 1].folderPath;
  }
}

function unstageFolder(folderPath: string) {
  const next = { ...stagedMap.value };
  delete next[folderPath];
  stagedMap.value = next;
  if (Object.keys(next).length === 0) {
    showPreviewModal.value = false;
  }
}

function clearAllStaged() {
  stagedMap.value = {};
  showPreviewModal.value = false;
}

async function confirmApplyStaged() {
  if (stagedPreviewRows.value.length === 0 || isApplyingBatch.value) return;
  isApplyingBatch.value = true;
  errorMessage.value = null;
  bannerMessage.value = null;
  try {
    const changes = stagedPreviewRows.value.map((r) => ({
      folderPath: r.folder.folderPath,
      targetSubjectId: r.targetSubjectId,
    }));
    const res = await invoke<BatchReassignResult>("apply_folder_reassignments", {
      changes,
      renameOnDisk: renameOnDisk.value,
    });
    stagedMap.value = {};
    showPreviewModal.value = false;
    const swapCount = res.changes.filter((c) => c.action === "swapped").length;
    const suffix = swapCount > 0 ? ` (${swapCount} swapped atomically)` : res.renamedOnDisk ? " (folders renamed on disk)" : "";
    bannerMessage.value = `Updated ${res.foldersUpdated} folder(s) / ${res.photosUpdated.toLocaleString()} photo(s)${suffix}.`;
    await Promise.all([loadFolders(), loadRosterData(), eventStore.refreshActiveEvent()]);
  } catch (err) {
    errorMessage.value = String(err);
  } finally {
    isApplyingBatch.value = false;
  }
}

function openRemoveModal(folder: FolderItem) {
  removeTarget.value = folder;
  removeMode.value = "trash";
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
    if (Object.prototype.hasOwnProperty.call(stagedMap.value, target.folderPath)) {
      unstageFolder(target.folderPath);
    }
    if (selectedFolderPath.value === target.folderPath) {
      selectedFolderPath.value = null;
    }
    removeTarget.value = null;
    if (res.mode === "trash") {
      bannerMessage.value = `Moved "${target.leafFolder}" (${res.photosRemoved} photos) to ${res.trashedTo || "_photofinish/trash"} and removed from event.`;
    } else if (res.mode === "delete_disk") {
      bannerMessage.value = `Permanently deleted "${target.leafFolder}" (${res.photosRemoved} photos) from disk and event.`;
    } else {
      bannerMessage.value = `Removed "${target.leafFolder}" (${res.photosRemoved} photos) from database (files remain on disk).`;
    }
    await Promise.all([loadFolders(), loadRosterData(), eventStore.refreshActiveEvent()]);
  } catch (err) {
    errorMessage.value = String(err);
  } finally {
    isRemoving.value = false;
  }
}

function openPreview(folder: FolderItem, index: number) {
  previewLightbox.value = { folder, index };
}

function onLightboxKeyDown(e: KeyboardEvent) {
  if (!previewLightbox.value) return;
  if (e.key === "Escape") {
    previewLightbox.value = null;
  } else if (e.key === "ArrowLeft" && previewLightbox.value.index > 0) {
    previewLightbox.value = { ...previewLightbox.value, index: previewLightbox.value.index - 1 };
  } else if (
    e.key === "ArrowRight" &&
    previewLightbox.value.index + 1 < previewLightbox.value.folder.samplePhotoIds.length
  ) {
    previewLightbox.value = { ...previewLightbox.value, index: previewLightbox.value.index + 1 };
  }
}

async function pickRosterFiles() {
  errorMessage.value = null;
  bannerMessage.value = null;
  const selected = await open({
    multiple: true,
    filters: [{ name: "Roster Files", extensions: ["txt", "csv"] }],
    title: "Select Roster File(s) (.txt or .csv)",
  });
  if (!selected) return;
  const paths = Array.isArray(selected) ? selected : [selected];
  if (paths.length === 0) return;

  try {
    const parsed = await invoke<ParsedRosterFile[]>("parse_roster_files", { paths });
    parsedFiles.value = parsed;
    const overrides: Record<string, string> = {};
    for (const file of parsed) {
      overrides[file.filePath] = file.defaultGroupLabel || "";
    }
    fileGroupOverrides.value = overrides;
  } catch (err) {
    errorMessage.value = String(err);
  }
}

async function confirmImportRoster() {
  if (parsedFiles.value.length === 0 || isImporting.value) return;
  isImporting.value = true;
  errorMessage.value = null;
  try {
    const entries: RosterEntryInput[] = [];
    for (const file of parsedFiles.value) {
      const overrideGroup = (fileGroupOverrides.value[file.filePath] || "").trim();
      for (const e of file.entries) {
        entries.push({
          ...e,
          groupLabel: overrideGroup || e.groupLabel,
        });
      }
    }
    const res = await invoke<ImportRosterResult>("import_roster", { entries });
    parsedFiles.value = [];
    fileGroupOverrides.value = {};
    bannerMessage.value = `Imported ${res.importedNew} new, updated ${res.updated} existing, and auto-matched ${res.autoMatchedPhotos.toLocaleString()} photo(s).`;
    await Promise.all([loadRosterData(), loadFolders(), eventStore.refreshActiveEvent()]);
  } catch (err) {
    errorMessage.value = String(err);
  } finally {
    isImporting.value = false;
  }
}

async function handleManualAdd() {
  if (!newFirstName.value.trim()) return;
  errorMessage.value = null;
  try {
    await invoke("add_subject", {
      number: newNumber.value.trim() || null,
      firstName: newFirstName.value.trim(),
      lastName: newLastName.value.trim() || null,
      groupLabel: newGroup.value.trim() || null,
      teamName: newTeam.value.trim() || null,
    });
    newNumber.value = "";
    newFirstName.value = "";
    newLastName.value = "";
    newTeam.value = "";
    showAddForm.value = false;
    await Promise.all([loadRosterData(), eventStore.refreshActiveEvent()]);
  } catch (err) {
    errorMessage.value = String(err);
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
    await Promise.all([loadRosterData(), loadFolders(), eventStore.refreshActiveEvent()]);
  } catch (err) {
    errorMessage.value = String(err);
  }
}

onMounted(() => {
  window.addEventListener("keydown", onLightboxKeyDown);
  if (eventStore.activeEvent) {
    loadRosterData();
    loadFolders();
  }
});

onUnmounted(() => {
  window.removeEventListener("keydown", onLightboxKeyDown);
});

watch(
  () => eventStore.activeEvent?.id,
  () => {
    selectedParent.value = "";
    selectedFolderPath.value = null;
    stagedMap.value = {};
    userPinnedScope.value = false;
    if (eventStore.activeEvent) {
      loadRosterData();
      loadFolders();
    }
  }
);
watch(
  () => eventStore.activeEvent?.photoCounts.total,
  (next, prev) => {
    if (eventStore.activeEvent && prev !== undefined && next !== prev) {
      loadFolders();
    }
  }
);
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
          <h1 class="text-2xl font-bold text-slate-900">Roster & Folders</h1>
          <p class="text-sm text-slate-500 mt-0.5">
            Import {{ subjectLabel.toLowerCase() }} rosters, fix out-of-order rotations, rename folders, or remove junk folders.
          </p>
        </div>

        <div class="flex items-center gap-2">
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
              <span
                v-if="folderData.totalUnassignedPhotos > 0"
                class="px-1.5 py-0.5 text-xs rounded-full bg-amber-100 text-amber-800"
              >
                {{ folderData.totalUnassignedPhotos.toLocaleString() }}
              </span>
            </button>
          </div>
        </div>
      </div>

      <div v-if="bannerMessage" class="p-3.5 rounded-lg bg-emerald-50 border border-emerald-200 text-sm text-emerald-800 flex items-center justify-between">
        <span>{{ bannerMessage }}</span>
        <button @click="bannerMessage = null" class="text-emerald-600 font-bold ml-4">&times;</button>
      </div>
      <div v-if="errorMessage" class="p-3.5 rounded-lg bg-red-50 border border-red-200 text-sm text-red-700 flex items-center justify-between">
        <span>{{ errorMessage }}</span>
        <button @click="errorMessage = null" class="text-red-600 font-bold ml-4">&times;</button>
      </div>

      <!-- ================= ROSTER TAB ================= -->
      <div v-show="subTab === 'roster'" class="space-y-5">
        <div class="flex flex-wrap items-center justify-between gap-3">
          <div class="flex flex-wrap items-center gap-2">
            <input
              v-model="rosterSearch"
              type="search"
              :placeholder="`Search ${subjectLabel.toLowerCase()} by # or name...`"
              class="w-64 border border-slate-300 bg-white rounded-lg px-3 py-2 text-sm"
            />
            <select
              v-model="selectedGroup"
              class="border border-slate-300 bg-white rounded-lg px-3 py-2 text-sm"
            >
              <option value="">All Groups / Sessions ({{ groups.length }})</option>
              <option v-for="g in groups" :key="g" :value="g">{{ g }}</option>
            </select>
          </div>

          <div class="flex flex-wrap items-center gap-2">
            <button
              @click="runAutoMatch"
              class="px-3.5 py-2 text-sm font-medium rounded-lg border border-slate-300 bg-white text-slate-700 hover:bg-slate-50"
            >
              Auto-Match Photos
            </button>
            <button
              @click="showAddForm = !showAddForm"
              class="px-3.5 py-2 text-sm font-medium rounded-lg border border-slate-300 bg-white text-slate-700 hover:bg-slate-50"
            >
              + Add {{ subjectLabel }}
            </button>
            <button
              @click="pickRosterFiles"
              class="px-4 py-2 text-sm font-medium rounded-lg bg-blue-600 text-white hover:bg-blue-700"
            >
              Import Roster Files (.txt / .csv)
            </button>
          </div>
        </div>

        <!-- Import Preview Panel -->
        <div v-if="parsedFiles.length > 0" class="bg-white rounded-xl border-2 border-blue-200 shadow-sm p-5 space-y-4">
          <div class="flex flex-wrap items-center justify-between gap-3">
            <div>
              <h2 class="text-base font-semibold text-slate-900">
                Ready to Import {{ totalParsedEntries.toLocaleString() }} {{ subjectLabel }}(s) from {{ parsedFiles.length }} file(s)
              </h2>
              <p class="text-xs text-slate-500 mt-0.5">
                Group/Session labels default from each filename and can be adjusted below. Any existing scanned photos matching these numbers will auto-link immediately.
              </p>
            </div>
            <div class="flex gap-2">
              <button
                @click="parsedFiles = []"
                class="px-3 py-1.5 text-sm text-slate-600 hover:text-slate-900"
              >
                Cancel
              </button>
              <button
                @click="confirmImportRoster"
                :disabled="isImporting || totalParsedEntries === 0"
                class="px-4 py-1.5 text-sm font-medium rounded-lg bg-blue-600 text-white hover:bg-blue-700 disabled:bg-slate-400"
              >
                {{ isImporting ? "Importing..." : `Import ${totalParsedEntries.toLocaleString()} & Auto-Match` }}
              </button>
            </div>
          </div>

          <div class="max-h-60 overflow-y-auto border border-slate-200 rounded-lg divide-y divide-slate-100">
            <div
              v-for="file in parsedFiles"
              :key="file.filePath"
              class="px-3.5 py-2.5 flex flex-wrap items-center justify-between gap-3 text-sm"
            >
              <div class="min-w-0">
                <div class="font-medium text-slate-800 font-mono text-xs">{{ file.filename }}</div>
                <div class="text-xs text-slate-500">
                  {{ file.entries.length }} entries
                  <span v-if="file.entries.length > 0" class="text-slate-400">
                    (e.g. {{ file.entries.slice(0, 3).map(e => `${e.number ? e.number + ' ' : ''}${e.firstName} ${e.lastName || ''}`.trim()).join(', ') }})
                  </span>
                </div>
                <div v-if="file.warnings.length > 0" class="text-xs text-amber-600 mt-0.5">
                  {{ file.warnings.length }} warning(s): {{ file.warnings[0] }}
                </div>
              </div>
              <div class="flex items-center gap-2">
                <label class="text-xs text-slate-500">Group:</label>
                <input
                  v-model="fileGroupOverrides[file.filePath]"
                  type="text"
                  placeholder="Session / Group"
                  class="w-36 border border-slate-300 rounded px-2 py-1 text-xs"
                />
              </div>
            </div>
          </div>
        </div>

        <!-- Quick Manual Add -->
        <div v-if="showAddForm" class="bg-white rounded-xl border border-slate-200 shadow-sm p-5">
          <h3 class="text-sm font-semibold text-slate-800 mb-3">Add Single {{ subjectLabel }}</h3>
          <form @submit.prevent="handleManualAdd" class="grid grid-cols-2 md:grid-cols-6 gap-3 items-end">
            <div>
              <label class="block text-xs text-slate-500 mb-1">Number / Bib</label>
              <input v-model="newNumber" type="text" placeholder="1022" class="w-full border border-slate-300 rounded px-2.5 py-1.5 text-sm font-mono" />
            </div>
            <div>
              <label class="block text-xs text-slate-500 mb-1">First Name *</label>
              <input v-model="newFirstName" type="text" required placeholder="Avery" class="w-full border border-slate-300 rounded px-2.5 py-1.5 text-sm" />
            </div>
            <div>
              <label class="block text-xs text-slate-500 mb-1">Last Name</label>
              <input v-model="newLastName" type="text" placeholder="W" class="w-full border border-slate-300 rounded px-2.5 py-1.5 text-sm" />
            </div>
            <div>
              <label class="block text-xs text-slate-500 mb-1">Group / Session</label>
              <input v-model="newGroup" type="text" placeholder="Session 1A" class="w-full border border-slate-300 rounded px-2.5 py-1.5 text-sm" />
            </div>
            <div>
              <label class="block text-xs text-slate-500 mb-1">Team / Club</label>
              <input v-model="newTeam" type="text" placeholder="Optional" class="w-full border border-slate-300 rounded px-2.5 py-1.5 text-sm" />
            </div>
            <button type="submit" class="px-4 py-1.5 bg-blue-600 text-white text-sm font-medium rounded-lg hover:bg-blue-700">
              Add
            </button>
          </form>
        </div>

        <!-- Roster Table -->
        <div class="bg-white rounded-xl border border-slate-200 shadow-sm overflow-hidden">
          <div class="overflow-x-auto">
            <table class="min-w-full divide-y divide-slate-200 text-sm">
              <thead class="bg-slate-50 text-xs uppercase text-slate-500">
                <tr>
                  <th class="px-4 py-3 text-left font-medium">Number</th>
                  <th class="px-4 py-3 text-left font-medium">{{ subjectLabel }}</th>
                  <th class="px-4 py-3 text-left font-medium">Group / Session</th>
                  <th class="px-4 py-3 text-left font-medium">Team</th>
                  <th class="px-4 py-3 text-right font-medium">Ready Photos</th>
                </tr>
              </thead>
              <tbody class="divide-y divide-slate-100">
                <tr v-for="s in filteredRosterSubjects" :key="s.id" class="hover:bg-slate-50">
                  <td class="px-4 py-2.5 font-mono text-slate-600">{{ s.number || "—" }}</td>
                  <td class="px-4 py-2.5 font-medium text-slate-900">{{ s.displayName }}</td>
                  <td class="px-4 py-2.5 text-slate-600">{{ s.groupLabel || "—" }}</td>
                  <td class="px-4 py-2.5 text-slate-500">{{ s.teamName || "—" }}</td>
                  <td class="px-4 py-2.5 text-right font-mono">
                    <span :class="s.photoCount > 0 ? 'text-emerald-700 font-semibold' : 'text-slate-400'">
                      {{ s.photoCount }}
                    </span>
                  </td>
                </tr>
                <tr v-if="filteredRosterSubjects.length === 0">
                  <td colspan="5" class="px-4 py-10 text-center text-slate-500">
                    No {{ subjectLabel.toLowerCase() }}s found. Click <strong>Import Roster Files</strong> to load your <code>Session *.txt</code> or <code>.csv</code> files.
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>
      </div>

      <!-- ================= FOLDERS TAB ================= -->
      <div v-show="subTab === 'folders'" class="space-y-4">
        <!-- Toolbar: Mode Toggle + Searchable Location Selector -->
        <div class="bg-white p-4 rounded-xl border border-slate-200 space-y-3">
          <div class="flex flex-wrap items-center justify-between gap-3">
            <div class="flex flex-wrap items-center gap-2.5">
              <div class="inline-flex rounded-lg bg-slate-100 p-0.5 text-xs font-medium border border-slate-200">
                <button
                  @click="switchFolderMode('unassigned')"
                  class="px-3 py-1.5 rounded-md transition-colors flex items-center gap-1.5"
                  :class="folderMode === 'unassigned' ? 'bg-white text-slate-900 shadow-sm' : 'text-slate-600 hover:text-slate-900'"
                >
                  Unassigned Only
                  <span
                    v-if="folderData.totalUnassignedFolders > 0"
                    class="px-1.5 py-0.2 rounded-full bg-amber-100 text-amber-800 text-[11px]"
                  >
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

            <button
              @click="runAutoMatch"
              class="px-3 py-1.5 text-xs font-medium rounded-lg border border-slate-300 bg-white text-slate-700 hover:bg-slate-50"
            >
              Auto-Match Numbered Folders
            </button>
          </div>

          <div class="flex flex-wrap items-center gap-2.5">
            <input
              v-model="locationFilterQuery"
              @keydown.enter.prevent="jumpToFirstFilteredLocation"
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
                All Unassigned Locations ({{ availableLocations.length }} location{{ availableLocations.length === 1 ? '' : 's' }}, {{ folderData.totalUnassignedPhotos.toLocaleString() }} photos)
              </option>
              <option
                v-if="selectedParent && !availableLocations.some((l) => l.parentPath === selectedParent)"
                :value="selectedParent"
              >
                {{ selectedParent }} (current)
              </option>
              <option v-if="folderMode === 'all' && availableLocations.length === 0" value="">
                No matching locations
              </option>
              <option v-for="loc in availableLocations" :key="loc.parentPath" :value="loc.parentPath">
                {{ loc.parentPath || "(event root)" }} — {{ folderMode === 'unassigned' ? `${loc.unassignedFolderCount} unassigned (${loc.unassignedPhotoCount} photos)` : `${loc.folderCount} folders (${loc.photoCount} photos)` }}
              </option>
            </select>
          </div>
        </div>

        <!-- Sticky Pending Changes Bar -->
        <div
          v-if="stagedCount > 0"
          class="sticky top-2 z-20 rounded-xl bg-indigo-600 text-white px-4 py-3 shadow-lg flex flex-wrap items-center justify-between gap-3"
        >
          <div class="flex items-center gap-3 text-sm">
            <span class="px-2.5 py-0.5 rounded-full bg-white/20 font-semibold">
              {{ stagedCount }} staged
            </span>
            <span>
              {{ stagedCount }} folder change{{ stagedCount === 1 ? '' : 's' }} pending ({{ stagedPhotoTotal.toLocaleString() }} photos)
              <span v-if="renameOnDisk" class="text-indigo-200">· will rename on disk</span>
            </span>
          </div>
          <div class="flex items-center gap-2">
            <button
              @click="clearAllStaged"
              class="px-3 py-1.5 text-xs font-medium rounded-lg bg-white/15 hover:bg-white/25 transition"
            >
              Discard All
            </button>
            <button
              @click="showPreviewModal = true"
              class="px-4 py-1.5 text-sm font-semibold rounded-lg bg-white text-indigo-700 hover:bg-indigo-50 shadow-sm transition"
            >
              Preview &amp; Confirm Changes &rarr;
            </button>
          </div>
        </div>

        <!-- Empty State -->
        <div v-if="folderData.folders.length === 0" class="bg-white rounded-xl border border-slate-200 p-12 text-center space-y-2">
          <div class="text-lg font-semibold text-emerald-700">
            {{ folderMode === 'unassigned' ? 'All folders matched!' : 'No folders in this location' }}
          </div>
          <p class="text-sm text-slate-500">
            <template v-if="folderMode === 'unassigned'">
              There are no unassigned photo folders. Switch to
              <button @click="switchFolderMode('all')" class="text-blue-600 font-medium underline">
                All Folders (Fix Rotations)
              </button>
              to fix out-of-order rotations or rename existing folders.
            </template>
            <template v-else>
              Select another location above or clear your location filter.
            </template>
          </p>
        </div>

        <!-- Two-Column Folder + Roster Picker -->
        <div v-else class="grid grid-cols-1 lg:grid-cols-2 gap-5">
          <!-- Left: Folders with Live Thumbnails & Staged Diff Pills -->
          <div class="bg-white rounded-xl border border-slate-200 overflow-hidden flex flex-col max-h-[68vh]">
            <div class="px-4 py-3 bg-slate-50 border-b border-slate-200 flex items-center justify-between gap-2">
              <div>
                <h2 class="text-sm font-semibold text-slate-800">
                  {{ folderMode === 'unassigned' ? 'Unassigned Folders' : 'Rotation Folders' }}
                  ({{ folderData.folders.length }})
                </h2>
                <p class="text-xs text-slate-500">
                  Click a folder, then click its true {{ subjectLabel.toLowerCase() }} on the right to stage changes. Click any photo to zoom.
                </p>
              </div>
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
                    : Object.prototype.hasOwnProperty.call(stagedMap, folder.folderPath)
                      ? 'border-indigo-300 bg-indigo-50/40 hover:border-indigo-400'
                      : 'border-slate-200 hover:border-slate-300 hover:bg-slate-50'
                "
              >
                <div class="flex items-start justify-between gap-2">
                  <div class="min-w-0 flex-1">
                    <div class="flex items-center gap-2 flex-wrap">
                      <span class="font-mono text-sm font-semibold text-slate-900 truncate">
                        {{ folder.leafFolder }}
                      </span>
                      <span class="px-2 py-0.5 text-[11px] rounded-full bg-slate-100 text-slate-600 font-medium">
                        {{ folder.photoCount }} photos
                      </span>
                    </div>
                    <div v-if="!selectedParent && folder.parentPath" class="text-[11px] text-slate-400 font-mono truncate mt-0.5">
                      {{ folder.parentPath }}
                    </div>

                    <!-- Current vs Staged Status -->
                    <div class="flex items-center flex-wrap gap-1.5 mt-1.5">
                      <span
                        v-if="folder.assignedSubjectName"
                        class="px-2 py-0.5 text-[11px] rounded bg-emerald-50 text-emerald-700 border border-emerald-200 font-medium"
                      >
                        Current: {{ folder.assignedSubjectName }}
                      </span>
                      <span
                        v-else-if="folder.isMixed"
                        class="px-2 py-0.5 text-[11px] rounded bg-orange-50 text-orange-700 border border-orange-200 font-medium"
                      >
                        Mixed assignment
                      </span>
                      <span
                        v-else
                        class="px-2 py-0.5 text-[11px] rounded bg-amber-50 text-amber-800 border border-amber-200 font-medium"
                      >
                        Unassigned
                      </span>

                      <template v-if="Object.prototype.hasOwnProperty.call(stagedMap, folder.folderPath)">
                        <span class="inline-flex items-center gap-1 px-2 py-0.5 text-[11px] rounded bg-indigo-600 text-white font-semibold">
                          <span>
                            &rarr;
                            {{
                              stagedMap[folder.folderPath]
                                ? subjectById.get(stagedMap[folder.folderPath]!)?.displayName || "Selected"
                                : "Unassign"
                            }}
                          </span>
                          <button
                            @click.stop="unstageFolder(folder.folderPath)"
                            title="Undo staged change"
                            class="ml-0.5 hover:text-indigo-200 font-bold"
                          >
                            &times;
                          </button>
                        </span>
                      </template>
                    </div>
                  </div>

                  <button
                    @click.stop="openRemoveModal(folder)"
                    title="Remove or delete this folder"
                    class="px-2 py-1 text-xs font-medium rounded-md text-slate-500 hover:text-red-700 hover:bg-red-50 border border-transparent hover:border-red-200 transition-colors flex-shrink-0"
                  >
                    Remove
                  </button>
                </div>

                <!-- Sample Thumbnails (click to zoom!) -->
                <div v-if="folder.samplePhotoIds.length > 0 && eventStore.lanStatus.running" class="grid grid-cols-4 gap-1.5">
                  <button
                    v-for="(pid, idx) in folder.samplePhotoIds"
                    :key="pid"
                    type="button"
                    @click.stop="openPreview(folder, idx)"
                    title="Click to inspect full preview"
                    class="group relative overflow-hidden rounded bg-slate-200 focus:outline-none focus:ring-2 focus:ring-blue-400"
                  >
                    <img
                      :src="`${thumbBaseUrl}/${pid}`"
                      loading="lazy"
                      class="w-full aspect-[3/4] object-cover group-hover:scale-105 transition-transform"
                    />
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
                    {{ selectedFolder.assignedSubjectId ? 'Reassign' : 'Assign' }}
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

              <!-- Smart Scope Pills: Same Group / Same Session / All Roster -->
              <div class="flex flex-wrap items-center gap-1.5 text-xs">
                <button
                  v-if="sameGroupSubjects.length > 0"
                  @click="setScope('group')"
                  class="px-2.5 py-1 rounded-full font-medium transition-colors border"
                  :class="
                    pickerScope === 'group'
                      ? 'bg-indigo-600 text-white border-indigo-600'
                      : 'bg-white text-slate-700 border-slate-300 hover:bg-slate-100'
                  "
                >
                  {{ shortGroupName || 'Same Group' }} ({{ sameGroupSubjects.length }})
                </button>
                <button
                  v-if="folderData.detectedSessionGroup && sameSessionSubjects.length > 0"
                  @click="setScope('session')"
                  class="px-2.5 py-1 rounded-full font-medium transition-colors border"
                  :class="
                    pickerScope === 'session'
                      ? 'bg-indigo-600 text-white border-indigo-600'
                      : 'bg-white text-slate-700 border-slate-300 hover:bg-slate-100'
                  "
                >
                  {{ folderData.detectedSessionGroup }} ({{ sameSessionSubjects.length }})
                </button>
                <button
                  @click="setScope('all')"
                  class="px-2.5 py-1 rounded-full font-medium transition-colors border"
                  :class="
                    pickerScope === 'all'
                      ? 'bg-indigo-600 text-white border-indigo-600'
                      : 'bg-white text-slate-700 border-slate-300 hover:bg-slate-100'
                  "
                >
                  All Roster ({{ allSubjects.length.toLocaleString() }})
                </button>
              </div>

              <div class="relative">
                <input
                  v-model="pickerSearch"
                  type="search"
                  :placeholder="`Search ${subjectLabel.toLowerCase()} by # or name...`"
                  class="w-full border border-slate-300 bg-white rounded-md px-3 py-1.5 text-sm"
                />
              </div>

              <!-- Helpful cross-scope banner for late / out-of-group athletes -->
              <div
                v-if="pickerSearch.trim() && (crossScopeMatchCounts.session > 0 || crossScopeMatchCounts.all > scopedPickerSubjects.length)"
                class="text-xs text-slate-600 flex flex-wrap items-center gap-2 bg-blue-50/80 border border-blue-200 rounded-md px-2.5 py-1.5"
              >
                <span>Late / different group?</span>
                <button
                  v-if="pickerScope === 'group' && crossScopeMatchCounts.session > 0"
                  @click="setScope('session')"
                  class="text-blue-700 font-semibold underline"
                >
                  Show {{ crossScopeMatchCounts.session }} in {{ folderData.detectedSessionGroup }}
                </button>
                <button
                  v-if="pickerScope !== 'all' && crossScopeMatchCounts.all > 0"
                  @click="setScope('all')"
                  class="text-blue-700 font-semibold underline"
                >
                  Show {{ crossScopeMatchCounts.all }} in All Roster
                </button>
              </div>
            </div>

            <div class="divide-y divide-slate-100 overflow-y-auto flex-1">
              <button
                v-for="s in scopedPickerSubjects"
                :key="s.id"
                @click="stageFolderTarget(s.id)"
                :disabled="!selectedFolder"
                class="w-full px-3.5 py-2 text-left flex items-center justify-between gap-3 transition-colors"
                :class="[
                  selectedFolder ? 'hover:bg-indigo-50 cursor-pointer' : 'opacity-50 cursor-not-allowed',
                  selectedFolder && stagedMap[selectedFolder.folderPath] === s.id
                    ? 'bg-indigo-50/90'
                    : '',
                ]"
              >
                <div class="flex items-center gap-3 min-w-0">
                  <!-- Cross-apparatus reference thumbnail (e.g. Floor photo while fixing Beam!) -->
                  <div class="w-9 h-12 rounded overflow-hidden bg-slate-100 border border-slate-200 flex-shrink-0 flex items-center justify-center">
                    <img
                      v-if="folderData.subjectSamplePhotos[s.id] && eventStore.lanStatus.running"
                      :src="`${thumbBaseUrl}/${folderData.subjectSamplePhotos[s.id]}`"
                      loading="lazy"
                      class="w-full h-full object-cover"
                    />
                    <span v-else class="text-[10px] text-slate-400 font-mono">
                      {{ s.number || "—" }}
                    </span>
                  </div>

                  <div class="min-w-0">
                    <div class="flex items-center gap-2">
                      <span v-if="s.number" class="font-mono text-xs font-semibold text-slate-600 bg-slate-100 px-1.5 py-0.5 rounded">
                        {{ s.number }}
                      </span>
                      <span class="text-sm font-medium text-slate-900 truncate">
                        {{ s.firstName }} {{ s.lastName || "" }}
                      </span>
                    </div>
                    <div class="text-xs text-slate-400 flex items-center gap-1.5 mt-0.5">
                      <span v-if="s.groupLabel">{{ s.groupLabel }}</span>
                      <span>· {{ s.photoCount }} photos</span>
                    </div>
                  </div>
                </div>

                <div class="flex items-center gap-1.5 flex-shrink-0">
                  <span
                    v-if="selectedFolder && stagedMap[selectedFolder.folderPath] === s.id"
                    class="px-2 py-0.5 text-[11px] rounded-full bg-indigo-600 text-white font-semibold"
                  >
                    Staged here
                  </span>
                  <span
                    v-else-if="subjectUsageInView.stagedTo.has(s.id)"
                    class="px-2 py-0.5 text-[11px] rounded-full bg-indigo-100 text-indigo-700 font-medium"
                  >
                    Staged
                  </span>
                  <span
                    v-else-if="selectedFolder?.assignedSubjectId === s.id"
                    class="px-2 py-0.5 text-[11px] rounded-full bg-emerald-100 text-emerald-700 font-medium"
                  >
                    Current
                  </span>
                  <span
                    v-else-if="subjectUsageInView.currentInLoc.has(s.id)"
                    class="px-2 py-0.5 text-[11px] rounded-full bg-slate-100 text-slate-500"
                  >
                    In rotation
                  </span>
                </div>
              </button>

              <div v-if="scopedPickerSubjects.length === 0" class="p-8 text-center text-sm text-slate-500 space-y-2">
                <div>No {{ subjectLabel.toLowerCase() }}s match this filter.</div>
                <div class="flex justify-center gap-2">
                  <button
                    v-if="pickerScope !== 'session' && folderData.detectedSessionGroup"
                    @click="setScope('session')"
                    class="px-3 py-1 text-xs rounded-md bg-slate-100 hover:bg-slate-200 text-slate-700 font-medium"
                  >
                    Search {{ folderData.detectedSessionGroup }}
                  </button>
                  <button
                    v-if="pickerScope !== 'all'"
                    @click="setScope('all')"
                    class="px-3 py-1 text-xs rounded-md bg-blue-50 hover:bg-blue-100 text-blue-700 font-medium"
                  >
                    Search All Roster
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </template>

    <!-- ================= PREVIEW & CONFIRM BATCH MODAL ================= -->
    <div
      v-if="showPreviewModal"
      class="fixed inset-0 z-50 bg-slate-900/60 backdrop-blur-xs flex items-center justify-center p-4"
      @click.self="showPreviewModal = false"
    >
      <div class="bg-white rounded-2xl shadow-2xl border border-slate-200 w-full max-w-3xl max-h-[86vh] flex flex-col overflow-hidden">
        <div class="px-6 py-4 border-b border-slate-200 flex items-center justify-between">
          <div>
            <h2 class="text-lg font-bold text-slate-900">
              Preview &amp; Confirm {{ stagedPreviewRows.length }} Folder Change{{ stagedPreviewRows.length === 1 ? '' : 's' }}
            </h2>
            <p class="text-xs text-slate-500 mt-0.5">
              Review all assignments and resulting folder names below. Swaps &amp; cyclic shifts run atomically with zero photo mixing.
            </p>
          </div>
          <button @click="showPreviewModal = false" class="text-slate-400 hover:text-slate-600 text-2xl leading-none">&times;</button>
        </div>

        <div class="px-6 py-3 bg-slate-50 border-b border-slate-200 flex flex-wrap items-center justify-between gap-3">
          <label class="flex items-center gap-2 text-sm text-slate-700 cursor-pointer font-medium">
            <input v-model="renameOnDisk" type="checkbox" class="rounded" />
            <span>Rename folders on disk to match selected {{ subjectLabel.toLowerCase() }} names</span>
          </label>
          <span class="text-xs text-slate-500 font-mono">
            {{ stagedPhotoTotal.toLocaleString() }} total photos affected
          </span>
        </div>

        <div v-if="hasMergeWarning" class="mx-6 mt-4 p-3 rounded-lg bg-amber-50 border border-amber-200 text-xs text-amber-800">
          <strong>Merge detected:</strong> One or more target folders already exist and are not moving out of the way. Photos will safely merge into the target folder (duplicate filenames are automatically suffixed so no photos are overwritten).
        </div>

        <div class="p-6 overflow-y-auto flex-1 space-y-2.5">
          <div
            v-for="row in stagedPreviewRows"
            :key="row.folder.folderPath"
            class="p-3 rounded-xl border border-slate-200 bg-slate-50/50 flex items-center justify-between gap-3"
          >
            <div class="flex items-center gap-3 min-w-0 flex-1">
              <img
                v-if="row.folder.samplePhotoIds[0] && eventStore.lanStatus.running"
                :src="`${thumbBaseUrl}/${row.folder.samplePhotoIds[0]}`"
                class="w-10 h-13 rounded object-cover bg-slate-200 flex-shrink-0"
              />
              <div class="min-w-0 flex-1">
                <div class="flex items-center gap-2 flex-wrap text-sm">
                  <span class="font-mono font-semibold text-slate-700">{{ row.folder.leafFolder }}</span>
                  <span class="text-slate-400">&rarr;</span>
                  <span class="font-semibold text-indigo-700">
                    {{ row.targetSubject ? row.targetSubject.displayName : "Unassigned" }}
                  </span>
                  <span
                    class="px-2 py-0.5 text-[11px] rounded-full font-semibold uppercase tracking-wide"
                    :class="{
                      'bg-blue-100 text-blue-700': row.action === 'swapped',
                      'bg-emerald-100 text-emerald-700': row.action === 'renamed',
                      'bg-amber-100 text-amber-800': row.action === 'merged',
                      'bg-slate-200 text-slate-700': row.action === 'reassigned',
                      'bg-rose-100 text-rose-700': row.action === 'unassigned',
                    }"
                  >
                    {{ row.action }}
                  </span>
                </div>
                <div class="text-xs text-slate-500 mt-0.5 flex items-center gap-2 flex-wrap">
                  <span>{{ row.folder.photoCount }} photos</span>
                  <span v-if="renameOnDisk && row.targetFolderPath !== row.folder.folderPath" class="font-mono text-slate-400 truncate">
                    Disk: {{ row.targetFolderPath }}
                  </span>
                </div>
              </div>
            </div>

            <button
              @click="unstageFolder(row.folder.folderPath)"
              title="Remove from batch"
              class="px-2 py-1 text-xs text-slate-400 hover:text-red-600 rounded"
            >
              &times;
            </button>
          </div>
        </div>

        <div class="px-6 py-4 border-t border-slate-200 bg-slate-50 flex items-center justify-end gap-2.5">
          <button
            @click="showPreviewModal = false"
            :disabled="isApplyingBatch"
            class="px-4 py-2 text-sm font-medium text-slate-600 hover:text-slate-900"
          >
            Back
          </button>
          <button
            @click="confirmApplyStaged"
            :disabled="isApplyingBatch || stagedPreviewRows.length === 0"
            class="px-5 py-2 text-sm font-semibold rounded-lg bg-indigo-600 text-white hover:bg-indigo-700 disabled:bg-slate-400"
          >
            {{ isApplyingBatch ? "Applying..." : `Confirm & Apply ${stagedPreviewRows.length} Change${stagedPreviewRows.length === 1 ? '' : 's'}` }}
          </button>
        </div>
      </div>
    </div>

    <!-- ================= REMOVE FOLDER MODAL ================= -->
    <div
      v-if="removeTarget"
      class="fixed inset-0 z-50 bg-slate-900/60 backdrop-blur-xs flex items-center justify-center p-4"
      @click.self="removeTarget = null"
    >
      <div class="bg-white rounded-2xl shadow-2xl border border-slate-200 w-full max-w-lg overflow-hidden">
        <div class="px-6 py-4 border-b border-slate-200 flex items-center justify-between">
          <div>
            <h2 class="text-lg font-bold text-slate-900">Remove Folder "{{ removeTarget.leafFolder }}"</h2>
            <p class="text-xs text-slate-500 font-mono mt-0.5 truncate">{{ removeTarget.folderPath }}</p>
          </div>
          <button @click="removeTarget = null" class="text-slate-400 hover:text-slate-600 text-2xl leading-none">&times;</button>
        </div>

        <div class="p-6 space-y-4">
          <div v-if="removeTarget.samplePhotoIds.length > 0 && eventStore.lanStatus.running" class="grid grid-cols-4 gap-2">
            <img
              v-for="pid in removeTarget.samplePhotoIds"
              :key="pid"
              :src="`${thumbBaseUrl}/${pid}`"
              class="w-full aspect-[3/4] object-cover rounded-lg bg-slate-200"
            />
          </div>

          <p class="text-sm text-slate-600">
            Choose how to remove these <strong>{{ removeTarget.photoCount }} photo(s)</strong>:
          </p>

          <div class="space-y-2">
            <label
              class="flex items-start gap-3 p-3 rounded-xl border cursor-pointer transition-colors"
              :class="removeMode === 'trash' ? 'border-blue-500 bg-blue-50/70' : 'border-slate-200 hover:bg-slate-50'"
            >
              <input v-model="removeMode" type="radio" value="trash" class="mt-0.5" />
              <div class="text-sm">
                <div class="font-semibold text-slate-900">Move to <code>_photofinish/trash</code> &amp; remove from event <span class="text-xs font-medium text-emerald-700">(Recommended)</span></div>
                <div class="text-xs text-slate-500 mt-0.5">
                  Reversible and safe. Moves files out of the event tree so future Scans won't re-import them, and removes them from the kiosk.
                </div>
              </div>
            </label>

            <label
              class="flex items-start gap-3 p-3 rounded-xl border cursor-pointer transition-colors"
              :class="removeMode === 'delete_disk' ? 'border-red-500 bg-red-50/70' : 'border-slate-200 hover:bg-slate-50'"
            >
              <input v-model="removeMode" type="radio" value="delete_disk" class="mt-0.5" />
              <div class="text-sm">
                <div class="font-semibold text-red-700">Permanently delete files from disk &amp; event</div>
                <div class="text-xs text-slate-500 mt-0.5">
                  Cannot be undone. Deletes this folder and its JPEG originals permanently from disk and removes them from the database.
                </div>
              </div>
            </label>

            <label
              class="flex items-start gap-3 p-3 rounded-xl border cursor-pointer transition-colors"
              :class="removeMode === 'db_only' ? 'border-slate-500 bg-slate-50' : 'border-slate-200 hover:bg-slate-50'"
            >
              <input v-model="removeMode" type="radio" value="db_only" class="mt-0.5" />
              <div class="text-sm">
                <div class="font-semibold text-slate-800">Remove from database only</div>
                <div class="text-xs text-slate-500 mt-0.5">
                  Leaves original files untouched on disk. Note: clicking <strong>Scan</strong> later will re-import them.
                </div>
              </div>
            </label>
          </div>
        </div>

        <div class="px-6 py-4 border-t border-slate-200 bg-slate-50 flex items-center justify-end gap-2.5">
          <button
            @click="removeTarget = null"
            :disabled="isRemoving"
            class="px-4 py-2 text-sm font-medium text-slate-600 hover:text-slate-900"
          >
            Cancel
          </button>
          <button
            @click="confirmRemoveFolder"
            :disabled="isRemoving"
            class="px-4 py-2 text-sm font-semibold rounded-lg text-white disabled:bg-slate-400"
            :class="removeMode === 'delete_disk' ? 'bg-red-600 hover:bg-red-700' : 'bg-slate-900 hover:bg-slate-800'"
          >
            {{
              isRemoving
                ? "Removing..."
                : removeMode === "delete_disk"
                  ? `Permanently Delete (${removeTarget.photoCount})`
                  : removeMode === "trash"
                    ? `Move to Trash (${removeTarget.photoCount})`
                    : `Remove from DB (${removeTarget.photoCount})`
            }}
          </button>
        </div>
      </div>
    </div>

    <!-- ================= QUICK PHOTO PREVIEW LIGHTBOX ================= -->
    <div
      v-if="previewLightbox"
      class="fixed inset-0 z-50 bg-black/90 flex flex-col items-center justify-center p-6"
      @click.self="previewLightbox = null"
    >
      <div class="w-full max-w-4xl flex items-center justify-between text-white mb-3">
        <div class="text-sm">
          <span class="font-mono font-semibold">{{ previewLightbox.folder.leafFolder }}</span>
          <span class="text-slate-400 ml-2">
            Sample {{ previewLightbox.index + 1 }} of {{ previewLightbox.folder.samplePhotoIds.length }}
          </span>
        </div>
        <button @click="previewLightbox = null" class="px-3 py-1 rounded-lg bg-white/10 hover:bg-white/20 text-sm">
          Close (Esc)
        </button>
      </div>
      <div class="relative flex items-center justify-center max-h-[80vh] w-full max-w-4xl">
        <button
          v-if="previewLightbox.index > 0"
          @click="previewLightbox = { ...previewLightbox, index: previewLightbox.index - 1 }"
          class="absolute left-2 px-3 py-2 rounded-full bg-black/60 hover:bg-black/80 text-white text-xl"
        >
          &lsaquo;
        </button>
        <img
          :src="`${previewBaseUrl}/${previewLightbox.folder.samplePhotoIds[previewLightbox.index]}`"
          class="max-h-[78vh] max-w-full rounded-lg object-contain shadow-2xl"
        />
        <button
          v-if="previewLightbox.index + 1 < previewLightbox.folder.samplePhotoIds.length"
          @click="previewLightbox = { ...previewLightbox, index: previewLightbox.index + 1 }"
          class="absolute right-2 px-3 py-2 rounded-full bg-black/60 hover:bg-black/80 text-white text-xl"
        >
          &rsaquo;
        </button>
      </div>
    </div>
  </div>
</template>
