import { defineStore } from "pinia";

export interface ReaderMapping {
  readerId: string;
  displayName: string;
  destination: string;
  photographer: string;
  currentOrder: number;
  cameraBrand: string;
  cameraFolderPath: string; // e.g., "DCIM" or "" for root
  renamePrefix: string; // e.g., "Gymnast" - used for auto-renaming folders
  autoRename: boolean; // Whether to auto-rename folders during copy
  fileRenamePrefix?: string; // e.g., "Gymnast" - used for renaming individual files
}

export interface SessionState {
  // Reader-to-destination mappings (keyed by reader_id)
  readerMappings: Record<string, ReaderMapping>;
  // Currently active reader
  activeReaderId: string | null;
}

export const useSessionStore = defineStore("session", {
  state: (): SessionState => ({
    readerMappings: {},
    activeReaderId: null,
  }),

  getters: {
    activeMapping(): ReaderMapping | null {
      if (!this.activeReaderId) return null;
      return this.readerMappings[this.activeReaderId] || null;
    },

    destinationPath(): string {
      if (!this.activeReaderId || !this.readerMappings[this.activeReaderId]) return "";
      const mapping = this.readerMappings[this.activeReaderId];
      const orderPadded = String(mapping.currentOrder).padStart(4, "0");
      return `${mapping.destination}/${orderPadded}`;
    },

    isConfigured(): boolean {
      if (!this.activeReaderId || !this.readerMappings[this.activeReaderId]) return false;
      const mapping = this.readerMappings[this.activeReaderId];
      return Boolean(mapping.destination && mapping.photographer);
    },

    currentPhotographer(): string {
      if (this.activeReaderId && this.readerMappings[this.activeReaderId]) {
        return this.readerMappings[this.activeReaderId].photographer;
      }
      return "";
    },

    currentDestination(): string {
      if (this.activeReaderId && this.readerMappings[this.activeReaderId]) {
        return this.readerMappings[this.activeReaderId].destination;
      }
      return "";
    },

    currentOrderNumber(): number {
      if (this.activeReaderId && this.readerMappings[this.activeReaderId]) {
        return this.readerMappings[this.activeReaderId].currentOrder;
      }
      return 1;
    },
  },

  actions: {
    setActiveReader(readerId: string | null) {
      this.activeReaderId = readerId;
    },

    setReaderMapping(
      readerId: string,
      displayName: string,
      destination: string,
      photographer: string,
      cameraBrand: string,
      cameraFolderPath: string,
      renamePrefix: string,
      autoRename: boolean,
      fileRenamePrefix?: string
    ) {
      const existing = this.readerMappings[readerId];
      this.readerMappings[readerId] = {
        readerId,
        displayName,
        destination,
        photographer,
        cameraBrand,
        cameraFolderPath,
        renamePrefix,
        autoRename,
        fileRenamePrefix,
        // Reset order if destination changed, otherwise keep existing
        currentOrder:
          existing && existing.destination === destination
            ? existing.currentOrder
            : 1,
      };
    },

    removeReaderMapping(readerId: string) {
      delete this.readerMappings[readerId];
      if (this.activeReaderId === readerId) {
        this.activeReaderId = null;
      }
    },

    clearAllMappings() {
      this.readerMappings = {};
      this.activeReaderId = null;
    },

    getReaderMapping(readerId: string): ReaderMapping | null {
      return this.readerMappings[readerId] || null;
    },

    incrementOrderForReader(readerId: string) {
      if (this.readerMappings[readerId]) {
        this.readerMappings[readerId].currentOrder++;
      }
    },
  },

  persist: true,
});
