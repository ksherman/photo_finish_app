import { defineStore } from "pinia";
import { invoke, Channel } from "@tauri-apps/api/core";
import type { VolumeInfo, DirectoryEntry, CopyProgressEvent } from "@/types";

export const useCardReaderStore = defineStore("cardReader", {
  state: () => ({
    volumes: [] as VolumeInfo[],
    selectedVolume: null as VolumeInfo | null,
    directories: [] as DirectoryEntry[],
    totalFileCount: 0,
    copyProgress: null as CopyProgressEvent | null,
    isCopying: false,
    copyErrors: [] as string[],
  }),

  actions: {
    async refreshVolumes() {
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

    async selectVolume(volume: VolumeInfo) {
      this.selectedVolume = volume;
      await this.loadDirectories();
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

    async copyFiles(destinationPath: string): Promise<number> {
      if (!this.selectedVolume || this.isCopying) return 0;

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
          sourcePath: this.selectedVolume.path,
          destinationPath,
          onProgress: channel,
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
      this.directories = [];
      this.totalFileCount = 0;
      this.copyProgress = null;
    },
  },
});
