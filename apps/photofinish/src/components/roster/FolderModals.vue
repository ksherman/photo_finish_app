<script setup lang="ts">
import type { FolderItem, RemoveFolderMode, SubjectRecord } from "@/types";

export interface StagedPreviewRow {
  folder: FolderItem;
  targetSubjectId: string | null;
  targetSubject: SubjectRecord | null;
  targetLeaf: string;
  targetFolderPath: string;
  action: "swapped" | "renamed" | "merged" | "reassigned" | "unassigned";
}

defineProps<{
  subjectLabel: string;
  lanRunning: boolean;
  thumbBaseUrl: string;
  previewBaseUrl: string;
  // Preview & Confirm batch modal
  showPreviewModal: boolean;
  renameOnDisk: boolean;
  stagedPreviewRows: StagedPreviewRow[];
  stagedPhotoTotal: number;
  hasMergeWarning: boolean;
  isApplyingBatch: boolean;
  // Remove folder modal
  removeTarget: FolderItem | null;
  removeMode: RemoveFolderMode;
  isRemoving: boolean;
  // Lightbox
  previewLightbox: { folder: FolderItem; index: number } | null;
}>();

const emit = defineEmits<{
  "update:showPreviewModal": [val: boolean];
  "update:renameOnDisk": [val: boolean];
  "update:removeMode": [val: RemoveFolderMode];
  unstage: [folderPath: string];
  confirmApply: [];
  closeRemove: [];
  confirmRemove: [];
  closeLightbox: [];
  setLightboxIndex: [idx: number];
}>();
</script>

<template>
  <!-- ================= PREVIEW & CONFIRM BATCH MODAL ================= -->
  <div
    v-if="showPreviewModal"
    class="fixed inset-0 z-50 bg-slate-900/60 backdrop-blur-xs flex items-center justify-center p-4"
    @click.self="emit('update:showPreviewModal', false)"
  >
    <div class="bg-white rounded-2xl shadow-2xl border border-slate-200 w-full max-w-3xl max-h-[86vh] flex flex-col overflow-hidden">
      <div class="px-6 py-4 border-b border-slate-200 flex items-center justify-between">
        <div>
          <h2 class="text-lg font-bold text-slate-900">
            Preview &amp; Confirm {{ stagedPreviewRows.length }} Folder Change{{ stagedPreviewRows.length === 1 ? "" : "s" }}
          </h2>
          <p class="text-xs text-slate-500 mt-0.5">
            Review all assignments and resulting folder names below. Swaps &amp; cyclic shifts run atomically with zero photo mixing.
          </p>
        </div>
        <button @click="emit('update:showPreviewModal', false)" class="text-slate-400 hover:text-slate-600 text-2xl leading-none">&times;</button>
      </div>

      <div class="px-6 py-3 bg-slate-50 border-b border-slate-200 flex flex-wrap items-center justify-between gap-3">
        <label class="flex items-center gap-2 text-sm text-slate-700 cursor-pointer font-medium">
          <input
            type="checkbox"
            :checked="renameOnDisk"
            @change="emit('update:renameOnDisk', ($event.target as HTMLInputElement).checked)"
            class="rounded"
          />
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
              v-if="row.folder.samplePhotoIds[0] && lanRunning"
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

          <button @click="emit('unstage', row.folder.folderPath)" title="Remove from batch" class="px-2 py-1 text-xs text-slate-400 hover:text-red-600 rounded">
            &times;
          </button>
        </div>
      </div>

      <div class="px-6 py-4 border-t border-slate-200 bg-slate-50 flex items-center justify-end gap-2.5">
        <button @click="emit('update:showPreviewModal', false)" :disabled="isApplyingBatch" class="px-4 py-2 text-sm font-medium text-slate-600 hover:text-slate-900">
          Back
        </button>
        <button
          @click="emit('confirmApply')"
          :disabled="isApplyingBatch || stagedPreviewRows.length === 0"
          class="px-5 py-2 text-sm font-semibold rounded-lg bg-indigo-600 text-white hover:bg-indigo-700 disabled:bg-slate-400"
        >
          {{ isApplyingBatch ? "Applying..." : `Confirm & Apply ${stagedPreviewRows.length} Change${stagedPreviewRows.length === 1 ? "" : "s"}` }}
        </button>
      </div>
    </div>
  </div>

  <!-- ================= REMOVE FOLDER MODAL ================= -->
  <div
    v-if="removeTarget"
    class="fixed inset-0 z-50 bg-slate-900/60 backdrop-blur-xs flex items-center justify-center p-4"
    @click.self="emit('closeRemove')"
  >
    <div class="bg-white rounded-2xl shadow-2xl border border-slate-200 w-full max-w-lg overflow-hidden">
      <div class="px-6 py-4 border-b border-slate-200 flex items-center justify-between">
        <div>
          <h2 class="text-lg font-bold text-slate-900">Remove Folder "{{ removeTarget.leafFolder }}"</h2>
          <p class="text-xs text-slate-500 font-mono mt-0.5 truncate">{{ removeTarget.folderPath }}</p>
        </div>
        <button @click="emit('closeRemove')" class="text-slate-400 hover:text-slate-600 text-2xl leading-none">&times;</button>
      </div>

      <div class="p-6 space-y-4">
        <div v-if="removeTarget.samplePhotoIds.length > 0 && lanRunning" class="grid grid-cols-4 gap-2">
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
            <input :checked="removeMode === 'trash'" @change="emit('update:removeMode', 'trash')" type="radio" class="mt-0.5" />
            <div class="text-sm">
              <div class="font-semibold text-slate-900">Move to <code>_photofinish/trash</code> &amp; remove from event <span class="text-xs font-medium text-emerald-700">(Recommended)</span></div>
              <div class="text-xs text-slate-500 mt-0.5">Reversible and safe. Moves files out of the event tree so future Scans won't re-import them, and removes them from the kiosk.</div>
            </div>
          </label>

          <label
            class="flex items-start gap-3 p-3 rounded-xl border cursor-pointer transition-colors"
            :class="removeMode === 'delete_disk' ? 'border-red-500 bg-red-50/70' : 'border-slate-200 hover:bg-slate-50'"
          >
            <input :checked="removeMode === 'delete_disk'" @change="emit('update:removeMode', 'delete_disk')" type="radio" class="mt-0.5" />
            <div class="text-sm">
              <div class="font-semibold text-red-700">Permanently delete files from disk &amp; event</div>
              <div class="text-xs text-slate-500 mt-0.5">Cannot be undone. Deletes this folder and its JPEG originals permanently from disk and removes them from the database.</div>
            </div>
          </label>

          <label
            class="flex items-start gap-3 p-3 rounded-xl border cursor-pointer transition-colors"
            :class="removeMode === 'db_only' ? 'border-slate-500 bg-slate-50' : 'border-slate-200 hover:bg-slate-50'"
          >
            <input :checked="removeMode === 'db_only'" @change="emit('update:removeMode', 'db_only')" type="radio" class="mt-0.5" />
            <div class="text-sm">
              <div class="font-semibold text-slate-800">Remove from database only</div>
              <div class="text-xs text-slate-500 mt-0.5">Leaves original files untouched on disk. Note: clicking <strong>Scan</strong> later will re-import them.</div>
            </div>
          </label>
        </div>
      </div>

      <div class="px-6 py-4 border-t border-slate-200 bg-slate-50 flex items-center justify-end gap-2.5">
        <button @click="emit('closeRemove')" :disabled="isRemoving" class="px-4 py-2 text-sm font-medium text-slate-600 hover:text-slate-900">
          Cancel
        </button>
        <button
          @click="emit('confirmRemove')"
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
    @click.self="emit('closeLightbox')"
  >
    <div class="w-full max-w-4xl flex items-center justify-between text-white mb-3">
      <div class="text-sm">
        <span class="font-mono font-semibold">{{ previewLightbox.folder.leafFolder }}</span>
        <span class="text-slate-400 ml-2">
          Sample {{ previewLightbox.index + 1 }} of {{ previewLightbox.folder.samplePhotoIds.length }}
        </span>
      </div>
      <button @click="emit('closeLightbox')" class="px-3 py-1 rounded-lg bg-white/10 hover:bg-white/20 text-sm">
        Close (Esc)
      </button>
    </div>
    <div class="relative flex items-center justify-center max-h-[80vh] w-full max-w-4xl">
      <button
        v-if="previewLightbox.index > 0"
        @click="emit('setLightboxIndex', previewLightbox.index - 1)"
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
        @click="emit('setLightboxIndex', previewLightbox.index + 1)"
        class="absolute right-2 px-3 py-2 rounded-full bg-black/60 hover:bg-black/80 text-white text-xl"
      >
        &rsaquo;
      </button>
    </div>
  </div>
</template>
