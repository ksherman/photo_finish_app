import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";
import type {
  CreateEventParams,
  EventPreset,
  EventSummary,
  LanServerStatus,
  LibraryEventEntry,
  ProcessingMetrics,
  ScanResult,
} from "@/types";

export const useEventStore = defineStore("event", {
  state: () => ({
    presets: [] as EventPreset[],
    library: [] as LibraryEventEntry[],
    activeEvent: null as EventSummary | null,
    lanStatus: {
      running: false,
      port: 4000,
      lanIp: null,
      lanUrl: null,
    } as LanServerStatus,
    processingMetrics: null as ProcessingMetrics | null,
    isScanning: false,
    lastError: null as string | null,
  }),

  actions: {
    async bootstrap() {
      await Promise.all([
        this.loadPresets(),
        this.refreshLibrary(),
        this.refreshActiveEvent(),
        this.refreshLanStatus(),
        this.refreshProcessingMetrics(),
      ]);
    },

    async refreshProcessingMetrics() {
      this.processingMetrics = await invoke<ProcessingMetrics>("get_processing_metrics");
    },

    async loadPresets() {
      this.presets = await invoke<EventPreset[]>("list_presets");
    },

    async refreshLibrary() {
      this.library = await invoke<LibraryEventEntry[]>("list_library_events");
    },

    async refreshActiveEvent() {
      this.activeEvent = await invoke<EventSummary | null>("get_active_event");
    },

    async refreshLanStatus() {
      this.lanStatus = await invoke<LanServerStatus>("get_lan_server_status");
    },

    async createEvent(params: CreateEventParams): Promise<EventSummary> {
      this.lastError = null;
      const summary = await invoke<EventSummary>("create_event", { params });
      this.activeEvent = summary;
      await this.refreshLibrary();
      return summary;
    },

    async openEvent(rootPath: string): Promise<EventSummary> {
      this.lastError = null;
      const summary = await invoke<EventSummary>("open_event", { rootPath });
      this.activeEvent = summary;
      await this.refreshLibrary();
      return summary;
    },

    async gatherEvents(parentPath: string): Promise<EventSummary[]> {
      this.lastError = null;
      const found = await invoke<EventSummary[]>("gather_events", { parentPath });
      await this.refreshLibrary();
      return found;
    },

    async removeLibraryEvent(rootPath: string) {
      await invoke("remove_library_event", { rootPath });
      await this.refreshLibrary();
    },

    async scanActiveEvent(): Promise<ScanResult | null> {
      if (!this.activeEvent || this.isScanning) return null;
      this.isScanning = true;
      try {
        const res = await invoke<ScanResult>("scan_active_event");
        await this.refreshActiveEvent();
        return res;
      } finally {
        this.isScanning = false;
      }
    },

    async toggleLanServer() {
      if (this.lanStatus.running) {
        this.lanStatus = await invoke<LanServerStatus>("stop_lan_server");
      } else {
        this.lanStatus = await invoke<LanServerStatus>("start_lan_server", {
          port: this.lanStatus.port || 4000,
        });
      }
    },
  },
});
