mod commands;

use std::sync::Arc;
use tauri::Manager;
use tokio::sync::{mpsc, Mutex, Semaphore};

use photofinish_core::db::AppDb;
use photofinish_core::event::{EventBundle, PendingPhotoJob};
use photofinish_core::imaging::process_photo_in_bundle;
use photofinish_core::server::{EmbeddedServer, SharedActiveEvent};

pub struct ImageTask {
    pub bundle: Arc<EventBundle>,
    pub job: PendingPhotoJob,
}

pub struct AppState {
    pub app_db: Arc<AppDb>,
    pub active_event: SharedActiveEvent,
    pub lan_server: Mutex<Option<EmbeddedServer>>,
    pub image_tx: mpsc::UnboundedSender<ImageTask>,
}

impl AppState {
    pub fn queue_photo_jobs(&self, bundle: &Arc<EventBundle>, jobs: Vec<PendingPhotoJob>) {
        for job in jobs {
            let _ = self.image_tx.send(ImageTask {
                bundle: Arc::clone(bundle),
                job,
            });
        }
    }
}

fn spawn_image_worker() -> mpsc::UnboundedSender<ImageTask> {
    let (tx, mut rx) = mpsc::unbounded_channel::<ImageTask>();
    let concurrency = std::thread::available_parallelism()
        .map(|n| n.get().saturating_sub(2).max(2))
        .unwrap_or(4);
    let semaphore = Arc::new(Semaphore::new(concurrency));

    tauri::async_runtime::spawn(async move {
        while let Some(task) = rx.recv().await {
            let permit = match Arc::clone(&semaphore).acquire_owned().await {
                Ok(p) => p,
                Err(_) => break,
            };
            tauri::async_runtime::spawn_blocking(move || {
                let _permit = permit;
                let _ = process_photo_in_bundle(
                    &task.bundle,
                    &task.job.photo_id,
                    &task.job.relative_path,
                );
            });
        }
    });

    tx
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
            let app_db = Arc::new(
                AppDb::open(&data_dir.join("photofinish_app.db"))
                    .expect("failed to open machine-local app database"),
            );
            let active_event = SharedActiveEvent::new(None);
            let image_tx = spawn_image_worker();

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

            app.manage(AppState {
                app_db,
                active_event,
                lan_server: Mutex::new(initial_server),
                image_tx,
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_presets,
            commands::list_library_events,
            commands::remove_library_event,
            commands::get_active_event,
            commands::create_event,
            commands::open_event,
            commands::gather_events,
            commands::scan_active_event,
            commands::get_lan_server_status,
            commands::start_lan_server,
            commands::stop_lan_server,
            commands::discover_card_readers,
            commands::list_directory,
            commands::list_reader_configs,
            commands::save_reader_config,
            commands::clear_reader_configs,
            commands::ingest_from_reader,
        ])
        .run(tauri::generate_context!())
        .expect("error while running PhotoFinish");
}
