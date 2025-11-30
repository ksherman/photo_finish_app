import { defineStore } from "pinia";
import { invoke, Channel } from "@tauri-apps/api/core";
import type {
  VolumeInfo,
  DirectoryEntry,
  CopyProgressEvent,
  CardReaderInfo,
} from "@/types";

export const useCardReaderStore = defineStore("cardReader", {
  state: () => ({
    volumes: [] as VolumeInfo[],
    cardReaders: [] as CardReaderInfo[],
    selectedVolume: null as VolumeInfo | null,
    selectedReader: null as CardReaderInfo | null,
    directories: [] as DirectoryEntry[],
    totalFileCount: 0,
    copyProgress: null as CopyProgressEvent | null,
    isCopying: false,
    copyErrors: [] as string[],
  }),

  actions: {
    async discoverReaders() {
      try {
        this.cardReaders = await invoke<CardReaderInfo[]>(
          "discover_card_readers"
        );

        // Auto-select if only one reader
        if (this.cardReaders.length === 1 && !this.selectedReader) {
          await this.selectReader(this.cardReaders[0]);
        }

        // Update file counts if reader is selected
        if (this.selectedReader) {
          const updated = this.cardReaders.find(
            (r) => r.reader_id === this.selectedReader?.reader_id
          );
          if (updated) {
            this.selectedReader = updated;
            this.totalFileCount = updated.file_count;
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
      // Keep for backwards compatibility, but prefer discoverReaders
      try {
        this.volumes = await invoke<VolumeInfo[]>("list_volumes");

        // Auto-select removable volume if only one
        const removable = this.volumes.filter((v) => v.is_removable);
        if (removable.length === 1 && !this.selectedVolume) {
          await this.selectVolume(removable[0]);
        }
      } catch (error) {
        console.error("Failed to list volumes:", error);
      }
    },

    async selectReader(reader: CardReaderInfo, cameraFolderPath?: string) {
      this.selectedReader = reader;
      this.totalFileCount = reader.file_count;
      await this.loadDirectoriesFromReader(cameraFolderPath);
    },

    async selectVolume(volume: VolumeInfo) {
      this.selectedVolume = volume;
      await this.loadDirectories();
    },

    async loadDirectoriesFromReader(cameraFolderPath?: string) {
      if (!this.selectedReader) return;

      try {
        let basePath = this.selectedReader.mount_point;
        // Append camera folder path if provided (e.g., "DCIM")
        if (cameraFolderPath) {
          basePath = `${basePath}/${cameraFolderPath}`;
        }

        const entries = await invoke<DirectoryEntry[]>("list_directory", {
          path: basePath,
        });

        // Filter to only directories (camera folders)
        this.directories = entries.filter((d) => d.is_directory);
      } catch (error) {
        console.error("Failed to load directories:", error);
      }
    },

    async loadDirectories() {
      if (!this.selectedVolume) return;

      try {
        const entries = await invoke<DirectoryEntry[]>("list_directory", {
          path: this.selectedVolume.path,
        });

        // Filter to only directories (camera folders)
        this.directories = entries.filter((d) => d.is_directory);

        // Calculate total file count
        this.totalFileCount = this.directories.reduce(
          (sum, dir) => sum + dir.file_count,
          0
        );
      } catch (error) {
        console.error("Failed to load directories:", error);
      }
    },

    async copyFiles(
      destinationPath: string,
      cameraFolderPath?: string,
      renamePrefix?: string,
      autoRename?: boolean
    ): Promise<number> {
      let sourcePath =
        this.selectedReader?.mount_point || this.selectedVolume?.path;
      if (!sourcePath || this.isCopying) return 0;

      // Append camera folder path if provided (e.g., "DCIM")
      if (cameraFolderPath) {
        sourcePath = `${sourcePath}/${cameraFolderPath}`;
      }

      this.isCopying = true;
      this.copyErrors = [];
      this.copyProgress = {
        total: 0,
        copied: 0,
        current_file: "",
        percentage: 0,
      };

      try {
        const channel = new Channel<CopyProgressEvent>();
        channel.onmessage = (progress) => {
          this.copyProgress = progress;
        };

        const copiedCount = await invoke<number>("copy_files_to_destination", {
          sourcePath,
          destinationPath,
          onProgress: channel,
          renamePrefix: renamePrefix || null,
          autoRename: autoRename || false,
        });

        return copiedCount;
      } catch (error) {
        this.copyErrors.push(String(error));
        throw error;
      } finally {
        this.isCopying = false;
      }
    },

    clearSelection() {
      this.selectedVolume = null;
      this.selectedReader = null;
      this.directories = [];
      this.totalFileCount = 0;
      this.copyProgress = null;
    },
  },
});
