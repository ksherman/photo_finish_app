use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::ipc::Channel;
use tauri::State;

use photofinish_core::avatars::AvatarMetrics;
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
    self, BatchReassignResult, FolderReassignmentInput, ImportRosterResult, IngestBatchSummary,
    ListFoldersResponse, ParsedRosterFile, RemoveFolderMode, RemoveFolderResult, RosterEntryInput,
};
use photofinish_core::server::{detect_lan_ip, EmbeddedServer, LanServerStatus};
use photofinish_core::watcher::WatcherStatus;

use crate::AppState;

// =========================================================================
// Internal Helpers
// =========================================================================

fn require_event(state: &State<'_, AppState>) -> Result<Arc<EventBundle>, String> {
    state
        .active_event
        .get()
        .ok_or_else(|| "No active event open".to_string())
}

async fn run_blocking<T, F>(f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> photofinish_core::Result<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || f().map_err(|e| e.to_string()))
        .await
        .map_err(|e| e.to_string())?
}

async fn with_event_blocking<T, F>(state: &State<'_, AppState>, f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(Arc<EventBundle>) -> photofinish_core::Result<T> + Send + 'static,
{
    let bundle = require_event(state)?;
    run_blocking(move || f(bundle)).await
}

fn err_str<T>(res: photofinish_core::Result<T>) -> Result<T, String> {
    res.map_err(|e| e.to_string())
}

// =========================================================================
// Events & Library
// =========================================================================

#[tauri::command]
pub fn list_presets() -> Vec<EventPreset> {
    preset::builtin_presets()
}

#[tauri::command]
pub fn remove_library_event(state: State<'_, AppState>, root_path: String) -> Result<(), String> {
    err_str(state.app_db.remove_library_event(&root_path))
}

#[tauri::command]
pub fn get_active_event(state: State<'_, AppState>) -> Result<Option<EventSummary>, String> {
    state
        .active_event
        .get()
        .map(|b| err_str(b.summary()))
        .transpose()
}

#[tauri::command]
pub fn create_event(
    state: State<'_, AppState>,
    params: CreateEventParams,
) -> Result<EventSummary, String> {
    let bundle = Arc::new(err_str(EventBundle::create(params))?);
    let summary = err_str(bundle.summary())?;
    err_str(
        state
            .app_db
            .upsert_library_event(bundle.root(), &summary.id, &summary.name),
    )?;
    state.active_event.set(Some(Arc::clone(&bundle)));
    state.watch_active_bundle(bundle);
    Ok(summary)
}

#[tauri::command]
pub fn open_event(state: State<'_, AppState>, root_path: String) -> Result<EventSummary, String> {
    let bundle = Arc::new(err_str(EventBundle::open(&root_path))?);
    let summary = err_str(bundle.summary())?;
    err_str(
        state
            .app_db
            .upsert_library_event(bundle.root(), &summary.id, &summary.name),
    )?;

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

impl LibraryBundleOverview {
    fn stub(entry: LibraryEventEntry, inspect_error: Option<String>) -> Self {
        Self {
            root_path: entry.root_path,
            event_id: entry.event_id,
            name: entry.name,
            last_opened_at: entry.last_opened_at,
            exists_on_disk: entry.exists_on_disk,
            order_code: None,
            station_code: None,
            photo_counts: None,
            subject_count: 0,
            order_count: 0,
            paid_order_count: 0,
            gross_paid_cents: 0,
            pending_fulfillment_count: 0,
            inspect_error,
        }
    }
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
    run_blocking(move || {
        let entries = app_db.list_library_events()?;
        let mut events = Vec::with_capacity(entries.len());
        let mut totals = LibraryTotals {
            total_events: entries.len(),
            ..Default::default()
        };
        let mut seen_orders: HashMap<String, usize> = HashMap::new();

        for entry in entries {
            if !entry.exists_on_disk {
                events.push(LibraryBundleOverview::stub(entry, None));
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
                    events.push(LibraryBundleOverview::stub(entry, Some(err.to_string())));
                }
            }
        }

        let mut duplicate_order_numbers: Vec<String> = seen_orders
            .into_iter()
            .filter_map(|(num, count)| (count > 1).then_some(num))
            .collect();
        duplicate_order_numbers.sort();

        Ok(LibraryOverviewResponse {
            events,
            totals,
            duplicate_order_numbers,
        })
    })
    .await
}

#[tauri::command]
pub async fn gather_events(
    state: State<'_, AppState>,
    parent_path: String,
) -> Result<Vec<EventSummary>, String> {
    let app_db = Arc::clone(&state.app_db);
    run_blocking(move || {
        let discovered = EventBundle::discover_in_directory(&parent_path)?;
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
}

#[tauri::command]
pub async fn scan_active_event(state: State<'_, AppState>) -> Result<ScanResult, String> {
    let bundle = require_event(&state)?;
    let b = Arc::clone(&bundle);
    let result = run_blocking(move || b.scan_directory()).await?;
    if !result.pending_jobs.is_empty() {
        state.queue_photo_jobs(&bundle, result.pending_jobs.clone());
    }
    Ok(result)
}

// =========================================================================
// Watcher, Imaging & Avatars
// =========================================================================

#[tauri::command]
pub fn get_watcher_status(state: State<'_, AppState>) -> WatcherStatus {
    state.watcher.lock().unwrap().status()
}

#[tauri::command]
pub fn set_auto_watch_enabled(
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<WatcherStatus, String> {
    err_str(
        state
            .app_db
            .set_setting("auto_watch_enabled", if enabled { "true" } else { "false" }),
    )?;
    let active = state.active_event.get();
    err_str(
        state
            .watcher
            .lock()
            .unwrap()
            .set_enabled(enabled, active, &state.image_pipeline),
    )
}

#[tauri::command]
pub fn get_processing_metrics(state: State<'_, AppState>) -> ProcessingMetrics {
    state.image_pipeline.metrics()
}

#[tauri::command]
pub async fn get_avatar_metrics(state: State<'_, AppState>) -> Result<AvatarMetrics, String> {
    let active = state.active_event.get();
    let pipeline = state.avatar_pipeline.clone();
    tauri::async_runtime::spawn_blocking(move || pipeline.metrics(active.as_deref()))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn generate_event_avatars(
    state: State<'_, AppState>,
    force: Option<bool>,
) -> Result<usize, String> {
    let pipeline = state.avatar_pipeline.clone();
    let force_regen = force.unwrap_or(false);
    with_event_blocking(&state, move |b| pipeline.enqueue_event_subjects(&b, force_regen)).await
}

// =========================================================================
// Roster, Folders & Card Naming Queue
// =========================================================================

#[tauri::command]
pub fn parse_roster_files(paths: Vec<String>) -> Result<Vec<ParsedRosterFile>, String> {
    paths
        .into_iter()
        .map(|p| err_str(roster::parse_roster_file(&PathBuf::from(p))))
        .collect()
}

#[tauri::command]
pub fn import_roster(
    state: State<'_, AppState>,
    entries: Vec<RosterEntryInput>,
) -> Result<ImportRosterResult, String> {
    let bundle = require_event(&state)?;
    let res = err_str(bundle.import_subjects(&entries))?;
    if res.auto_matched_photos > 0 {
        let _ = state.avatar_pipeline.enqueue_event_subjects(&bundle, false);
    }
    Ok(res)
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
    err_str(require_event(&state)?.add_subject(
        number.as_deref(),
        &first_name,
        last_name.as_deref(),
        group_label.as_deref(),
        team_name.as_deref(),
    ))
}

#[tauri::command]
pub fn list_subject_groups(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    err_str(require_event(&state)?.list_subject_groups())
}

#[tauri::command]
pub async fn list_subjects(
    state: State<'_, AppState>,
    query: Option<String>,
    group_label: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<SubjectRecord>, String> {
    with_event_blocking(&state, move |b| {
        b.list_subjects(query.as_deref(), group_label.as_deref(), limit.unwrap_or(500))
    })
    .await
}

#[tauri::command]
pub fn auto_match_photos(state: State<'_, AppState>) -> Result<usize, String> {
    let bundle = require_event(&state)?;
    let matched = err_str(bundle.auto_match_unassigned_photos())?;
    if matched > 0 {
        let _ = state.avatar_pipeline.enqueue_event_subjects(&bundle, false);
    }
    Ok(matched)
}

#[tauri::command]
pub async fn list_ingest_batches(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<IngestBatchSummary>, String> {
    with_event_blocking(&state, move |b| b.list_ingest_batches(limit.unwrap_or(40))).await
}

#[tauri::command]
pub async fn list_folders(
    state: State<'_, AppState>,
    parent_filter: Option<String>,
    only_unassigned: bool,
    context_parent: Option<String>,
    batch_id: Option<String>,
) -> Result<ListFoldersResponse, String> {
    with_event_blocking(&state, move |b| {
        b.list_folders(
            parent_filter.as_deref(),
            only_unassigned,
            context_parent.as_deref(),
            batch_id.as_deref(),
        )
    })
    .await
}

#[tauri::command]
pub async fn apply_folder_reassignments(
    state: State<'_, AppState>,
    changes: Vec<FolderReassignmentInput>,
    rename_on_disk: bool,
) -> Result<BatchReassignResult, String> {
    let bundle = require_event(&state)?;
    let affected_subjects: Vec<String> = changes
        .iter()
        .filter_map(|c| c.target_subject_id.clone())
        .collect();
    let pause_guard = state.pause_watcher();
    let b = Arc::clone(&bundle);
    let res = run_blocking(move || {
        let _pause = pause_guard;
        b.apply_folder_reassignments(&changes, rename_on_disk)
    })
    .await?;

    if !affected_subjects.is_empty() {
        state
            .avatar_pipeline
            .enqueue_subjects(&bundle, affected_subjects, false);
    }
    Ok(res)
}

#[tauri::command]
pub async fn remove_folder(
    state: State<'_, AppState>,
    folder_path: String,
    mode: RemoveFolderMode,
) -> Result<RemoveFolderResult, String> {
    let pause_guard = state.pause_watcher();
    with_event_blocking(&state, move |b| {
        let _pause = pause_guard;
        b.remove_folder(&folder_path, mode)
    })
    .await
}

// =========================================================================
// Embedded LAN Server
// =========================================================================

#[tauri::command]
pub async fn get_lan_server_status(state: State<'_, AppState>) -> Result<LanServerStatus, String> {
    let guard = state.lan_server.lock().await;
    if let Some(ref server) = *guard {
        Ok(server.status())
    } else {
        let port = state
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
    let _ = state.app_db.set_setting("lan_port", &target_port.to_string());
    let server = err_str(EmbeddedServer::start(target_port, state.active_event.clone()).await)?;
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

// =========================================================================
// Card Readers, App Settings & Card Ingest
// =========================================================================

#[tauri::command]
pub async fn discover_card_readers() -> Result<Vec<CardReaderInfo>, String> {
    run_blocking(hardware::discover_card_readers).await
}

#[tauri::command]
pub fn list_directory(
    path: String,
    auto_enter_dcim: Option<bool>,
) -> Result<Vec<DirectoryEntry>, String> {
    let target = if auto_enter_dcim.unwrap_or(false) {
        hardware::resolve_photo_source_dir(&PathBuf::from(path))
    } else {
        PathBuf::from(path)
    };
    err_str(hardware::list_directory(&target))
}

#[tauri::command]
pub fn list_reader_configs(state: State<'_, AppState>) -> Result<Vec<ReaderConfig>, String> {
    err_str(state.app_db.list_reader_configs())
}

#[tauri::command]
pub fn save_reader_config(
    state: State<'_, AppState>,
    config: ReaderConfig,
) -> Result<(), String> {
    err_str(state.app_db.save_reader_config(&config))
}

#[tauri::command]
pub fn clear_reader_configs(state: State<'_, AppState>) -> Result<(), String> {
    err_str(state.app_db.clear_reader_configs())
}

#[tauri::command]
pub fn save_reader_layout(
    state: State<'_, AppState>,
    updates: Vec<ReaderLayoutUpdate>,
) -> Result<(), String> {
    err_str(state.app_db.save_reader_layout(&updates))
}

#[tauri::command]
pub fn delete_reader_config(state: State<'_, AppState>, reader_id: String) -> Result<(), String> {
    err_str(state.app_db.delete_reader_config(&reader_id))
}

#[tauri::command]
pub fn get_app_setting(state: State<'_, AppState>, key: String) -> Result<Option<String>, String> {
    err_str(state.app_db.get_setting(&key))
}

#[tauri::command]
pub fn set_app_setting(
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> Result<(), String> {
    err_str(state.app_db.set_setting(&key, &value))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IngestRequest {
    pub reader_id: String,
    pub reader_name: String,
    pub volume_name: String,
    pub photographer: String,
    pub source_path: String,
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
        rename_prefix: request.rename_prefix,
        auto_rename: request.auto_rename,
        file_rename_prefix: request.file_rename_prefix,
        included_folders: request.included_folders,
    };
    let meta = IngestBatchMeta {
        reader_id: Some(request.reader_id.clone()),
        reader_name: Some(request.reader_name),
        volume_name: Some(request.volume_name),
        photographer: Some(request.photographer),
        source_path: Some(request.source_path),
        destination_rel_path: request.destination_rel_path,
        tags: request.tags,
    };

    let pause_guard = state.pause_watcher();
    let b = Arc::clone(&bundle);
    let (batch_id, copied_count, pending_jobs) = run_blocking(move || {
        let _pause = pause_guard;
        let files = hardware::copy_files_into_event(&copy_opts, |ev| {
            let _ = on_progress.send(ev);
        })?;
        let copied_count = files.len();
        let (batch_id, pending) = b.record_ingest_batch(meta, &files)?;
        Ok((batch_id, copied_count, pending))
    })
    .await?;

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
    with_event_blocking(&state, |b| b.list_orders()).await
}

#[tauri::command]
pub async fn update_order_payment(
    state: State<'_, AppState>,
    request: UpdatePaymentRequest,
) -> Result<OrderRecord, String> {
    with_event_blocking(&state, move |b| b.update_order_payment(&request)).await
}

#[tauri::command]
pub async fn update_order_notes(
    state: State<'_, AppState>,
    order_id: String,
    notes: Option<String>,
) -> Result<OrderRecord, String> {
    with_event_blocking(&state, move |b| b.update_order_notes(&order_id, notes.as_deref())).await
}

#[tauri::command]
pub async fn update_order_item_fulfillment(
    state: State<'_, AppState>,
    order_id: String,
    item_id: String,
    fulfillment_status: String,
) -> Result<OrderRecord, String> {
    with_event_blocking(&state, move |b| {
        b.update_order_item_fulfillment(&order_id, &item_id, &fulfillment_status)
    })
    .await
}

#[tauri::command]
pub fn list_event_products(state: State<'_, AppState>) -> Result<Vec<ProductRecord>, String> {
    err_str(require_event(&state)?.list_products(true))
}

#[tauri::command]
pub fn upsert_event_product(
    state: State<'_, AppState>,
    input: ProductUpsertInput,
) -> Result<ProductRecord, String> {
    err_str(require_event(&state)?.upsert_product(&input))
}

#[tauri::command]
pub fn update_event_order_settings(
    state: State<'_, AppState>,
    input: EventOrderSettingsInput,
) -> Result<EventSummary, String> {
    err_str(require_event(&state)?.update_event_order_settings(&input))
}

#[tauri::command]
pub async fn discover_usb_drives() -> Result<Vec<UsbDriveInfo>, String> {
    run_blocking(hardware::discover_usb_drives).await
}

#[tauri::command]
pub async fn eject_usb_drive(disk_id: String) -> Result<(), String> {
    run_blocking(move || hardware::eject_drive(&disk_id)).await
}

#[tauri::command]
pub async fn get_order_usb_plan(
    state: State<'_, AppState>,
    order_id: String,
) -> Result<UsbOrderPlan, String> {
    with_event_blocking(&state, move |b| {
        b.resolve_order_usb_copy_plan(&order_id).map(|(plan, _)| plan)
    })
    .await
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
    with_event_blocking(&state, move |bundle| {
        let (_plan, copy_items) = bundle.resolve_order_usb_copy_plan(&request.order_id)?;
        if copy_items.is_empty() {
            return Err(photofinish_core::Error::InvalidInput(
                "No ready digital photos found to copy for this order".into(),
            ));
        }

        let dest_path = PathBuf::from(&request.destination_dir);
        let (files_copied, bytes_copied, elapsed_secs) =
            hardware::copy_order_files_to_usb(&dest_path, &copy_items, |ev| {
                let _ = on_progress.send(ev);
            })?;

        let items_marked_fulfilled = if request.auto_mark_fulfilled {
            bundle.mark_usb_items_fulfilled(&request.order_id)?
        } else {
            0
        };

        let mut ejected = false;
        let mut eject_warning = None;
        if let Some(disk_id) = request.eject_disk_id.as_deref().filter(|s| !s.trim().is_empty()) {
            match hardware::eject_drive(disk_id) {
                Ok(()) => ejected = true,
                Err(e) => eject_warning = Some(e.to_string()),
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

    run_blocking(move || {
        let mut collected: Vec<(String, Vec<OrderRecord>)> = Vec::new();
        let mut skipped_events: Vec<String> = Vec::new();

        if all_library_events {
            for entry in app_db.list_library_events()? {
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
                    Err(e) => skipped_events.push(format!("{} ({})", entry.name, e)),
                }
            }
        } else {
            let bundle = active
                .ok_or_else(|| photofinish_core::Error::InvalidInput("No active event open".into()))?;
            let summary = bundle.summary()?;
            let res = bundle.list_orders()?;
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
        std::fs::write(&output_path, csv).map_err(|e| {
            photofinish_core::Error::Io(format!("Failed to write CSV to {}: {}", output_path, e))
        })?;

        Ok(CsvExportResult {
            output_path,
            event_count,
            order_count,
            item_row_count,
            skipped_events,
        })
    })
    .await
}
