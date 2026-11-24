import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import type { CsvExportResult } from "@/types";

export async function exportOrdersCsvDialog(options: {
  allLibraryEvents: boolean;
  defaultFilename: string;
  title: string;
}): Promise<string | null> {
  const targetPath = await save({
    defaultPath: options.defaultFilename,
    filters: [{ name: "CSV Spreadsheet", extensions: ["csv"] }],
    title: options.title,
  });
  if (!targetPath) return null;

  const res = await invoke<CsvExportResult>("export_orders_csv", {
    outputPath: targetPath,
    allLibraryEvents: options.allLibraryEvents,
  });
  const skippedNote =
    res.skippedEvents.length > 0
      ? ` (Skipped ${res.skippedEvents.length}: ${res.skippedEvents.join(", ")})`
      : "";
  return `Exported ${res.orderCount} order(s) (${res.itemRowCount} line items across ${res.eventCount} event${res.eventCount === 1 ? "" : "s"}) to ${res.outputPath}.${skippedNote}`;
}
