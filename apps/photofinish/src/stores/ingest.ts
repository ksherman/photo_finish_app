import { defineStore } from "pinia";
import { Channel, invoke } from "@tauri-apps/api/core";
import type {
  CardReaderInfo,
  CopyProgressEvent,
  IngestResult,
  ReaderConfig,
} from "@/types";

interface CopyState {
  isCopying: boolean;
  progress: CopyProgressEvent | null;
  error: string | null;
  lastResult: {
    copiedCount: number;
    queuedCount: number;
    durationSeconds: number;
    success: boolean;
  } | null;
}

export const useIngestStore = defineStore("ingest", {
  state: () => ({
    cardReaders: [] as CardReaderInfo[],
    readerConfigs: {} as Record<string, ReaderConfig>,
    copyStates: {} as Record<string, CopyState>,
  }),

  getters: {
    getReaderConfig: (state) => (readerId: string): ReaderConfig | null => {
      return state.readerConfigs[readerId] || null;
    },

    getCopyState: (state) => (readerId: string): CopyState => {
      return (
        state.copyStates[readerId] || {
          isCopying: false,
          progress: null,
          error: null,
          lastResult: null,
        }
      );
    },
  },

  actions: {
    async loadReaderConfigs() {
      const list = await invoke<ReaderConfig[]>("list_reader_configs");
      const map: Record<string, ReaderConfig> = {};
      for (const cfg of list) {
        map[cfg.readerId] = cfg;
      }
      this.readerConfigs = map;
    },

    async saveReaderConfig(config: ReaderConfig) {
      await invoke("save_reader_config", { config });
      this.readerConfigs[config.readerId] = { ...config };
    },

    async clearAllConfigs() {
      await invoke("clear_reader_configs");
      this.readerConfigs = {};
    },

    async discoverReaders() {
      try {
        this.cardReaders = await invoke<CardReaderInfo[]>("discover_card_readers");
      } catch (err) {
        console.error("Failed to discover card readers:", err);
      }
    },

    async ingestFromReader(
      reader: CardReaderInfo,
      config: ReaderConfig,
      destinationRelPath: string,
      includedFolders?: string[]
    ): Promise<IngestResult> {
      this.copyStates[reader.reader_id] = {
        isCopying: true,
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
            readerName: reader.display_name,
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
          copiedCount: result.copiedCount,
          queuedCount: result.queuedCount,
          durationSeconds,
          success: true,
        };
        await this.loadReaderConfigs();
        return result;
      } catch (err) {
        const msg = String(err);
        if (this.copyStates[reader.reader_id]) {
          this.copyStates[reader.reader_id].error = msg;
          this.copyStates[reader.reader_id].lastResult = {
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

    resetCopyState(readerId: string) {
      delete this.copyStates[readerId];
    },
  },
});
