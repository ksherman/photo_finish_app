use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::event::EventBundle;
use crate::imaging::ImagePipeline;
use crate::util::{is_jpeg_path, now_iso};
use crate::{Error, Result};

const DEFAULT_DEBOUNCE_MS: u64 = 1500;
const INTERNAL_COOLDOWN_MS: u64 = 1000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatcherStatus {
    pub enabled: bool,
    pub watching: bool,
    pub watched_root: Option<String>,
    pub last_sync_at: Option<String>,
    pub last_added: usize,
    pub last_removed: usize,
    pub total_added: usize,
    pub total_removed: usize,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatcherSyncEvent {
    pub event_root: String,
    pub added: usize,
    pub removed: usize,
    pub queued: usize,
    pub synced_at: String,
}

type SyncCallback = Arc<dyn Fn(WatcherSyncEvent) + Send + Sync>;

/// RAII guard that pauses auto-watcher ingestion while PhotoFinish performs internal disk writes
/// (e.g., Card Ingest, Rotation Folder Renaming, Folder Removal) and applies a short cooldown
/// afterwards so trailing macOS FSEvents from our own writes are cleanly ignored.
pub struct WatcherPauseGuard {
    pause_count: Arc<AtomicUsize>,
    suppress_until_ms: Arc<AtomicU64>,
}

impl Drop for WatcherPauseGuard {
    fn drop(&mut self) {
        let until = now_epoch_ms().saturating_add(INTERNAL_COOLDOWN_MS);
        self.suppress_until_ms.store(until, Ordering::SeqCst);
        self.pause_count.fetch_sub(1, Ordering::SeqCst);
    }
}

struct ActiveWatchHandle {
    _watcher: RecommendedWatcher,
    stop_flag: Arc<AtomicBool>,
    tx: mpsc::Sender<WatchSignal>,
    thread: Option<JoinHandle<()>>,
    root: PathBuf,
}

impl ActiveWatchHandle {
    fn stop(mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        let _ = self.tx.send(WatchSignal::Stop);
        drop(self._watcher);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

enum WatchSignal {
    Paths(Vec<PathBuf>),
    RescanAll,
    Stop,
}

/// Manages a single macOS FSEvents / cross-platform `notify` stream for the active Event Bundle.
pub struct EventWatcherManager {
    enabled: bool,
    debounce: Duration,
    pause_count: Arc<AtomicUsize>,
    suppress_until_ms: Arc<AtomicU64>,
    telemetry: Arc<Mutex<WatcherTelemetry>>,
    active: Option<ActiveWatchHandle>,
    on_sync: Option<SyncCallback>,
}

#[derive(Default)]
struct WatcherTelemetry {
    last_sync_at: Option<String>,
    last_added: usize,
    last_removed: usize,
    total_added: usize,
    total_removed: usize,
    last_error: Option<String>,
}

impl EventWatcherManager {
    pub fn new(enabled: bool, on_sync: Option<SyncCallback>) -> Self {
        Self::with_debounce(enabled, Duration::from_millis(DEFAULT_DEBOUNCE_MS), on_sync)
    }

    pub fn with_debounce(
        enabled: bool,
        debounce: Duration,
        on_sync: Option<SyncCallback>,
    ) -> Self {
        Self {
            enabled,
            debounce,
            pause_count: Arc::new(AtomicUsize::new(0)),
            suppress_until_ms: Arc::new(AtomicU64::new(0)),
            telemetry: Arc::new(Mutex::new(WatcherTelemetry::default())),
            active: None,
            on_sync,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Acquires an RAII pause guard for internal file mutations (Card Ingest, Renames, Deletes).
    pub fn pause(&self) -> WatcherPauseGuard {
        self.pause_count.fetch_add(1, Ordering::SeqCst);
        WatcherPauseGuard {
            pause_count: Arc::clone(&self.pause_count),
            suppress_until_ms: Arc::clone(&self.suppress_until_ms),
        }
    }

    pub fn stop(&mut self) {
        if let Some(handle) = self.active.take() {
            handle.stop();
        }
    }

    pub fn set_enabled(
        &mut self,
        enabled: bool,
        active_bundle: Option<Arc<EventBundle>>,
        image_pipeline: &ImagePipeline,
    ) -> Result<WatcherStatus> {
        self.enabled = enabled;
        if !enabled {
            self.stop();
        } else if let Some(bundle) = active_bundle {
            self.watch_bundle(bundle, image_pipeline)?;
        }
        Ok(self.status())
    }

    /// Starts (or switches) watching the given `EventBundle` if auto-watch is enabled.
    pub fn watch_bundle(
        &mut self,
        bundle: Arc<EventBundle>,
        image_pipeline: &ImagePipeline,
    ) -> Result<()> {
        self.stop();
        if !self.enabled {
            return Ok(());
        }

        let raw_root = bundle.root().to_path_buf();
        let canonical_root = fs::canonicalize(&raw_root).unwrap_or_else(|_| raw_root.clone());

        let (tx, rx) = mpsc::channel::<WatchSignal>();
        let stop_flag = Arc::new(AtomicBool::new(false));
        let cb_tx = tx.clone();
        let cb_pause = Arc::clone(&self.pause_count);
        let cb_suppress = Arc::clone(&self.suppress_until_ms);

        let mut watcher =
            notify::recommended_watcher(move |res: std::result::Result<Event, notify::Error>| {
                if cb_pause.load(Ordering::SeqCst) > 0
                    || now_epoch_ms() < cb_suppress.load(Ordering::SeqCst)
                {
                    return;
                }
                match res {
                    Ok(ev) => {
                        // Ignore pure metadata / access-time touches (e.g. Spotlight reading files)
                        if matches!(ev.kind, notify::EventKind::Access(_)) {
                            return;
                        }
                        if ev.need_rescan() {
                            let _ = cb_tx.send(WatchSignal::RescanAll);
                        } else if !ev.paths.is_empty() {
                            let _ = cb_tx.send(WatchSignal::Paths(ev.paths));
                        }
                    }
                    Err(_) => {
                        let _ = cb_tx.send(WatchSignal::RescanAll);
                    }
                }
            })
            .map_err(|e| Error::Io(format!("Failed to initialize filesystem watcher: {}", e)))?;

        watcher
            .watch(&raw_root, RecursiveMode::Recursive)
            .map_err(|e| {
                Error::Io(format!(
                    "Failed to watch event folder {}: {}",
                    raw_root.display(),
                    e
                ))
            })?;

        // Reset telemetry for newly watched bundle
        if let Ok(mut t) = self.telemetry.lock() {
            *t = WatcherTelemetry::default();
        }

        let worker_stop = Arc::clone(&stop_flag);
        let worker_pause = Arc::clone(&self.pause_count);
        let worker_suppress = Arc::clone(&self.suppress_until_ms);
        let worker_telemetry = Arc::clone(&self.telemetry);
        let worker_pipeline = image_pipeline.clone();
        let worker_callback = self.on_sync.clone();
        let debounce = self.debounce;
        let thread_root = raw_root.clone();

        let thread = thread::Builder::new()
            .name("pf-fs-watcher".into())
            .spawn(move || {
                let mut dirty: HashSet<String> = HashSet::new();
                let mut last_event_at: Option<Instant> = None;

                loop {
                    if worker_stop.load(Ordering::SeqCst) {
                        break;
                    }

                    let timeout = match last_event_at {
                        Some(ts) => debounce.saturating_sub(ts.elapsed()),
                        None => Duration::from_secs(60),
                    };

                    match rx.recv_timeout(timeout) {
                        Ok(WatchSignal::Stop) => break,
                        Ok(WatchSignal::RescanAll) => {
                            dirty.clear();
                            dirty.insert(String::new());
                            last_event_at = Some(Instant::now());
                        }
                        Ok(WatchSignal::Paths(paths)) => {
                            let mut any = false;
                            for p in paths {
                                if let Some(rel) =
                                    normalize_watch_path(&thread_root, &canonical_root, &p)
                                {
                                    dirty.insert(rel);
                                    any = true;
                                }
                            }
                            if any {
                                last_event_at = Some(Instant::now());
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            if dirty.is_empty() {
                                last_event_at = None;
                                continue;
                            }
                            // Wait if an internal operation is currently paused/cooling down
                            if worker_pause.load(Ordering::SeqCst) > 0
                                || now_epoch_ms() < worker_suppress.load(Ordering::SeqCst)
                            {
                                last_event_at = Some(Instant::now());
                                continue;
                            }

                            let batch: Vec<String> = dirty.drain().collect();
                            last_event_at = None;

                            match bundle.reconcile_subpaths(&batch) {
                                Ok(scan) => {
                                    let queued = scan.pending_jobs.len();
                                    if !scan.pending_jobs.is_empty() {
                                        worker_pipeline.enqueue(&bundle, scan.pending_jobs);
                                    }
                                    if scan.photos_new > 0 || scan.photos_removed > 0 {
                                        let synced_at = now_iso();
                                        if let Ok(mut t) = worker_telemetry.lock() {
                                            t.last_sync_at = Some(synced_at.clone());
                                            t.last_added = scan.photos_new;
                                            t.last_removed = scan.photos_removed;
                                            t.total_added += scan.photos_new;
                                            t.total_removed += scan.photos_removed;
                                            t.last_error = None;
                                        }
                                        if let Some(ref cb) = worker_callback {
                                            cb(WatcherSyncEvent {
                                                event_root: thread_root
                                                    .to_string_lossy()
                                                    .to_string(),
                                                added: scan.photos_new,
                                                removed: scan.photos_removed,
                                                queued,
                                                synced_at,
                                            });
                                        }
                                    }
                                }
                                Err(err) => {
                                    if let Ok(mut t) = worker_telemetry.lock() {
                                        t.last_error = Some(err.to_string());
                                    }
                                }
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
            })
            .map_err(|e| Error::Io(format!("Failed to spawn watcher thread: {}", e)))?;

        self.active = Some(ActiveWatchHandle {
            _watcher: watcher,
            stop_flag,
            tx,
            thread: Some(thread),
            root: raw_root,
        });

        Ok(())
    }

    pub fn status(&self) -> WatcherStatus {
        let t = self.telemetry.lock().unwrap();
        WatcherStatus {
            enabled: self.enabled,
            watching: self.active.is_some(),
            watched_root: self
                .active
                .as_ref()
                .map(|a| a.root.to_string_lossy().to_string()),
            last_sync_at: t.last_sync_at.clone(),
            last_added: t.last_added,
            last_removed: t.last_removed,
            total_added: t.total_added,
            total_removed: t.total_removed,
            last_error: t.last_error.clone(),
        }
    }
}

impl Drop for EventWatcherManager {
    fn drop(&mut self) {
        self.stop();
    }
}

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Converts an OS-delivered absolute path into a clean relative folder prefix inside the event
/// bundle, ignoring hidden dotfiles and `_`-prefixed internal directories (like `_photofinish`).
pub fn normalize_watch_path(
    raw_root: &Path,
    canonical_root: &Path,
    abs_path: &Path,
) -> Option<String> {
    let rel = abs_path
        .strip_prefix(canonical_root)
        .or_else(|_| abs_path.strip_prefix(raw_root))
        .ok()?;

    if rel.as_os_str().is_empty() {
        return Some(String::new());
    }

    // Ignore anything under `_photofinish/` or any hidden `.`-prefixed file/folder
    for comp in rel.components() {
        let s = comp.as_os_str().to_string_lossy();
        if s.starts_with('.') || s.starts_with('_') {
            return None;
        }
    }

    // Map individual files (whether present or deleted JPEGs) to their parent directory
    // so reconciling handles both adds & single-file deletes cleanly in O(folder size).
    let target_rel = if abs_path.is_file() || is_jpeg_path(abs_path) {
        rel.parent().unwrap_or_else(|| Path::new(""))
    } else {
        rel
    };

    let s = target_rel
        .to_string_lossy()
        .replace('\\', "/")
        .trim_matches('/')
        .to_string();
    Some(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::CreateEventParams;
    use tempfile::tempdir;

    #[test]
    fn normalize_watch_path_ignores_internal_and_maps_jpegs_to_parent() {
        let root = Path::new("/Volumes/SSD/Meet");
        let canon = Path::new("/Volumes/SSD/Meet");

        assert_eq!(
            normalize_watch_path(root, canon, Path::new("/Volumes/SSD/Meet/_photofinish/photofinish.db-wal")),
            None
        );
        assert_eq!(
            normalize_watch_path(root, canon, Path::new("/Volumes/SSD/Meet/Gym A/.DS_Store")),
            None
        );
        assert_eq!(
            normalize_watch_path(
                root,
                canon,
                Path::new("/Volumes/SSD/Meet/Gym A/Session 10A/105 Julia B/IMG_001.jpg")
            ),
            Some("Gym A/Session 10A/105 Julia B".to_string())
        );
        assert_eq!(
            normalize_watch_path(
                root,
                canon,
                Path::new("/Volumes/SSD/Meet/Gym A/Session 18A")
            ),
            Some("Gym A/Session 18A".to_string())
        );
        assert_eq!(
            normalize_watch_path(root, canon, Path::new("/Volumes/SSD/Meet")),
            Some(String::new())
        );
    }

    #[test]
    fn watcher_auto_indexes_finder_drop_and_prunes_deleted_folder() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("watch-meet");
        let bundle = Arc::new(
            EventBundle::create(CreateEventParams {
                root_path: root.clone(),
                name: "Watch Meet".into(),
                order_code: Some("WTC".into()),
                ..Default::default()
            })
            .unwrap(),
        );

        let pipeline = ImagePipeline::start(Some(1));
        let (sync_tx, sync_rx) = mpsc::channel::<WatcherSyncEvent>();
        let mut mgr = EventWatcherManager::with_debounce(
            true,
            Duration::from_millis(200),
            Some(Arc::new(move |ev| {
                let _ = sync_tx.send(ev);
            })),
        );
        mgr.watch_bundle(Arc::clone(&bundle), &pipeline).unwrap();

        // Give OS watcher a moment to arm its stream
        thread::sleep(Duration::from_millis(150));

        // 1. Simulate dropping a folder with 2 JPEGs in Finder
        let session_dir = root.join("Gym A/Session 18A/Group 1A/Beam/105 Julia B");
        fs::create_dir_all(&session_dir).unwrap();
        fs::write(session_dir.join("IMG_001.jpg"), b"fake-jpeg-1").unwrap();
        fs::write(session_dir.join("IMG_002.jpg"), b"fake-jpeg-2").unwrap();

        let add_ev = sync_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("watcher should detect dropped photos");
        assert_eq!(add_ev.added, 2);
        assert_eq!(add_ev.removed, 0);
        assert_eq!(bundle.photo_counts().unwrap().total, 2);

        // 2. Simulate deleting Session 18A in Finder
        fs::remove_dir_all(root.join("Gym A/Session 18A")).unwrap();

        let del_ev = sync_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("watcher should prune deleted folder");
        assert_eq!(del_ev.removed, 2);
        assert_eq!(bundle.photo_counts().unwrap().total, 0);

        mgr.stop();
    }
}
