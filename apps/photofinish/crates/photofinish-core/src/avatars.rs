use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use crate::db::{avatars_dir, subject_avatar_path};
use crate::event::EventBundle;
use crate::{Error, Result};

pub const AVATAR_OUTPUT_SIZE_PX: i32 = 256;
pub const AVATAR_JPEG_QUALITY: f64 = 0.85;
pub const AVATAR_SAMPLE_LIMIT: usize = 8;

#[cfg(target_os = "macos")]
extern "C" {
    fn pf_vision_is_available() -> std::os::raw::c_int;
    fn pf_vision_generate_avatar(
        preview_paths: *const *const std::os::raw::c_char,
        path_count: usize,
        output_jpeg_path: *const std::os::raw::c_char,
        output_size_px: std::os::raw::c_int,
        jpeg_quality: f64,
    ) -> std::os::raw::c_int;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AvatarOutcome {
    FaceGenerated,
    SaliencyGenerated,
    FallbackGenerated,
    SkippedExisting,
    NoPreviews,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvatarMetrics {
    pub engine: String,
    pub queued: usize,
    pub in_flight: usize,
    pub completed: u64,
    pub errors: u64,
    pub total_with_photos: usize,
    pub cached_avatars: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvatarBatchUpdatedEvent {
    pub event_root: String,
    pub generated: usize,
    pub subject_ids: Vec<String>,
}

pub fn active_avatar_engine_name() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        if unsafe { pf_vision_is_available() } != 0 {
            return "macOS Apple Vision (Face + Saliency)";
        }
    }
    "Square Center-Top Fallback"
}

/// Generates a cached `256x256` square portrait avatar at `_photofinish/avatars/{subject_id}.jpg`
/// using up to 8 already-generated `1280px` previews (never touching 24MP originals).
///
/// On macOS Apple Silicon, runs Apple's native Vision framework (`VNDetectFaceCaptureQualityRequest`
/// + `VNGenerateAttentionBasedSaliencyImageRequest`) on the Neural Engine to isolate the solo
/// foreground gymnast and frame face + hair + leotard cleanly (~100 ms/athlete).
pub fn generate_subject_avatar(
    bundle: &EventBundle,
    subject_id: &str,
    force: bool,
) -> Result<AvatarOutcome> {
    let out_path = subject_avatar_path(bundle.root(), subject_id);
    if !force && out_path.is_file() {
        return Ok(AvatarOutcome::SkippedExisting);
    }

    let preview_paths = bundle.sample_subject_preview_paths(subject_id, AVATAR_SAMPLE_LIMIT)?;
    if preview_paths.is_empty() {
        return Ok(AvatarOutcome::NoPreviews);
    }

    generate_avatar_from_preview_paths(&preview_paths, &out_path)
}

pub fn generate_avatar_from_preview_paths(
    preview_paths: &[PathBuf],
    output_path: &Path,
) -> Result<AvatarOutcome> {
    if preview_paths.is_empty() {
        return Ok(AvatarOutcome::NoPreviews);
    }

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| Error::Io(format!("{}: {}", parent.display(), e)))?;
    }

    #[cfg(target_os = "macos")]
    {
        use std::ffi::CString;
        let mut c_paths: Vec<CString> = Vec::with_capacity(preview_paths.len());
        for p in preview_paths {
            if let Some(s) = p.to_str() {
                if let Ok(cs) = CString::new(s) {
                    c_paths.push(cs);
                }
            }
        }
        let ptrs: Vec<*const std::os::raw::c_char> = c_paths.iter().map(|c| c.as_ptr()).collect();
        let out_c = CString::new(
            output_path
                .to_str()
                .ok_or_else(|| Error::InvalidInput("Non-UTF8 avatar path".into()))?,
        )
        .map_err(|e| Error::InvalidInput(e.to_string()))?;

        let code = unsafe {
            pf_vision_generate_avatar(
                ptrs.as_ptr(),
                ptrs.len(),
                out_c.as_ptr(),
                AVATAR_OUTPUT_SIZE_PX,
                AVATAR_JPEG_QUALITY,
            )
        };

        match code {
            1 => return Ok(AvatarOutcome::FaceGenerated),
            2 => return Ok(AvatarOutcome::SaliencyGenerated),
            0 => {
                // Fall through to pure-Rust center-top square crop if Vision found nothing
            }
            _ => {
                return Err(Error::Image(format!(
                    "Apple Vision avatar writer failed for {}",
                    output_path.display()
                )));
            }
        }
    }

    generate_center_top_fallback(&preview_paths[0], output_path)?;
    Ok(AvatarOutcome::FallbackGenerated)
}

/// Pure-Rust center-top square crop fallback (used on non-macOS or if Vision detects no region).
fn generate_center_top_fallback(src_path: &Path, dst_path: &Path) -> Result<()> {
    let img = image::open(src_path)
        .map_err(|e| Error::Image(format!("open {}: {}", src_path.display(), e)))?;
    let (w, h) = (img.width(), img.height());
    let side = w.min(h);
    let x = (w - side) / 2;
    // Bias upward so portraits keep the athlete's head in frame
    let y = ((h - side) as f32 * 0.2) as u32;
    let cropped = img.crop_imm(x, y, side, side);
    let resized = cropped.resize_exact(
        AVATAR_OUTPUT_SIZE_PX as u32,
        AVATAR_OUTPUT_SIZE_PX as u32,
        image::imageops::FilterType::Triangle,
    );

    if let Some(parent) = dst_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| Error::Io(format!("{}: {}", parent.display(), e)))?;
    }
    let tmp = dst_path.with_extension("jpg.tmp");
    let mut out_file = std::fs::File::create(&tmp)
        .map_err(|e| Error::Io(format!("create {}: {}", tmp.display(), e)))?;
    let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(
        &mut out_file,
        (AVATAR_JPEG_QUALITY * 100.0) as u8,
    );
    enc.encode_image(&resized)
        .map_err(|e| Error::Image(format!("encode avatar: {}", e)))?;
    drop(out_file);
    std::fs::rename(&tmp, dst_path)
        .map_err(|e| Error::Io(format!("rename avatar: {}", e)))?;
    Ok(())
}

/// Counts how many matched subjects with ready previews exist vs how many have cached avatars.
pub fn count_event_avatar_coverage(bundle: &EventBundle) -> Result<(usize, usize)> {
    let ids = bundle.list_subject_ids_with_ready_photos()?;
    let total = ids.len();
    let dir = avatars_dir(bundle.root());
    if !dir.is_dir() {
        return Ok((total, 0));
    }
    let cached = ids
        .iter()
        .filter(|sid| subject_avatar_path(bundle.root(), sid).is_file())
        .count();
    Ok((total, cached))
}

// ============================================================================
// Low-Priority Background Avatar Pipeline (deduplicated, non-blocking)
// ============================================================================

struct AvatarJob {
    bundle: Arc<EventBundle>,
    subject_id: String,
    force: bool,
}

struct SharedAvatarQueue {
    items: Mutex<VecDeque<AvatarJob>>,
    enqueued_keys: Mutex<HashSet<(PathBuf, String)>>,
    notify: Condvar,
}

type AvatarUpdateCallback = Arc<dyn Fn(AvatarBatchUpdatedEvent) + Send + Sync>;

#[derive(Clone)]
pub struct AvatarPipeline {
    queue: Arc<SharedAvatarQueue>,
    in_flight: Arc<AtomicUsize>,
    completed: Arc<AtomicU64>,
    errors: Arc<AtomicU64>,
}

impl AvatarPipeline {
    /// Spawns a lightweight 2-worker background pool so Vision runs smoothly on Apple's
    /// Neural Engine without competing with card copy or thumbnail workers.
    pub fn start(on_update: Option<AvatarUpdateCallback>) -> Self {
        let queue = Arc::new(SharedAvatarQueue {
            items: Mutex::new(VecDeque::new()),
            enqueued_keys: Mutex::new(HashSet::new()),
            notify: Condvar::new(),
        });
        let in_flight = Arc::new(AtomicUsize::new(0));
        let completed = Arc::new(AtomicU64::new(0));
        let errors = Arc::new(AtomicU64::new(0));

        // 2 workers saturate the Neural Engine nicely (~20 athletes/sec) without starving CPU
        let workers = 2usize;
        for idx in 0..workers {
            let q = Arc::clone(&queue);
            let inf = Arc::clone(&in_flight);
            let comp = Arc::clone(&completed);
            let errs = Arc::clone(&errors);
            let cb = on_update.clone();

            let _ = thread::Builder::new()
                .name(format!("pf-avatar-{}", idx))
                .spawn(move || loop {
                    let job = {
                        let mut guard = q.items.lock().unwrap();
                        while guard.is_empty() {
                            guard = q.notify.wait(guard).unwrap();
                        }
                        guard.pop_front().unwrap()
                    };

                    inf.fetch_add(1, Ordering::SeqCst);
                    let key = (job.bundle.root().to_path_buf(), job.subject_id.clone());
                    let res = generate_subject_avatar(&job.bundle, &job.subject_id, job.force);
                    if let Ok(mut keys) = q.enqueued_keys.lock() {
                        keys.remove(&key);
                    }
                    inf.fetch_sub(1, Ordering::SeqCst);

                    match res {
                        Ok(
                            AvatarOutcome::FaceGenerated
                            | AvatarOutcome::SaliencyGenerated
                            | AvatarOutcome::FallbackGenerated,
                        ) => {
                            comp.fetch_add(1, Ordering::Relaxed);
                            if let Some(ref notify_fn) = cb {
                                notify_fn(AvatarBatchUpdatedEvent {
                                    event_root: job.bundle.root().to_string_lossy().to_string(),
                                    generated: 1,
                                    subject_ids: vec![job.subject_id],
                                });
                            }
                        }
                        Ok(_) => {}
                        Err(_) => {
                            errs.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                });
        }

        Self {
            queue,
            in_flight,
            completed,
            errors,
        }
    }

    /// Enqueues specific subjects for background avatar generation (deduplicated).
    pub fn enqueue_subjects(
        &self,
        bundle: &Arc<EventBundle>,
        subject_ids: impl IntoIterator<Item = String>,
        force: bool,
    ) -> usize {
        let mut items = self.queue.items.lock().unwrap();
        let mut keys = self.queue.enqueued_keys.lock().unwrap();
        let mut added = 0usize;

        for sid in subject_ids {
            if !force && subject_avatar_path(bundle.root(), &sid).is_file() {
                continue;
            }
            let key = (bundle.root().to_path_buf(), sid.clone());
            if keys.insert(key) {
                items.push_back(AvatarJob {
                    bundle: Arc::clone(bundle),
                    subject_id: sid,
                    force,
                });
                added += 1;
            }
        }
        drop(keys);
        drop(items);
        if added > 0 {
            self.queue.notify.notify_all();
        }
        added
    }

    /// Queues all matched subjects in an Event Bundle that have ready previews (skipping
    /// already-cached avatars unless `force == true`).
    pub fn enqueue_event_subjects(&self, bundle: &Arc<EventBundle>, force: bool) -> Result<usize> {
        let ids = bundle.list_subject_ids_with_ready_photos()?;
        Ok(self.enqueue_subjects(bundle, ids, force))
    }

    pub fn metrics(&self, bundle: Option<&EventBundle>) -> AvatarMetrics {
        let queued = self.queue.items.lock().unwrap().len();
        let in_flight = self.in_flight.load(Ordering::Relaxed);
        let (total_with_photos, cached_avatars) = bundle
            .and_then(|b| count_event_avatar_coverage(b).ok())
            .unwrap_or((0, 0));

        AvatarMetrics {
            engine: active_avatar_engine_name().to_string(),
            queued,
            in_flight,
            completed: self.completed.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
            total_with_photos,
            cached_avatars,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb};
    use tempfile::tempdir;

    #[test]
    fn generates_square_avatar_from_preview_jpeg() {
        let dir = tempdir().unwrap();
        let preview = dir.path().join("prev.jpg");
        let out = dir.path().join("_photofinish/avatars/sub_test01.jpg");

        let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_fn(800, 1200, |x, y| Rgb([(x % 255) as u8, (y % 255) as u8, 200]));
        img.save(&preview).unwrap();

        let outcome = generate_avatar_from_preview_paths(&[preview], &out).unwrap();
        assert!(matches!(
            outcome,
            AvatarOutcome::FaceGenerated
                | AvatarOutcome::SaliencyGenerated
                | AvatarOutcome::FallbackGenerated
        ));
        assert!(out.is_file());

        let decoded = image::open(&out).unwrap();
        assert_eq!(decoded.width(), AVATAR_OUTPUT_SIZE_PX as u32);
        assert_eq!(decoded.height(), AVATAR_OUTPUT_SIZE_PX as u32);
    }
}
