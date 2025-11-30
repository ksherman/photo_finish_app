import { defineStore } from "pinia";
import { invoke, Channel } from "@tauri-apps/api/core";
import type {
  VolumeInfo,
  DirectoryEntry,
  CopyProgressEvent,
  CardReaderInfo,
} from "@/types";

interface CopyState {
  isCopying: boolean;
  progress: CopyProgressEvent | null;
  error: string | null;
  lastResult: { count: number; success: boolean } | null;
}

export const useCardReaderStore = defineStore("cardReader", {
  state: () => ({
    volumes: [] as VolumeInfo[],
    cardReaders: [] as CardReaderInfo[],
    
    // Selection state for Setup UI
    selectedVolume: null as VolumeInfo | null,
    selectedReader: null as CardReaderInfo | null,
    directories: [] as DirectoryEntry[], // For preview in Setup
    
    // Per-reader state
    copyStates: {} as Record<string, CopyState>,
  }),

  getters: {
    getCopyState: (state) => (readerId: string) => {
      return state.copyStates[readerId] || {
        isCopying: false,
        progress: null,
        error: null,
        lastResult: null
      };
    }
  },

  actions: {
    async discoverReaders() {
      try {
        const readers = await invoke<CardReaderInfo[]>(
          "discover_card_readers"
        );
        
        // Preserve existing readers' object references if possible or just replace
        // But we need to update file counts.
        this.cardReaders = readers;

        // If we have a selected reader, update it
        if (this.selectedReader) {
          const updated = this.cardReaders.find(
            (r) => r.reader_id === this.selectedReader?.reader_id
          );
          if (updated) {
            this.selectedReader = updated;
          } else {
            // Reader was disconnected
            this.clearSelection();
          }
        }
      } catch (error) {
        console.error("Failed to discover card readers:", error);
      }
    },

    async refreshVolumes() {
      try {
        this.volumes = await invoke<VolumeInfo[]>("list_volumes");
      } catch (error) {
        console.error("Failed to list volumes:", error);
      }
    },

    async selectReader(reader: CardReaderInfo, cameraFolderPath?: string) {
      this.selectedReader = reader;
      await this.loadDirectoriesFromReader(cameraFolderPath);
    },

    async loadDirectoriesFromReader(cameraFolderPath?: string) {
      if (!this.selectedReader) return;

      try {
        let basePath = this.selectedReader.mount_point;
        if (cameraFolderPath) {
          basePath = `${basePath}/${cameraFolderPath}`;
        }

        const entries = await invoke<DirectoryEntry[]>("list_directory", {
          path: basePath,
        });

        this.directories = entries.filter((d) => d.is_directory);
      } catch (error) {
        console.error("Failed to load directories:", error);
        this.directories = [];
      }
    },

    async copyFiles(
      readerId: string,
      destinationPath: string,
      cameraFolderPath?: string,
      renamePrefix?: string,
      autoRename?: boolean
    ): Promise<number> {
      const reader = this.cardReaders.find(r => r.reader_id === readerId);
      if (!reader) throw new Error("Reader not found");

      // Initialize state for this reader
      this.copyStates[readerId] = {
        isCopying: true,
        progress: {
          total: 0,
          copied: 0,
          current_file: "",
          percentage: 0,
        },
        error: null,
        lastResult: null
      };

      let sourcePath = reader.mount_point;
      if (cameraFolderPath) {
        sourcePath = `${sourcePath}/${cameraFolderPath}`;
      }

      try {
        const channel = new Channel<CopyProgressEvent>();
        channel.onmessage = (progress) => {
          if (this.copyStates[readerId]) {
            this.copyStates[readerId].progress = progress;
          }
        };

        const copiedCount = await invoke<number>("copy_files_to_destination", {
          sourcePath,
          destinationPath,
          onProgress: channel,
          renamePrefix: renamePrefix || null,
          autoRename: autoRename || false,
        });

        this.copyStates[readerId].lastResult = { count: copiedCount, success: true };
        return copiedCount;

      } catch (error) {
        const msg = String(error);
        if (this.copyStates[readerId]) {
            this.copyStates[readerId].error = msg;
            this.copyStates[readerId].lastResult = { count: 0, success: false };
        }
        throw error;
      } finally {
        if (this.copyStates[readerId]) {
            this.copyStates[readerId].isCopying = false;
        }
      }
    },

    clearSelection() {
      this.selectedVolume = null;
      this.selectedReader = null;
      this.directories = [];
    },
    
    resetCopyState(readerId: string) {
        if (this.copyStates[readerId]) {
            delete this.copyStates[readerId];
        }
    }
  },
});