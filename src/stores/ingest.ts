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
    getReaderConfig: (state) => (readerId: string): ReaderConfig | null =>
      state.readerConfigs[readerId] || null,

    getCopyState: (state) => (readerId: string): CopyState =>
      state.copyStates[readerId] || {
        isCopying: false,
        readerLabel: "",
        destinationRelPath: "",
        progress: null,
        error: null,
        lastResult: null,
      },

    activeCopies: (state): ActiveCopyInfo[] =>
      Object.entries(state.copyStates)
        .filter(([, cs]) => cs.isCopying)
        .map(([readerId, cs]) => ({
          readerId,
          readerLabel: cs.readerLabel || readerId,
          destinationRelPath: cs.destinationRelPath,
          progress: cs.progress,
        })),

    pendingNamingBatches: (state): IngestBatchSummary[] =>
      state.ingestBatches.filter((b) => b.unassignedFolders > 0),
  },

  actions: {
    async loadReaderConfigs() {
      const [list, savedGroupsJson] = await Promise.all([
        invoke<ReaderConfig[]>("list_reader_configs"),
        invoke<string | null>("get_app_setting", { key: "reader_group_order" }).catch(() => null),
      ]);
      const map: Record<string, ReaderConfig> = {};
      for (const cfg of list) map[cfg.readerId] = cfg;
      this.readerConfigs = map;

      let parsedGroups: string[] = [];
      if (savedGroupsJson) {
        try {
          const arr = JSON.parse(savedGroupsJson);
          if (Array.isArray(arr)) {
            parsedGroups = arr.filter(
              (x): x is string => typeof x === "string" && x.trim().length > 0
            );
          }
        } catch {
          parsedGroups = [];
        }
      }
      const merged = [...parsedGroups];
      for (const cfg of list) {
        const g = cfg.groupName?.trim();
        if (g && !merged.includes(g)) merged.push(g);
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
      await this.reassignReadersInGroup(oldName, trimmed);
      await this.saveGroupOrder(this.groupOrder.map((g) => (g === oldName ? trimmed : g)));
    },

    async removeGroup(name: string) {
      await this.reassignReadersInGroup(name, null);
      await this.saveGroupOrder(this.groupOrder.filter((g) => g !== name));
    },

    async reassignReadersInGroup(oldName: string, nextName: string | null) {
      const updates: ReaderLayoutUpdate[] = Object.values(this.readerConfigs)
        .filter((cfg) => (cfg.groupName || "") === oldName)
        .map((cfg) => ({
          readerId: cfg.readerId,
          groupName: nextName,
          sortOrder: cfg.sortOrder,
        }));
      if (updates.length > 0) await this.saveReaderLayout(updates);
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
        this.ingestBatches = await invoke<IngestBatchSummary[]>("list_ingest_batches", { limit });
      } catch (err) {
        console.warn("list_ingest_batches failed:", err);
        this.ingestBatches = [];
      }
    },

    async executeIngest(
      request: {
        readerId: string;
        readerName: string;
        volumeName: string;
        photographer: string;
        sourcePath: string;
        destinationRelPath: string;
        renamePrefix: string | null;
        autoRename: boolean;
        fileRenamePrefix: string | null;
        includedFolders: string[] | null;
      },
      reloadReaderConfigs: boolean
    ): Promise<IngestResult> {
      const { readerId, readerName, destinationRelPath } = request;
      this.copyStates[readerId] = {
        isCopying: true,
        readerLabel: readerName,
        destinationRelPath,
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
          request: { ...request, tags: {} },
          onProgress: channel,
        });

        this.copyStates[readerId].lastResult = {
          batchId: result.batchId,
          destinationRelPath,
          copiedCount: result.copiedCount,
          queuedCount: result.queuedCount,
          durationSeconds: (performance.now() - startTime) / 1000,
          success: true,
        };
        await Promise.all([
          reloadReaderConfigs ? this.loadReaderConfigs() : Promise.resolve(),
          this.loadIngestBatches(),
        ]);
        return result;
      } catch (err) {
        if (this.copyStates[readerId]) {
          this.copyStates[readerId].error = String(err);
          this.copyStates[readerId].lastResult = {
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
        if (this.copyStates[readerId]) {
          this.copyStates[readerId].isCopying = false;
        }
      }
    },

    async ingestFromReader(
      reader: CardReaderInfo,
      config: ReaderConfig,
      destinationRelPath: string,
      includedFolders?: string[]
    ): Promise<IngestResult> {
      const labelCore = config.customLabel?.trim() || reader.display_name;
      const readerName = config.groupName?.trim()
        ? `${config.groupName.trim()} · ${labelCore}`
        : labelCore;

      return this.executeIngest(
        {
          readerId: reader.reader_id,
          readerName,
          volumeName: reader.volume_name,
          photographer: config.photographer,
          sourcePath: reader.mount_point,
          destinationRelPath,
          renamePrefix: config.renamePrefix || null,
          autoRename: config.autoRename,
          fileRenamePrefix: config.fileRenamePrefix || null,
          includedFolders: includedFolders ?? null,
        },
        true
      );
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

      return this.executeIngest(
        {
          readerId,
          readerName: params.label.trim() || volumeName,
          volumeName,
          photographer: params.photographer.trim() || "Manual",
          sourcePath: params.sourcePath,
          destinationRelPath: params.destinationRelPath,
          renamePrefix: params.renamePrefix.trim() || null,
          autoRename: params.autoRename,
          fileRenamePrefix: null,
          includedFolders: null,
        },
        false
      );
    },

    resetCopyState(readerId: string) {
      delete this.copyStates[readerId];
    },
  },
});
