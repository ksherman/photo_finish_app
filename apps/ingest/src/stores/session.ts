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
}

export interface SessionState {
  // Legacy single-destination mode
  destination: string;
  photographer: string;
  currentOrder: number;
  // Reader-to-destination mappings (keyed by reader_id)
  readerMappings: Record<string, ReaderMapping>;
  // Currently active reader
  activeReaderId: string | null;
}

export const useSessionStore = defineStore("session", {
  state: (): SessionState => ({
    destination: "",
    photographer: "",
    currentOrder: 1,
    readerMappings: {},
    activeReaderId: null,
  }),

  getters: {
    activeMapping(): ReaderMapping | null {
      if (!this.activeReaderId) return null;
      return this.readerMappings[this.activeReaderId] || null;
    },

    destinationPath(): string {
      // Use active reader mapping if available
      if (this.activeReaderId && this.readerMappings[this.activeReaderId]) {
        const mapping = this.readerMappings[this.activeReaderId];
        const orderPadded = String(mapping.currentOrder).padStart(4, "0");
        return `${mapping.destination}/${orderPadded}`;
      }
      // Fallback to legacy mode
      if (!this.destination) return "";
      const orderPadded = String(this.currentOrder).padStart(4, "0");
      return `${this.destination}/${orderPadded}`;
    },

    isConfigured(): boolean {
      // Check active reader mapping first
      if (this.activeReaderId && this.readerMappings[this.activeReaderId]) {
        const mapping = this.readerMappings[this.activeReaderId];
        return Boolean(mapping.destination && mapping.photographer);
      }
      // Fallback to legacy mode
      return Boolean(this.destination && this.photographer);
    },

    // Get photographer for current context
    currentPhotographer(): string {
      if (this.activeReaderId && this.readerMappings[this.activeReaderId]) {
        return this.readerMappings[this.activeReaderId].photographer;
      }
      return this.photographer;
    },

    // Get destination for current context
    currentDestination(): string {
      if (this.activeReaderId && this.readerMappings[this.activeReaderId]) {
        return this.readerMappings[this.activeReaderId].destination;
      }
      return this.destination;
    },

    // Get order for current context
    currentOrderNumber(): number {
      if (this.activeReaderId && this.readerMappings[this.activeReaderId]) {
        return this.readerMappings[this.activeReaderId].currentOrder;
      }
      return this.currentOrder;
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
      autoRename: boolean
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

    getReaderMapping(readerId: string): ReaderMapping | null {
      return this.readerMappings[readerId] || null;
    },

    incrementOrder() {
      if (this.activeReaderId && this.readerMappings[this.activeReaderId]) {
        this.readerMappings[this.activeReaderId].currentOrder++;
      } else {
        this.currentOrder++;
      }
    },

    incrementOrderForReader(readerId: string) {
      if (this.readerMappings[readerId]) {
        this.readerMappings[readerId].currentOrder++;
      }
    },

    resetOrder() {
      if (this.activeReaderId && this.readerMappings[this.activeReaderId]) {
        this.readerMappings[this.activeReaderId].currentOrder = 1;
      } else {
        this.currentOrder = 1;
      }
    },

    setDestination(path: string) {
      // If destination changed, reset order
      if (path !== this.destination) {
        this.destination = path;
        this.currentOrder = 1;
      }
    },
  },

  persist: true,
});
