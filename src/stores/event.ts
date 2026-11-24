import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";
import type {
  CreateEventParams,
  EventPreset,
  EventSummary,
  LanServerStatus,
  LibraryOverviewResponse,
  ProcessingMetrics,
  ScanResult,
  WatcherStatus,
} from "@/types";

let syncNoticeTimer: ReturnType<typeof setTimeout> | null = null;

export const useEventStore = defineStore("event", {
  state: () => ({
    presets: [] as EventPreset[],
    libraryOverview: null as LibraryOverviewResponse | null,
    machineStationCode: "A",
    activeEvent: null as EventSummary | null,
    lanStatus: {
      running: false,
      port: 4000,
      lanIp: null,
      lanUrl: null,
    } as LanServerStatus,
    watcherStatus: {
      enabled: true,
      watching: false,
      watchedRoot: null,
      lastSyncAt: null,
      lastAdded: 0,
      lastRemoved: 0,
      totalAdded: 0,
      totalRemoved: 0,
      lastError: null,
    } as WatcherStatus,
    lastSyncNotice: null as string | null,
    fsSyncGeneration: 0,
    pendingBatchToName: null as string | null,
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
        this.refreshWatcherStatus(),
        this.refreshProcessingMetrics(),
      ]);
    },

    async refreshWatcherStatus() {
      this.watcherStatus = await invoke<WatcherStatus>("get_watcher_status");
    },

    async toggleAutoWatch() {
      const next = !this.watcherStatus.enabled;
      this.watcherStatus = await invoke<WatcherStatus>("set_auto_watch_enabled", {
        enabled: next,
      });
    },

    showSyncNotice(message: string, durationMs = 6000) {
      this.lastSyncNotice = message;
      if (syncNoticeTimer) clearTimeout(syncNoticeTimer);
      syncNoticeTimer = setTimeout(() => {
        this.lastSyncNotice = null;
      }, durationMs);
    },

    openBatchForNaming(batchId: string) {
      this.pendingBatchToName = batchId;
    },

    clearPendingBatchToName() {
      this.pendingBatchToName = null;
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
      this.libraryOverview = await invoke<LibraryOverviewResponse>("get_library_overview");
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
      await Promise.all([this.refreshLibrary(), this.refreshWatcherStatus()]);
      return summary;
    },

    async openEvent(rootPath: string): Promise<EventSummary> {
      this.lastError = null;
      const summary = await invoke<EventSummary>("open_event", { rootPath });
      this.activeEvent = summary;
      await Promise.all([this.refreshLibrary(), this.refreshWatcherStatus()]);
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
        const parts: string[] = [];
        if (res.photosNew > 0) parts.push(`+${res.photosNew.toLocaleString()} new`);
        if (res.photosRemoved > 0) parts.push(`${res.photosRemoved.toLocaleString()} removed`);
        if (parts.length > 0) {
          this.fsSyncGeneration += 1;
          this.showSyncNotice(`Scan: ${parts.join(" · ")}`);
        } else {
          this.showSyncNotice("Scan: Up to date", 3500);
        }
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
