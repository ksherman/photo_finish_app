import { defineStore } from "pinia";
import { Channel, invoke } from "@tauri-apps/api/core";
import type {
  CardReaderInfo,
  CopyProgressEvent,
  IngestBatchSummary,
  IngestResult,
  ReaderConfig,
  ReaderLayoutUpdate,
} from "@/types";

export interface CopyState {
  isCopying: boolean;
  readerLabel: string;
  destinationRelPath: string;
  progress: CopyProgressEvent | null;
  error: string | null;
  lastResult: {
    batchId: string;
    destinationRelPath: string;
    copiedCount: number;
    queuedCount: number;
    durationSeconds: number;
    success: boolean;
  } | null;
}

export interface ActiveCopyInfo {
  readerId: string;
  readerLabel: string;
  destinationRelPath: string;
  progress: CopyProgressEvent | null;
}

export const useIngestStore = defineStore("ingest", {
  state: () => ({
    cardReaders: [] as CardReaderInfo[],
    readerConfigs: {} as Record<string, ReaderConfig>,
    groupOrder: [] as string[],
    copyStates: {} as Record<string, CopyState>,
    ingestBatches: [] as IngestBatchSummary[],
  }),

  getters: {
    getReaderConfig: (state) => (readerId: string): ReaderConfig | null => {
      return state.readerConfigs[readerId] || null;
    },

    getCopyState: (state) => (readerId: string): CopyState => {
      return (
        state.copyStates[readerId] || {
          isCopying: false,
          readerLabel: "",
          destinationRelPath: "",
          progress: null,
          error: null,
          lastResult: null,
        }
      );
    },

    activeCopies: (state): ActiveCopyInfo[] => {
      return Object.entries(state.copyStates)
        .filter(([, cs]) => cs.isCopying)
        .map(([readerId, cs]) => ({
          readerId,
          readerLabel: cs.readerLabel || readerId,
          destinationRelPath: cs.destinationRelPath,
          progress: cs.progress,
        }));
    },

    pendingNamingBatches: (state): IngestBatchSummary[] => {
      return state.ingestBatches.filter((b) => b.unassignedFolders > 0);
    },
  },

  actions: {
    async loadReaderConfigs() {
      const [list, savedGroupsJson] = await Promise.all([
        invoke<ReaderConfig[]>("list_reader_configs"),
        invoke<string | null>("get_app_setting", { key: "reader_group_order" }).catch(() => null),
      ]);
      const map: Record<string, ReaderConfig> = {};
      for (const cfg of list) {
        map[cfg.readerId] = cfg;
      }
      this.readerConfigs = map;

      let parsedGroups: string[] = [];
      if (savedGroupsJson) {
        try {
          const arr = JSON.parse(savedGroupsJson);
          if (Array.isArray(arr)) {
            parsedGroups = arr.filter((x): x is string => typeof x === "string" && x.trim().length > 0);
          }
        } catch {
          parsedGroups = [];
        }
      }
      // Ensure any group referenced by a saved reader config is present in groupOrder
      const merged = [...parsedGroups];
      for (const cfg of list) {
        const g = cfg.groupName?.trim();
        if (g && !merged.includes(g)) {
          merged.push(g);
        }
      }
      this.groupOrder = merged;
    },

    async saveGroupOrder(groups: string[]) {
      const clean: string[] = [];
      for (const g of groups) {
        const t = g.trim();
        if (t && !clean.includes(t)) clean.push(t);
      }
      this.groupOrder = clean;
      await invoke("set_app_setting", {
        key: "reader_group_order",
        value: JSON.stringify(clean),
      });
    },

    async addGroup(name: string) {
      const trimmed = name.trim();
      if (!trimmed || this.groupOrder.includes(trimmed)) return;
      await this.saveGroupOrder([...this.groupOrder, trimmed]);
    },

    async renameGroup(oldName: string, newName: string) {
      const trimmed = newName.trim();
      if (!trimmed || trimmed === oldName) return;
      const updates: ReaderLayoutUpdate[] = [];
      for (const cfg of Object.values(this.readerConfigs)) {
        if ((cfg.groupName || "") === oldName) {
          updates.push({
            readerId: cfg.readerId,
            groupName: trimmed,
            sortOrder: cfg.sortOrder,
          });
        }
      }
      if (updates.length > 0) {
        await this.saveReaderLayout(updates);
      }
      const nextOrder = this.groupOrder.map((g) => (g === oldName ? trimmed : g));
      await this.saveGroupOrder(nextOrder);
    },

    async removeGroup(name: string) {
      const updates: ReaderLayoutUpdate[] = [];
      for (const cfg of Object.values(this.readerConfigs)) {
        if ((cfg.groupName || "") === name) {
          updates.push({
            readerId: cfg.readerId,
            groupName: null,
            sortOrder: cfg.sortOrder,
          });
        }
      }
      if (updates.length > 0) {
        await this.saveReaderLayout(updates);
      }
      await this.saveGroupOrder(this.groupOrder.filter((g) => g !== name));
    },

    async moveGroup(name: string, delta: -1 | 1) {
      const idx = this.groupOrder.indexOf(name);
      const target = idx + delta;
      if (idx < 0 || target < 0 || target >= this.groupOrder.length) return;
      const next = [...this.groupOrder];
      const [moved] = next.splice(idx, 1);
      next.splice(target, 0, moved);
      await this.saveGroupOrder(next);
    },

    async saveReaderConfig(config: ReaderConfig) {
      await invoke("save_reader_config", { config });
      this.readerConfigs[config.readerId] = { ...config };
      const g = config.groupName?.trim();
      if (g && !this.groupOrder.includes(g)) {
        await this.saveGroupOrder([...this.groupOrder, g]);
      }
    },

    async saveReaderLayout(updates: ReaderLayoutUpdate[]) {
      // Optimistically update local store so drag-and-drop feels instantaneous
      for (const u of updates) {
        const existing = this.readerConfigs[u.readerId];
        if (existing) {
          this.readerConfigs[u.readerId] = {
            ...existing,
            groupName: u.groupName?.trim() || null,
            sortOrder: u.sortOrder,
          };
        }
      }
      await invoke("save_reader_layout", { updates });
    },

    async deleteReaderConfig(readerId: string) {
      await invoke("delete_reader_config", { readerId });
      delete this.readerConfigs[readerId];
    },

    async clearAllConfigs() {
      await invoke("clear_reader_configs");
      this.readerConfigs = {};
      await this.saveGroupOrder([]);
    },

    async discoverReaders() {
      try {
        this.cardReaders = await invoke<CardReaderInfo[]>("discover_card_readers");
      } catch (err) {
        console.error("Failed to discover card readers:", err);
      }
    },

    async ejectReader(diskId: string) {
      await invoke("eject_usb_drive", { diskId });
      await this.discoverReaders();
    },

    async loadIngestBatches(limit = 40) {
      try {
        this.ingestBatches = await invoke<IngestBatchSummary[]>("list_ingest_batches", {
          limit,
        });
      } catch {
        this.ingestBatches = [];
      }
    },

    async ingestFromReader(
      reader: CardReaderInfo,
      config: ReaderConfig,
      destinationRelPath: string,
      includedFolders?: string[]
    ): Promise<IngestResult> {
      const labelCore = config.customLabel?.trim() || reader.display_name;
      const readerLabel = config.groupName?.trim()
        ? `${config.groupName.trim()} · ${labelCore}`
        : labelCore;

      this.copyStates[reader.reader_id] = {
        isCopying: true,
        readerLabel,
        destinationRelPath,
        progress: { total: 0, copied: 0, current_file: "", percentage: 0 },
        error: null,
        lastResult: null,
      };

      const sourcePath = config.cameraFolderPath
        ? `${reader.mount_point}/${config.cameraFolderPath}`
        : reader.mount_point;

      try {
        const startTime = performance.now();
        const channel = new Channel<CopyProgressEvent>();
        channel.onmessage = (progress) => {
          if (this.copyStates[reader.reader_id]) {
            this.copyStates[reader.reader_id].progress = progress;
          }
        };

        const result = await invoke<IngestResult>("ingest_from_reader", {
          request: {
            readerId: reader.reader_id,
            readerName: readerLabel,
            volumeName: reader.volume_name,
            photographer: config.photographer,
            sourcePath,
            destinationRelPath,
            renamePrefix: config.renamePrefix || null,
            autoRename: config.autoRename,
            fileRenamePrefix: config.fileRenamePrefix || null,
            includedFolders: includedFolders ?? null,
            tags: {},
          },
          onProgress: channel,
        });

        const durationSeconds = (performance.now() - startTime) / 1000;
        this.copyStates[reader.reader_id].lastResult = {
          batchId: result.batchId,
          destinationRelPath,
          copiedCount: result.copiedCount,
          queuedCount: result.queuedCount,
          durationSeconds,
          success: true,
        };
        await Promise.all([this.loadReaderConfigs(), this.loadIngestBatches()]);
        return result;
      } catch (err) {
        const msg = String(err);
        if (this.copyStates[reader.reader_id]) {
          this.copyStates[reader.reader_id].error = msg;
          this.copyStates[reader.reader_id].lastResult = {
            batchId: "",
            destinationRelPath,
            copiedCount: 0,
            queuedCount: 0,
            durationSeconds: 0,
            success: false,
          };
        }
        throw err;
      } finally {
        if (this.copyStates[reader.reader_id]) {
          this.copyStates[reader.reader_id].isCopying = false;
        }
      }
    },

    async ingestFromLocalFolder(params: {
      sourcePath: string;
      destinationRelPath: string;
      label: string;
      photographer: string;
      autoRename: boolean;
      renamePrefix: string;
    }): Promise<IngestResult> {
      const readerId = `folder-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`;
      const volumeName = params.sourcePath.split("/").filter(Boolean).pop() || "Folder";
      const readerLabel = params.label.trim() || volumeName;

      this.copyStates[readerId] = {
        isCopying: true,
        readerLabel,
        destinationRelPath: params.destinationRelPath,
        progress: { total: 0, copied: 0, current_file: "", percentage: 0 },
        error: null,
        lastResult: null,
      };

      try {
        const startTime = performance.now();
        const channel = new Channel<CopyProgressEvent>();
        channel.onmessage = (progress) => {
          if (this.copyStates[readerId]) {
            this.copyStates[readerId].progress = progress;
          }
        };

        const result = await invoke<IngestResult>("ingest_from_reader", {
          request: {
            readerId,
            readerName: readerLabel,
            volumeName,
            photographer: params.photographer.trim() || "Manual",
            sourcePath: params.sourcePath,
            destinationRelPath: params.destinationRelPath,
            renamePrefix: params.renamePrefix.trim() || null,
            autoRename: params.autoRename,
            fileRenamePrefix: null,
            includedFolders: null,
            tags: {},
          },
          onProgress: channel,
        });

        const durationSeconds = (performance.now() - startTime) / 1000;
        this.copyStates[readerId].lastResult = {
          batchId: result.batchId,
          destinationRelPath: params.destinationRelPath,
          copiedCount: result.copiedCount,
          queuedCount: result.queuedCount,
          durationSeconds,
          success: true,
        };
        await this.loadIngestBatches();
        return result;
      } catch (err) {
        if (this.copyStates[readerId]) {
          this.copyStates[readerId].error = String(err);
        }
        throw err;
      } finally {
        if (this.copyStates[readerId]) {
          this.copyStates[readerId].isCopying = false;
        }
      }
    },

    resetCopyState(readerId: string) {
      delete this.copyStates[readerId];
    },
  },
});
