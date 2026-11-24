import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";
import type {
  CreateEventParams,
  EventPreset,
  EventSummary,
  LanServerStatus,
  LibraryEventEntry,
  LibraryOverviewResponse,
  ProcessingMetrics,
  ScanResult,
} from "@/types";

export const useEventStore = defineStore("event", {
  state: () => ({
    presets: [] as EventPreset[],
    library: [] as LibraryEventEntry[],
    libraryOverview: null as LibraryOverviewResponse | null,
    machineStationCode: "A",
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
        this.loadMachineStationCode(),
        this.refreshLibrary(),
        this.refreshActiveEvent(),
        this.refreshLanStatus(),
        this.refreshProcessingMetrics(),
      ]);
    },

    async loadMachineStationCode() {
      const saved = await invoke<string | null>("get_app_setting", {
        key: "default_station_code",
      });
      const clean = (saved || "A").trim().toUpperCase();
      this.machineStationCode = clean || "A";
    },

    async setMachineStationCode(code: string) {
      const clean = code.trim().toUpperCase().replace(/[^A-Z0-9]/g, "").slice(0, 4);
      if (!clean) return;
      await invoke("set_app_setting", {
        key: "default_station_code",
        value: clean,
      });
      this.machineStationCode = clean;
    },

    async refreshProcessingMetrics() {
      this.processingMetrics = await invoke<ProcessingMetrics>("get_processing_metrics");
    },

    async loadPresets() {
      this.presets = await invoke<EventPreset[]>("list_presets");
    },

    async refreshLibrary() {
      const [entries, overview] = await Promise.all([
        invoke<LibraryEventEntry[]>("list_library_events"),
        invoke<LibraryOverviewResponse>("get_library_overview"),
      ]);
      this.library = entries;
      this.libraryOverview = overview;
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
