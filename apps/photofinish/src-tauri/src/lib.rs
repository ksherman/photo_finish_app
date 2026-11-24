mod commands;

use std::sync::Arc;
use tauri::{Emitter, Manager};
use tokio::sync::Mutex;

use photofinish_core::avatars::AvatarPipeline;
use photofinish_core::db::AppDb;
use photofinish_core::event::{EventBundle, PendingPhotoJob};
use photofinish_core::imaging::ImagePipeline;
use photofinish_core::server::{EmbeddedServer, SharedActiveEvent};
use photofinish_core::watcher::{EventWatcherManager, WatcherPauseGuard};

pub struct AppState {
    pub app_db: Arc<AppDb>,
    pub active_event: SharedActiveEvent,
    pub lan_server: Mutex<Option<EmbeddedServer>>,
    pub image_pipeline: ImagePipeline,
    pub avatar_pipeline: AvatarPipeline,
    pub watcher: std::sync::Mutex<EventWatcherManager>,
}

impl AppState {
    pub fn queue_photo_jobs(&self, bundle: &Arc<EventBundle>, jobs: Vec<PendingPhotoJob>) {
        self.image_pipeline.enqueue(bundle, jobs);
    }

    pub fn watch_active_bundle(&self, bundle: Arc<EventBundle>) {
        if let Ok(mut mgr) = self.watcher.lock() {
            if let Err(err) = mgr.watch_bundle(bundle, &self.image_pipeline) {
                eprintln!("PhotoFinish: filesystem watcher warning: {}", err);
            }
        }
    }

    pub fn pause_watcher(&self) -> WatcherPauseGuard {
        self.watcher.lock().unwrap().pause()
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app data dir");
            let app_db_path = data_dir.join("photofinish_app.db");

            // Seamlessly migrate existing local reader configs, library bookmarks, and settings
            // from the pre-release `com.photofinish.app` bundle directory on first launch.
            if let Some(legacy_db_path) = data_dir
                .parent()
                .map(|p| p.join("com.photofinish.app").join("photofinish_app.db"))
            {
                if let Err(err) =
                    photofinish_core::db::migrate_legacy_app_db(&legacy_db_path, &app_db_path)
                {
                    eprintln!(
                        "PhotoFinish: warning migrating legacy app database ({}): {}",
                        legacy_db_path.display(),
                        err
                    );
                }
            }

            let app_db = Arc::new(
                AppDb::open(&app_db_path)
                    .expect("failed to open machine-local app database"),
            );
            let active_event = SharedActiveEvent::new(None);
            let image_pipeline = ImagePipeline::start(None);

            // Best-effort auto-start LAN kiosk server on default port 4000
            let default_port: u16 = app_db
                .get_setting("lan_port")
                .ok()
                .flatten()
                .and_then(|s| s.parse().ok())
                .unwrap_or(4000);

            let initial_server = tauri::async_runtime::block_on(async {
                EmbeddedServer::start(default_port, active_event.clone())
                    .await
                    .ok()
            });

            let auto_watch_enabled = app_db
                .get_setting("auto_watch_enabled")
                .ok()
                .flatten()
                .map(|v| v != "false" && v != "0")
                .unwrap_or(true);

            let emit_handle = app.handle().clone();
            let watcher = std::sync::Mutex::new(EventWatcherManager::new(
                auto_watch_enabled,
                Some(Arc::new(move |ev| {
                    let _ = emit_handle.emit("event-fs-synced", &ev);
                })),
            ));

            let avatar_emit = app.handle().clone();
            let avatar_pipeline = AvatarPipeline::start(Some(Arc::new(move |ev| {
                let _ = avatar_emit.emit("avatars-updated", &ev);
            })));

            app.manage(AppState {
                app_db,
                active_event,
                lan_server: Mutex::new(initial_server),
                image_pipeline,
                avatar_pipeline,
                watcher,
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_presets,
            commands::get_library_overview,
            commands::remove_library_event,
            commands::get_active_event,
            commands::create_event,
            commands::open_event,
            commands::gather_events,
            commands::scan_active_event,
            commands::get_watcher_status,
            commands::set_auto_watch_enabled,
            commands::get_processing_metrics,
            commands::get_avatar_metrics,
            commands::generate_event_avatars,
            commands::parse_roster_files,
            commands::import_roster,
            commands::add_subject,
            commands::list_subject_groups,
            commands::list_subjects,
            commands::auto_match_photos,
            commands::list_ingest_batches,
            commands::list_folders,
            commands::apply_folder_reassignments,
            commands::remove_folder,
            commands::get_lan_server_status,
            commands::start_lan_server,
            commands::stop_lan_server,
            commands::discover_card_readers,
            commands::list_directory,
            commands::list_reader_configs,
            commands::save_reader_config,
            commands::save_reader_layout,
            commands::delete_reader_config,
            commands::clear_reader_configs,
            commands::get_app_setting,
            commands::set_app_setting,
            commands::ingest_from_reader,
            commands::list_orders,
            commands::update_order_payment,
            commands::update_order_notes,
            commands::update_order_item_fulfillment,
            commands::list_event_products,
            commands::upsert_event_product,
            commands::update_event_order_settings,
            commands::discover_usb_drives,
            commands::eject_usb_drive,
            commands::get_order_usb_plan,
            commands::copy_order_to_usb,
            commands::export_orders_csv,
        ])
        .run(tauri::generate_context!())
        .expect("error while running PhotoFinish");
}
