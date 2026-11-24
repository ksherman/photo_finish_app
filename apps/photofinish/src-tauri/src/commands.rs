use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::ipc::Channel;
use tauri::State;

use photofinish_core::db::{LibraryEventEntry, ReaderConfig, ReaderLayoutUpdate};
use photofinish_core::event::{
    BundleOverview, CreateEventParams, EventBundle, EventSummary, IngestBatchMeta, PhotoCounts,
    ScanResult, SubjectRecord,
};
use photofinish_core::hardware::{
    self, CardReaderInfo, CopyOptions, CopyProgressEvent, DirectoryEntry, UsbCopyProgressEvent,
    UsbCopyResult, UsbDriveInfo,
};
use photofinish_core::imaging::ProcessingMetrics;
use photofinish_core::orders::{
    self, EventOrderSettingsInput, OrderListResponse, OrderRecord, ProductRecord,
    ProductUpsertInput, UpdatePaymentRequest, UsbOrderPlan,
};
use photofinish_core::preset::{self, EventPreset};
use photofinish_core::roster::{
    self, AssignFolderResult, BatchReassignResult, FolderReassignmentInput, ImportRosterResult,
    IngestBatchSummary, ListFoldersResponse, ParsedRosterFile, RemoveFolderMode,
    RemoveFolderResult, RosterEntryInput, UnassignedFoldersResponse,
};
use photofinish_core::server::{detect_lan_ip, EmbeddedServer, LanServerStatus};
use photofinish_core::watcher::WatcherStatus;

use crate::AppState;

#[tauri::command]
pub fn list_presets() -> Vec<EventPreset> {
    preset::builtin_presets()
}

#[tauri::command]
pub fn list_library_events(state: State<'_, AppState>) -> Result<Vec<LibraryEventEntry>, String> {
    state.app_db.list_library_events().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_library_event(state: State<'_, AppState>, root_path: String) -> Result<(), String> {
    state
        .app_db
        .remove_library_event(&root_path)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_active_event(state: State<'_, AppState>) -> Result<Option<EventSummary>, String> {
    match state.active_event.get() {
        Some(bundle) => bundle.summary().map(Some).map_err(|e| e.to_string()),
        None => Ok(None),
    }
}

#[tauri::command]
pub fn create_event(
    state: State<'_, AppState>,
    params: CreateEventParams,
) -> Result<EventSummary, String> {
    let bundle = Arc::new(EventBundle::create(params).map_err(|e| e.to_string())?);
    let summary = bundle.summary().map_err(|e| e.to_string())?;
    state
        .app_db
        .upsert_library_event(bundle.root(), &summary.id, &summary.name)
        .map_err(|e| e.to_string())?;
    state.active_event.set(Some(Arc::clone(&bundle)));
    state.watch_active_bundle(bundle);
    Ok(summary)
}

#[tauri::command]
pub fn open_event(state: State<'_, AppState>, root_path: String) -> Result<EventSummary, String> {
    let bundle = Arc::new(EventBundle::open(&root_path).map_err(|e| e.to_string())?);
    let summary = bundle.summary().map_err(|e| e.to_string())?;
    state
        .app_db
        .upsert_library_event(bundle.root(), &summary.id, &summary.name)
        .map_err(|e| e.to_string())?;

    if let Ok(pending) = bundle.list_unprocessed_photos() {
        if !pending.is_empty() {
            state.queue_photo_jobs(&bundle, pending);
        }
    }

    state.active_event.set(Some(Arc::clone(&bundle)));
    state.watch_active_bundle(bundle);
    Ok(summary)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryBundleOverview {
    pub root_path: String,
    pub event_id: String,
    pub name: String,
    pub last_opened_at: String,
    pub exists_on_disk: bool,
    pub order_code: Option<String>,
    pub station_code: Option<String>,
    pub photo_counts: Option<PhotoCounts>,
    pub subject_count: i64,
    pub order_count: i64,
    pub paid_order_count: i64,
    pub gross_paid_cents: i64,
    pub pending_fulfillment_count: i64,
    pub inspect_error: Option<String>,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryTotals {
    pub total_events: usize,
    pub online_events: usize,
    pub total_photos: i64,
    pub ready_photos: i64,
    pub total_subjects: i64,
    pub total_orders: i64,
    pub paid_orders: i64,
    pub gross_paid_cents: i64,
    pub pending_fulfillment_orders: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryOverviewResponse {
    pub events: Vec<LibraryBundleOverview>,
    pub totals: LibraryTotals,
    pub duplicate_order_numbers: Vec<String>,
}

#[tauri::command]
pub async fn get_library_overview(
    state: State<'_, AppState>,
) -> Result<LibraryOverviewResponse, String> {
    let app_db = Arc::clone(&state.app_db);
    tauri::async_runtime::spawn_blocking(move || -> Result<LibraryOverviewResponse, String> {
        let entries = app_db.list_library_events().map_err(|e| e.to_string())?;
        let mut events = Vec::with_capacity(entries.len());
        let mut totals = LibraryTotals {
            total_events: entries.len(),
            ..Default::default()
        };
        let mut seen_orders: HashMap<String, usize> = HashMap::new();

        for entry in entries {
            if !entry.exists_on_disk {
                events.push(LibraryBundleOverview {
                    root_path: entry.root_path,
                    event_id: entry.event_id,
                    name: entry.name,
                    last_opened_at: entry.last_opened_at,
                    exists_on_disk: false,
                    order_code: None,
                    station_code: None,
                    photo_counts: None,
                    subject_count: 0,
                    order_count: 0,
                    paid_order_count: 0,
                    gross_paid_cents: 0,
                    pending_fulfillment_count: 0,
                    inspect_error: None,
                });
                continue;
            }

            match EventBundle::inspect_readonly(&entry.root_path) {
                Ok(BundleOverview {
                    event_id,
                    name,
                    order_code,
                    station_code,
                    photo_counts,
                    subject_count,
                    order_count,
                    paid_order_count,
                    gross_paid_cents,
                    pending_fulfillment_count,
                    order_numbers,
                    ..
                }) => {
                    totals.online_events += 1;
                    totals.total_photos += photo_counts.total;
                    totals.ready_photos += photo_counts.ready;
                    totals.total_subjects += subject_count;
                    totals.total_orders += order_count;
                    totals.paid_orders += paid_order_count;
                    totals.gross_paid_cents += gross_paid_cents;
                    totals.pending_fulfillment_orders += pending_fulfillment_count;

                    for num in order_numbers {
                        *seen_orders.entry(num).or_insert(0) += 1;
                    }

                    events.push(LibraryBundleOverview {
                        root_path: entry.root_path,
                        event_id,
                        name,
                        last_opened_at: entry.last_opened_at,
                        exists_on_disk: true,
                        order_code: Some(order_code),
                        station_code: Some(station_code),
                        photo_counts: Some(photo_counts),
                        subject_count,
                        order_count,
                        paid_order_count,
                        gross_paid_cents,
                        pending_fulfillment_count,
                        inspect_error: None,
                    });
                }
                Err(err) => {
                    events.push(LibraryBundleOverview {
                        root_path: entry.root_path,
                        event_id: entry.event_id,
                        name: entry.name,
                        last_opened_at: entry.last_opened_at,
                        exists_on_disk: true,
                        order_code: None,
                        station_code: None,
                        photo_counts: None,
                        subject_count: 0,
                        order_count: 0,
                        paid_order_count: 0,
                        gross_paid_cents: 0,
                        pending_fulfillment_count: 0,
                        inspect_error: Some(err.to_string()),
                    });
                }
            }
        }

        let mut duplicate_order_numbers: Vec<String> = seen_orders
            .into_iter()
            .filter_map(|(num, count)| if count > 1 { Some(num) } else { None })
            .collect();
        duplicate_order_numbers.sort();

        Ok(LibraryOverviewResponse {
            events,
            totals,
            duplicate_order_numbers,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn gather_events(
    state: State<'_, AppState>,
    parent_path: String,
) -> Result<Vec<EventSummary>, String> {
    let app_db = Arc::clone(&state.app_db);
    tauri::async_runtime::spawn_blocking(move || -> Result<Vec<EventSummary>, String> {
        let discovered =
            EventBundle::discover_in_directory(&parent_path).map_err(|e| e.to_string())?;
        for summary in &discovered {
            let _ = app_db.upsert_library_event(
                &PathBuf::from(&summary.root_path),
                &summary.id,
                &summary.name,
            );
        }
        Ok(discovered)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn scan_active_event(state: State<'_, AppState>) -> Result<ScanResult, String> {
    let bundle = state
        .active_event
        .get()
        .ok_or_else(|| "No active event open".to_string())?;

    let bundle_clone = Arc::clone(&bundle);
    let result = tauri::async_runtime::spawn_blocking(move || bundle_clone.scan_directory())
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;

    if !result.pending_jobs.is_empty() {
        state.queue_photo_jobs(&bundle, result.pending_jobs.clone());
    }

    Ok(result)
}

#[tauri::command]
pub fn get_watcher_status(state: State<'_, AppState>) -> WatcherStatus {
    state.watcher.lock().unwrap().status()
}

#[tauri::command]
pub fn set_auto_watch_enabled(
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<WatcherStatus, String> {
    state
        .app_db
        .set_setting("auto_watch_enabled", if enabled { "true" } else { "false" })
        .map_err(|e| e.to_string())?;
    let active = state.active_event.get();
    state
        .watcher
        .lock()
        .unwrap()
        .set_enabled(enabled, active, &state.image_pipeline)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_processing_metrics(state: State<'_, AppState>) -> ProcessingMetrics {
    state.image_pipeline.metrics()
}

#[tauri::command]
pub fn parse_roster_files(paths: Vec<String>) -> Result<Vec<ParsedRosterFile>, String> {
    let mut parsed = Vec::with_capacity(paths.len());
    for p in paths {
        parsed.push(roster::parse_roster_file(&PathBuf::from(p)).map_err(|e| e.to_string())?);
    }
    Ok(parsed)
}

#[tauri::command]
pub fn import_roster(
    state: State<'_, AppState>,
    entries: Vec<RosterEntryInput>,
) -> Result<ImportRosterResult, String> {
    let bundle = state
        .active_event
        .get()
        .ok_or_else(|| "No active event open".to_string())?;
    bundle.import_subjects(&entries).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_subject(
    state: State<'_, AppState>,
    number: Option<String>,
    first_name: String,
    last_name: Option<String>,
    group_label: Option<String>,
    team_name: Option<String>,
) -> Result<SubjectRecord, String> {
    let bundle = state
        .active_event
        .get()
        .ok_or_else(|| "No active event open".to_string())?;
    bundle
        .add_subject(
            number.as_deref(),
            &first_name,
            last_name.as_deref(),
            group_label.as_deref(),
            team_name.as_deref(),
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_subject_groups(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let bundle = state
        .active_event
        .get()
        .ok_or_else(|| "No active event open".to_string())?;
    bundle.list_subject_groups().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_subjects(
    state: State<'_, AppState>,
    query: Option<String>,
    group_label: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<SubjectRecord>, String> {
    let bundle = state
        .active_event
        .get()
        .ok_or_else(|| "No active event open".to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        bundle
            .list_subjects(query.as_deref(), group_label.as_deref(), limit.unwrap_or(500))
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn auto_match_photos(state: State<'_, AppState>) -> Result<usize, String> {
    let bundle = state
        .active_event
        .get()
        .ok_or_else(|| "No active event open".to_string())?;
    bundle
        .auto_match_unassigned_photos()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_unassigned_folders(
    state: State<'_, AppState>,
    parent_filter: Option<String>,
) -> Result<UnassignedFoldersResponse, String> {
    let bundle = state
        .active_event
        .get()
        .ok_or_else(|| "No active event open".to_string())?;
    bundle
        .list_unassigned_folders(parent_filter.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn assign_folder_to_subject(
    state: State<'_, AppState>,
    folder_path: String,
    subject_id: String,
    rename_on_disk: bool,
) -> Result<AssignFolderResult, String> {
    let bundle = state
        .active_event
        .get()
        .ok_or_else(|| "No active event open".to_string())?;
    let _pause = state.pause_watcher();
    bundle
        .assign_folder_to_subject(&folder_path, &subject_id, rename_on_disk)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_ingest_batches(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<IngestBatchSummary>, String> {
    let bundle = state
        .active_event
        .get()
        .ok_or_else(|| "No active event open".to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        bundle
            .list_ingest_batches(limit.unwrap_or(40))
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn list_folders(
    state: State<'_, AppState>,
    parent_filter: Option<String>,
    only_unassigned: bool,
    context_parent: Option<String>,
    batch_id: Option<String>,
) -> Result<ListFoldersResponse, String> {
    let bundle = state
        .active_event
        .get()
        .ok_or_else(|| "No active event open".to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        bundle
            .list_folders(
                parent_filter.as_deref(),
                only_unassigned,
                context_parent.as_deref(),
                batch_id.as_deref(),
            )
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn apply_folder_reassignments(
    state: State<'_, AppState>,
    changes: Vec<FolderReassignmentInput>,
    rename_on_disk: bool,
) -> Result<BatchReassignResult, String> {
    let bundle = state
        .active_event
        .get()
        .ok_or_else(|| "No active event open".to_string())?;
    let pause_guard = state.pause_watcher();
    tauri::async_runtime::spawn_blocking(move || {
        let _pause = pause_guard;
        bundle
            .apply_folder_reassignments(&changes, rename_on_disk)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn remove_folder(
    state: State<'_, AppState>,
    folder_path: String,
    mode: RemoveFolderMode,
) -> Result<RemoveFolderResult, String> {
    let bundle = state
        .active_event
        .get()
        .ok_or_else(|| "No active event open".to_string())?;
    let pause_guard = state.pause_watcher();
    tauri::async_runtime::spawn_blocking(move || {
        let _pause = pause_guard;
        bundle
            .remove_folder(&folder_path, mode)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_lan_server_status(state: State<'_, AppState>) -> Result<LanServerStatus, String> {
    let guard = state.lan_server.lock().await;
    if let Some(ref server) = *guard {
        Ok(server.status())
    } else {
        let port: u16 = state
            .app_db
            .get_setting("lan_port")
            .ok()
            .flatten()
            .and_then(|s| s.parse().ok())
            .unwrap_or(4000);
        Ok(LanServerStatus {
            running: false,
            port,
            lan_ip: detect_lan_ip(),
            lan_url: None,
        })
    }
}

#[tauri::command]
pub async fn start_lan_server(
    state: State<'_, AppState>,
    port: Option<u16>,
) -> Result<LanServerStatus, String> {
    let mut guard = state.lan_server.lock().await;
    if let Some(ref server) = *guard {
        return Ok(server.status());
    }
    let target_port = port.unwrap_or(4000);
    let _ = state
        .app_db
        .set_setting("lan_port", &target_port.to_string());
    let server = EmbeddedServer::start(target_port, state.active_event.clone())
        .await
        .map_err(|e| e.to_string())?;
    let status = server.status();
    *guard = Some(server);
    Ok(status)
}

#[tauri::command]
pub async fn stop_lan_server(state: State<'_, AppState>) -> Result<LanServerStatus, String> {
    let mut guard = state.lan_server.lock().await;
    let port = guard.as_ref().map(|s| s.port()).unwrap_or(4000);
    if let Some(mut server) = guard.take() {
        server.stop();
    }
    Ok(LanServerStatus {
        running: false,
        port,
        lan_ip: detect_lan_ip(),
        lan_url: None,
    })
}

#[tauri::command]
pub async fn discover_card_readers() -> Result<Vec<CardReaderInfo>, String> {
    tauri::async_runtime::spawn_blocking(hardware::discover_card_readers)
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_directory(path: String) -> Result<Vec<DirectoryEntry>, String> {
    hardware::list_directory(&PathBuf::from(path)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_reader_configs(state: State<'_, AppState>) -> Result<Vec<ReaderConfig>, String> {
    state.app_db.list_reader_configs().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_reader_config(
    state: State<'_, AppState>,
    config: ReaderConfig,
) -> Result<(), String> {
    state
        .app_db
        .save_reader_config(&config)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn clear_reader_configs(state: State<'_, AppState>) -> Result<(), String> {
    state
        .app_db
        .clear_reader_configs()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_reader_layout(
    state: State<'_, AppState>,
    updates: Vec<ReaderLayoutUpdate>,
) -> Result<(), String> {
    state
        .app_db
        .save_reader_layout(&updates)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_reader_config(
    state: State<'_, AppState>,
    reader_id: String,
) -> Result<(), String> {
    state
        .app_db
        .delete_reader_config(&reader_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_app_setting(state: State<'_, AppState>, key: String) -> Result<Option<String>, String> {
    state.app_db.get_setting(&key).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_app_setting(
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> Result<(), String> {
    state
        .app_db
        .set_setting(&key, &value)
        .map_err(|e| e.to_string())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IngestRequest {
    pub reader_id: String,
    pub reader_name: String,
    pub volume_name: String,
    pub photographer: String,
    pub source_path: String,
    /// Relative destination path inside the active Event Bundle (e.g. "Gym A/Session 1A/Group 1A/Beam").
    pub destination_rel_path: String,
    pub rename_prefix: Option<String>,
    pub auto_rename: bool,
    pub file_rename_prefix: Option<String>,
    pub included_folders: Option<Vec<String>>,
    #[serde(default)]
    pub tags: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IngestResult {
    pub batch_id: String,
    pub copied_count: usize,
    pub queued_count: usize,
}

#[tauri::command]
pub async fn ingest_from_reader(
    state: State<'_, AppState>,
    request: IngestRequest,
    on_progress: Channel<CopyProgressEvent>,
) -> Result<IngestResult, String> {
    let bundle = state
        .active_event
        .get()
        .ok_or_else(|| "Open or create an event before ingesting photos".to_string())?;

    let copy_opts = CopyOptions {
        source_path: PathBuf::from(&request.source_path),
        event_root: bundle.root().to_path_buf(),
        destination_rel_path: request.destination_rel_path.clone(),
        rename_prefix: request.rename_prefix.clone(),
        auto_rename: request.auto_rename,
        file_rename_prefix: request.file_rename_prefix.clone(),
        included_folders: request.included_folders.clone(),
    };

    let bundle_clone = Arc::clone(&bundle);
    let meta = IngestBatchMeta {
        reader_id: Some(request.reader_id.clone()),
        reader_name: Some(request.reader_name.clone()),
        volume_name: Some(request.volume_name.clone()),
        photographer: Some(request.photographer.clone()),
        source_path: Some(request.source_path.clone()),
        destination_rel_path: request.destination_rel_path.clone(),
        tags: request.tags,
    };

    let pause_guard = state.pause_watcher();
    let (batch_id, copied_count, pending_jobs) =
        tauri::async_runtime::spawn_blocking(move || -> Result<_, String> {
            let _pause = pause_guard;
            let files = hardware::copy_files_into_event(&copy_opts, |ev| {
                let _ = on_progress.send(ev);
            })
            .map_err(|e| e.to_string())?;

            let copied_count = files.len();
            let (batch_id, pending) = bundle_clone
                .record_ingest_batch(meta, &files)
                .map_err(|e| e.to_string())?;

            Ok((batch_id, copied_count, pending))
        })
        .await
        .map_err(|e| e.to_string())??;

    let queued_count = pending_jobs.len();
    state.queue_photo_jobs(&bundle, pending_jobs);
    let _ = state.app_db.increment_reader_order(&request.reader_id);

    Ok(IngestResult {
        batch_id,
        copied_count,
        queued_count,
    })
}

// =========================================================================
// Orders, Pricing Catalog, Instant USB-C/A Fulfillment & CSV Export
// =========================================================================

#[tauri::command]
pub async fn list_orders(state: State<'_, AppState>) -> Result<OrderListResponse, String> {
    let bundle = state
        .active_event
        .get()
        .ok_or_else(|| "No active event open".to_string())?;
    tauri::async_runtime::spawn_blocking(move || bundle.list_orders().map_err(|e| e.to_string()))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn update_order_payment(
    state: State<'_, AppState>,
    request: UpdatePaymentRequest,
) -> Result<OrderRecord, String> {
    let bundle = state
        .active_event
        .get()
        .ok_or_else(|| "No active event open".to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        bundle
            .update_order_payment(&request)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn update_order_notes(
    state: State<'_, AppState>,
    order_id: String,
    notes: Option<String>,
) -> Result<OrderRecord, String> {
    let bundle = state
        .active_event
        .get()
        .ok_or_else(|| "No active event open".to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        bundle
            .update_order_notes(&order_id, notes.as_deref())
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn update_order_item_fulfillment(
    state: State<'_, AppState>,
    order_id: String,
    item_id: String,
    fulfillment_status: String,
) -> Result<OrderRecord, String> {
    let bundle = state
        .active_event
        .get()
        .ok_or_else(|| "No active event open".to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        bundle
            .update_order_item_fulfillment(&order_id, &item_id, &fulfillment_status)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn list_event_products(state: State<'_, AppState>) -> Result<Vec<ProductRecord>, String> {
    let bundle = state
        .active_event
        .get()
        .ok_or_else(|| "No active event open".to_string())?;
    bundle.list_products(true).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn upsert_event_product(
    state: State<'_, AppState>,
    input: ProductUpsertInput,
) -> Result<ProductRecord, String> {
    let bundle = state
        .active_event
        .get()
        .ok_or_else(|| "No active event open".to_string())?;
    bundle.upsert_product(&input).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_event_order_settings(
    state: State<'_, AppState>,
    input: EventOrderSettingsInput,
) -> Result<EventSummary, String> {
    let bundle = state
        .active_event
        .get()
        .ok_or_else(|| "No active event open".to_string())?;
    bundle
        .update_event_order_settings(&input)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn discover_usb_drives() -> Result<Vec<UsbDriveInfo>, String> {
    tauri::async_runtime::spawn_blocking(hardware::discover_usb_drives)
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn eject_usb_drive(disk_id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || hardware::eject_drive(&disk_id))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_order_usb_plan(
    state: State<'_, AppState>,
    order_id: String,
) -> Result<UsbOrderPlan, String> {
    let bundle = state
        .active_event
        .get()
        .ok_or_else(|| "No active event open".to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        bundle
            .resolve_order_usb_copy_plan(&order_id)
            .map(|(plan, _)| plan)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyOrderToUsbRequest {
    pub order_id: String,
    pub destination_dir: String,
    pub eject_disk_id: Option<String>,
    pub auto_mark_fulfilled: bool,
}

#[tauri::command]
pub async fn copy_order_to_usb(
    state: State<'_, AppState>,
    request: CopyOrderToUsbRequest,
    on_progress: Channel<UsbCopyProgressEvent>,
) -> Result<UsbCopyResult, String> {
    let bundle = state
        .active_event
        .get()
        .ok_or_else(|| "No active event open".to_string())?;

    tauri::async_runtime::spawn_blocking(move || -> Result<UsbCopyResult, String> {
        let (_plan, copy_items) = bundle
            .resolve_order_usb_copy_plan(&request.order_id)
            .map_err(|e| e.to_string())?;

        if copy_items.is_empty() {
            return Err("No ready digital photos found to copy for this order".to_string());
        }

        let dest_path = PathBuf::from(&request.destination_dir);
        let (files_copied, bytes_copied, elapsed_secs) =
            hardware::copy_order_files_to_usb(&dest_path, &copy_items, |ev| {
                let _ = on_progress.send(ev);
            })
            .map_err(|e| e.to_string())?;

        let items_marked_fulfilled = if request.auto_mark_fulfilled {
            bundle
                .mark_usb_items_fulfilled(&request.order_id)
                .map_err(|e| e.to_string())?
        } else {
            0
        };

        let mut ejected = false;
        let mut eject_warning = None;
        if let Some(ref disk_id) = request.eject_disk_id {
            if !disk_id.trim().is_empty() {
                match hardware::eject_drive(disk_id) {
                    Ok(()) => ejected = true,
                    Err(e) => eject_warning = Some(e.to_string()),
                }
            }
        }

        let mb_per_sec = (bytes_copied as f64 / (1024.0 * 1024.0)) / elapsed_secs.max(0.001);

        Ok(UsbCopyResult {
            destination_dir: request.destination_dir,
            files_copied,
            bytes_copied,
            elapsed_secs,
            mb_per_sec,
            ejected,
            eject_warning,
            items_marked_fulfilled,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CsvExportResult {
    pub output_path: String,
    pub event_count: usize,
    pub order_count: usize,
    pub item_row_count: usize,
    pub skipped_events: Vec<String>,
}

#[tauri::command]
pub async fn export_orders_csv(
    state: State<'_, AppState>,
    output_path: String,
    all_library_events: bool,
) -> Result<CsvExportResult, String> {
    let active = state.active_event.get();
    let app_db = Arc::clone(&state.app_db);

    tauri::async_runtime::spawn_blocking(move || -> Result<CsvExportResult, String> {
        let mut collected: Vec<(String, Vec<OrderRecord>)> = Vec::new();
        let mut skipped_events: Vec<String> = Vec::new();

        if all_library_events {
            let entries = app_db.list_library_events().map_err(|e| e.to_string())?;
            for entry in entries {
                if !entry.exists_on_disk {
                    skipped_events.push(format!("{} (drive offline)", entry.name));
                    continue;
                }
                match EventBundle::open(&entry.root_path) {
                    Ok(bundle) => match (bundle.summary(), bundle.list_orders()) {
                        (Ok(summary), Ok(res)) => collected.push((summary.name, res.orders)),
                        (Err(e), _) | (_, Err(e)) => {
                            skipped_events.push(format!("{} ({})", entry.name, e));
                        }
                    },
                    Err(e) => {
                        skipped_events.push(format!("{} ({})", entry.name, e));
                    }
                }
            }
        } else {
            let bundle = active.ok_or_else(|| "No active event open".to_string())?;
            let summary = bundle.summary().map_err(|e| e.to_string())?;
            let res = bundle.list_orders().map_err(|e| e.to_string())?;
            collected.push((summary.name, res.orders));
        }

        let event_count = collected.len();
        let order_count: usize = collected.iter().map(|(_, o)| o.len()).sum();
        let item_row_count: usize = collected
            .iter()
            .flat_map(|(_, o)| o.iter())
            .map(|o| o.items.len())
            .sum();

        let csv = orders::format_orders_csv(&collected);
        std::fs::write(&output_path, csv)
            .map_err(|e| format!("Failed to write CSV to {}: {}", output_path, e))?;

        Ok(CsvExportResult {
            output_path,
            event_count,
            order_count,
            item_row_count,
            skipped_events,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}
