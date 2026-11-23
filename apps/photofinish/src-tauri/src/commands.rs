use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::ipc::Channel;
use tauri::State;

use photofinish_core::db::{LibraryEventEntry, ReaderConfig};
use photofinish_core::event::{
    CreateEventParams, EventBundle, EventSummary, IngestBatchMeta, ScanResult, SubjectRecord,
};
use photofinish_core::hardware::{
    self, CardReaderInfo, CopyOptions, CopyProgressEvent, DirectoryEntry,
};
use photofinish_core::imaging::ProcessingMetrics;
use photofinish_core::preset::{self, EventPreset};
use photofinish_core::roster::{
    self, AssignFolderResult, ImportRosterResult, ParsedRosterFile, RosterEntryInput,
    UnassignedFoldersResponse,
};
use photofinish_core::server::{detect_lan_ip, EmbeddedServer, LanServerStatus};

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
    state.active_event.set(Some(bundle));
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

    state.active_event.set(Some(bundle));
    Ok(summary)
}

#[tauri::command]
pub fn gather_events(
    state: State<'_, AppState>,
    parent_path: String,
) -> Result<Vec<EventSummary>, String> {
    let discovered =
        EventBundle::discover_in_directory(&parent_path).map_err(|e| e.to_string())?;
    for summary in &discovered {
        let _ = state.app_db.upsert_library_event(
            &PathBuf::from(&summary.root_path),
            &summary.id,
            &summary.name,
        );
    }
    Ok(discovered)
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
pub fn list_subjects(
    state: State<'_, AppState>,
    query: Option<String>,
    group_label: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<SubjectRecord>, String> {
    let bundle = state
        .active_event
        .get()
        .ok_or_else(|| "No active event open".to_string())?;
    bundle
        .list_subjects(query.as_deref(), group_label.as_deref(), limit.unwrap_or(500))
        .map_err(|e| e.to_string())
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
    bundle
        .assign_folder_to_subject(&folder_path, &subject_id, rename_on_disk)
        .map_err(|e| e.to_string())
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

    let (batch_id, copied_count, pending_jobs) =
        tauri::async_runtime::spawn_blocking(move || -> Result<_, String> {
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
