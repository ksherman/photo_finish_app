#[cfg(target_os = "macos")]
mod macos;

use fast_image_resize::images::{Image, ImageRef};
use fast_image_resize::{FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer};
use image::codecs::jpeg::{JpegDecoder, JpegEncoder};
use image::metadata::Orientation;
use image::{DynamicImage, ExtendedColorType, ImageDecoder, RgbImage};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter};
use std::path::Path;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use crate::db::{BUNDLE_DIR_NAME, PREVIEWS_DIR_NAME, THUMBNAILS_DIR_NAME};
use crate::event::{EventBundle, PendingPhotoJob, PhotoProcessUpdate};
use crate::{Error, Result};

pub const THUMBNAIL_MAX_EDGE: u32 = 320;
pub const PREVIEW_MAX_EDGE: u32 = 1280;
pub const THUMBNAIL_QUALITY: u8 = 82;
pub const PREVIEW_QUALITY: u8 = 86;

const DB_FLUSH_BATCH_SIZE: usize = 64;
const DB_FLUSH_MAX_WAIT: Duration = Duration::from_millis(150);

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StageTimingsMs {
    pub decode_preview_ms: f64,
    pub encode_preview_ms: f64,
    pub thumb_ms: f64,
    pub total_ms: f64,
}

#[derive(Debug, Clone)]
pub struct GeneratedDerivatives {
    pub thumbnail_rel_path: String,
    pub preview_rel_path: String,
    pub original_width: u32,
    pub original_height: u32,
    pub timings: StageTimingsMs,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessingMetrics {
    pub engine: String,
    pub worker_threads: usize,
    pub in_flight: usize,
    pub queued: usize,
    pub completed: u64,
    pub errors: u64,
    pub photos_per_sec: f64,
    pub eta_seconds: Option<f64>,
    pub avg_decode_preview_ms: f64,
    pub avg_encode_preview_ms: f64,
    pub avg_thumb_ms: f64,
    pub avg_total_ms: f64,
    pub avg_db_flush_ms: f64,
}

pub fn active_engine_name() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "macOS ImageIO (Hardware DCT)"
    }
    #[cfg(not(target_os = "macos"))]
    {
        "Pure Rust SIMD (fast_image_resize)"
    }
}

/// Generates a 1280px preview and 320px thumbnail for `source_rel_path` inside `event_root`.
pub fn generate_derivatives(
    event_root: &Path,
    photo_id: &str,
    source_rel_path: &str,
) -> Result<GeneratedDerivatives> {
    let mut resizer = Resizer::new();
    generate_derivatives_with_resizer(event_root, photo_id, source_rel_path, &mut resizer)
}

pub fn generate_derivatives_with_resizer(
    event_root: &Path,
    photo_id: &str,
    source_rel_path: &str,
    resizer: &mut Resizer,
) -> Result<GeneratedDerivatives> {
    #[cfg(target_os = "macos")]
    {
        match macos::generate_derivatives_imageio(event_root, photo_id, source_rel_path) {
            Ok(deriv) => return Ok(deriv),
            Err(err) => {
                tracing::debug!(
                    "ImageIO fallback to pure-Rust for {}: {}",
                    source_rel_path,
                    err
                );
            }
        }
    }

    generate_derivatives_pure_rust(event_root, photo_id, source_rel_path, resizer)
}

/// Optimized pure-Rust SIMD pipeline:
/// 1. Resizes full-res RGB -> 1280px BEFORE applying EXIF orientation.
/// 2. Cascades 1280px -> 320px from the already-oriented preview.
/// 3. Reuses the caller's thread-local `Resizer`.
pub fn generate_derivatives_pure_rust(
    event_root: &Path,
    photo_id: &str,
    source_rel_path: &str,
    resizer: &mut Resizer,
) -> Result<GeneratedDerivatives> {
    let total_start = Instant::now();
    let source_path = event_root.join(source_rel_path);
    let file =
        File::open(&source_path).map_err(|e| Error::Io(format!("{}: {}", source_path.display(), e)))?;

    let s0 = Instant::now();
    let mut decoder =
        JpegDecoder::new(BufReader::new(file)).map_err(|e| Error::Image(e.to_string()))?;
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let rgb = DynamicImage::from_decoder(decoder)
        .map_err(|e| Error::Image(e.to_string()))?
        .into_rgb8();
    let (raw_w, raw_h) = rgb.dimensions();

    let (orig_w, orig_h) = match orientation {
        Orientation::Rotate90
        | Orientation::Rotate270
        | Orientation::Rotate90FlipH
        | Orientation::Rotate270FlipH => (raw_h, raw_w),
        _ => (raw_w, raw_h),
    };

    let opts = ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::CatmullRom));

    let (pw, ph) = fit_dimensions(raw_w, raw_h, PREVIEW_MAX_EDGE);
    let prev_raw: Vec<u8> = if pw == raw_w && ph == raw_h {
        rgb.into_raw()
    } else {
        let src_view = ImageRef::new(raw_w, raw_h, rgb.as_raw(), PixelType::U8x3)
            .map_err(|e| Error::Image(e.to_string()))?;
        let mut prev_img = Image::new(pw, ph, PixelType::U8x3);
        resizer
            .resize(&src_view, &mut prev_img, Some(&opts))
            .map_err(|e| Error::Image(e.to_string()))?;
        prev_img.into_vec()
    };

    let (prev_buf, final_pw, final_ph) = if orientation == Orientation::NoTransforms {
        (prev_raw, pw, ph)
    } else {
        let rgb_prev = RgbImage::from_raw(pw, ph, prev_raw)
            .ok_or_else(|| Error::Image("preview buffer size mismatch".into()))?;
        let mut dyn_prev = DynamicImage::ImageRgb8(rgb_prev);
        dyn_prev.apply_orientation(orientation);
        let oriented = dyn_prev.into_rgb8();
        let (ow, oh) = oriented.dimensions();
        (oriented.into_raw(), ow, oh)
    };
    let decode_preview_ms = s0.elapsed().as_secs_f64() * 1000.0;

    let thumb_rel = format!("{}/{}/{}.jpg", BUNDLE_DIR_NAME, THUMBNAILS_DIR_NAME, photo_id);
    let prev_rel = format!("{}/{}/{}.jpg", BUNDLE_DIR_NAME, PREVIEWS_DIR_NAME, photo_id);

    let s1 = Instant::now();
    encode_rgb_jpeg(
        &prev_buf,
        final_pw,
        final_ph,
        PREVIEW_QUALITY,
        &event_root.join(&prev_rel),
    )?;
    let encode_preview_ms = s1.elapsed().as_secs_f64() * 1000.0;

    let s2 = Instant::now();
    let (tw, th) = fit_dimensions(final_pw, final_ph, THUMBNAIL_MAX_EDGE);
    let thumb_buf: Vec<u8> = if tw == final_pw && th == final_ph {
        prev_buf
    } else {
        let prev_view = ImageRef::new(final_pw, final_ph, &prev_buf, PixelType::U8x3)
            .map_err(|e| Error::Image(e.to_string()))?;
        let mut thumb_img = Image::new(tw, th, PixelType::U8x3);
        resizer
            .resize(&prev_view, &mut thumb_img, Some(&opts))
            .map_err(|e| Error::Image(e.to_string()))?;
        thumb_img.into_vec()
    };
    encode_rgb_jpeg(&thumb_buf, tw, th, THUMBNAIL_QUALITY, &event_root.join(&thumb_rel))?;
    let thumb_ms = s2.elapsed().as_secs_f64() * 1000.0;

    Ok(GeneratedDerivatives {
        thumbnail_rel_path: thumb_rel,
        preview_rel_path: prev_rel,
        original_width: orig_w,
        original_height: orig_h,
        timings: StageTimingsMs {
            decode_preview_ms,
            encode_preview_ms,
            thumb_ms,
            total_ms: total_start.elapsed().as_secs_f64() * 1000.0,
        },
    })
}

fn encode_rgb_jpeg(rgb: &[u8], w: u32, h: u32, quality: u8, output_path: &Path) -> Result<()> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|e| Error::Io(format!("{}: {}", parent.display(), e)))?;
    }
    let out_file = File::create(output_path)
        .map_err(|e| Error::Io(format!("{}: {}", output_path.display(), e)))?;
    let mut writer = BufWriter::new(out_file);
    let mut encoder = JpegEncoder::new_with_quality(&mut writer, quality);
    encoder
        .encode(rgb, w, h, ExtendedColorType::Rgb8)
        .map_err(|e| Error::Image(e.to_string()))?;
    Ok(())
}

pub(crate) fn fit_dimensions(w: u32, h: u32, max_edge: u32) -> (u32, u32) {
    let longest = w.max(h);
    if longest <= max_edge || longest == 0 {
        return (w.max(1), h.max(1));
    }
    let scale = max_edge as f64 / longest as f64;
    let new_w = ((w as f64 * scale).round() as u32).max(1);
    let new_h = ((h as f64 * scale).round() as u32).max(1);
    (new_w, new_h)
}

// ============================================================================
// Dedicated Background ImagePipeline (Fixed Worker Pool + Batched DB Flusher)
// ============================================================================

struct QueuedTask {
    bundle: Arc<EventBundle>,
    job: PendingPhotoJob,
}

struct CompletedOutcome {
    bundle: Arc<EventBundle>,
    update: PhotoProcessUpdate,
}

struct TelemetryState {
    recent_completions: VecDeque<Instant>,
    samples: u64,
    sum_decode_preview_ms: f64,
    sum_encode_preview_ms: f64,
    sum_thumb_ms: f64,
    sum_total_ms: f64,
    db_flushes: u64,
    sum_db_flush_ms: f64,
    batch_start: Option<Instant>,
    batch_completed: u64,
}

impl Default for TelemetryState {
    fn default() -> Self {
        Self {
            recent_completions: VecDeque::with_capacity(512),
            samples: 0,
            sum_decode_preview_ms: 0.0,
            sum_encode_preview_ms: 0.0,
            sum_thumb_ms: 0.0,
            sum_total_ms: 0.0,
            db_flushes: 0,
            sum_db_flush_ms: 0.0,
            batch_start: None,
            batch_completed: 0,
        }
    }
}

struct SharedQueue {
    items: Mutex<VecDeque<QueuedTask>>,
    notify: Condvar,
}

#[derive(Clone)]
pub struct ImagePipeline {
    worker_threads: usize,
    queue: Arc<SharedQueue>,
    in_flight: Arc<AtomicUsize>,
    completed: Arc<AtomicU64>,
    errors: Arc<AtomicU64>,
    telemetry: Arc<Mutex<TelemetryState>>,
}

impl ImagePipeline {
    pub fn start(worker_threads: Option<usize>) -> Self {
        let workers = worker_threads.unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get().saturating_sub(2).max(2))
                .unwrap_or(4)
        });

        let queue = Arc::new(SharedQueue {
            items: Mutex::new(VecDeque::new()),
            notify: Condvar::new(),
        });
        let in_flight = Arc::new(AtomicUsize::new(0));
        let completed = Arc::new(AtomicU64::new(0));
        let errors = Arc::new(AtomicU64::new(0));
        let telemetry = Arc::new(Mutex::new(TelemetryState::default()));

        let (result_tx, result_rx) = mpsc::channel::<CompletedOutcome>();

        // Dedicated DB batch flusher thread (commits up to 64 updates per SQLite transaction)
        {
            let telemetry_db = Arc::clone(&telemetry);
            let completed_db = Arc::clone(&completed);
            let errors_db = Arc::clone(&errors);
            let queue_db = Arc::clone(&queue);
            let in_flight_db = Arc::clone(&in_flight);

            std::thread::Builder::new()
                .name("pf-db-flusher".into())
                .spawn(move || {
                    let mut pending: Vec<CompletedOutcome> = Vec::with_capacity(DB_FLUSH_BATCH_SIZE);
                    loop {
                        if pending.is_empty() {
                            match result_rx.recv() {
                                Ok(item) => pending.push(item),
                                Err(_) => break,
                            }
                        }

                        let deadline = Instant::now() + DB_FLUSH_MAX_WAIT;
                        while pending.len() < DB_FLUSH_BATCH_SIZE {
                            let remaining = deadline.saturating_duration_since(Instant::now());
                            if remaining.is_zero() {
                                break;
                            }
                            match result_rx.recv_timeout(remaining) {
                                Ok(item) => pending.push(item),
                                Err(_) => break,
                            }
                        }

                        if pending.is_empty() {
                            continue;
                        }

                        let flush_start = Instant::now();
                        flush_outcomes_by_bundle(&mut pending, &completed_db, &errors_db);
                        let flush_ms = flush_start.elapsed().as_secs_f64() * 1000.0;

                        let mut t = telemetry_db.lock().unwrap();
                        t.db_flushes += 1;
                        t.sum_db_flush_ms += flush_ms;

                        let remaining_q = queue_db.items.lock().unwrap().len();
                        if remaining_q == 0 && in_flight_db.load(Ordering::Relaxed) == 0 {
                            if let Some(start) = t.batch_start.take() {
                                let elapsed = start.elapsed().as_secs_f64().max(0.001);
                                let count = t.batch_completed;
                                if count >= 10 {
                                    tracing::info!(
                                        "PhotoFinish imaging batch done: {} photos in {:.2}s ({:.1} photos/sec) via {}",
                                        count,
                                        elapsed,
                                        count as f64 / elapsed,
                                        active_engine_name()
                                    );
                                }
                                t.batch_completed = 0;
                            }
                        }
                    }
                })
                .expect("failed to spawn DB batch flusher thread");
        }

        for worker_idx in 0..workers {
            let queue_w = Arc::clone(&queue);
            let in_flight_w = Arc::clone(&in_flight);
            let telemetry_w = Arc::clone(&telemetry);
            let tx_w = result_tx.clone();

            std::thread::Builder::new()
                .name(format!("pf-img-{}", worker_idx))
                .spawn(move || {
                    let mut resizer = Resizer::new();
                    loop {
                        let task = {
                            let mut guard = queue_w.items.lock().unwrap();
                            while guard.is_empty() {
                                guard = queue_w.notify.wait(guard).unwrap();
                            }
                            in_flight_w.fetch_add(1, Ordering::AcqRel);
                            guard.pop_front().unwrap()
                        };

                        let outcome = match generate_derivatives_with_resizer(
                            task.bundle.root(),
                            &task.job.photo_id,
                            &task.job.relative_path,
                            &mut resizer,
                        ) {
                            Ok(deriv) => {
                                let now = Instant::now();
                                let mut t = telemetry_w.lock().unwrap();
                                t.samples += 1;
                                t.batch_completed += 1;
                                t.sum_decode_preview_ms += deriv.timings.decode_preview_ms;
                                t.sum_encode_preview_ms += deriv.timings.encode_preview_ms;
                                t.sum_thumb_ms += deriv.timings.thumb_ms;
                                t.sum_total_ms += deriv.timings.total_ms;
                                t.recent_completions.push_back(now);
                                let cutoff = now - Duration::from_secs(5);
                                while matches!(t.recent_completions.front(), Some(&ts) if ts < cutoff)
                                {
                                    t.recent_completions.pop_front();
                                }

                                PhotoProcessUpdate::Ready {
                                    photo_id: task.job.photo_id,
                                    thumbnail_rel_path: deriv.thumbnail_rel_path,
                                    preview_rel_path: deriv.preview_rel_path,
                                    width: deriv.original_width,
                                    height: deriv.original_height,
                                }
                            }
                            Err(err) => PhotoProcessUpdate::Error {
                                photo_id: task.job.photo_id,
                                message: err.to_string(),
                            },
                        };

                        let _ = tx_w.send(CompletedOutcome {
                            bundle: task.bundle,
                            update: outcome,
                        });
                        in_flight_w.fetch_sub(1, Ordering::AcqRel);
                    }
                })
                .expect("failed to spawn imaging worker thread");
        }

        Self {
            worker_threads: workers,
            queue,
            in_flight,
            completed,
            errors,
            telemetry,
        }
    }

    pub fn enqueue(&self, bundle: &Arc<EventBundle>, jobs: Vec<PendingPhotoJob>) {
        if jobs.is_empty() {
            return;
        }
        {
            let mut t = self.telemetry.lock().unwrap();
            if t.batch_start.is_none() {
                t.batch_start = Some(Instant::now());
                t.batch_completed = 0;
            }
        }
        let mut guard = self.queue.items.lock().unwrap();
        for job in jobs {
            guard.push_back(QueuedTask {
                bundle: Arc::clone(bundle),
                job,
            });
        }
        drop(guard);
        self.queue.notify.notify_all();
    }

    pub fn metrics(&self) -> ProcessingMetrics {
        let queued = self.queue.items.lock().unwrap().len();
        let in_flight = self.in_flight.load(Ordering::Relaxed);
        let completed = self.completed.load(Ordering::Relaxed);
        let errors = self.errors.load(Ordering::Relaxed);

        let now = Instant::now();
        let mut t = self.telemetry.lock().unwrap();
        let cutoff = now - Duration::from_secs(5);
        while matches!(t.recent_completions.front(), Some(&ts) if ts < cutoff) {
            t.recent_completions.pop_front();
        }

        let photos_per_sec = if t.recent_completions.len() >= 2 {
            let span = now
                .duration_since(*t.recent_completions.front().unwrap())
                .as_secs_f64()
                .max(0.25);
            t.recent_completions.len() as f64 / span
        } else if let Some(start) = t.batch_start {
            let elapsed = start.elapsed().as_secs_f64();
            if elapsed > 0.2 && t.batch_completed > 0 {
                t.batch_completed as f64 / elapsed
            } else {
                0.0
            }
        } else {
            0.0
        };

        let remaining = queued + in_flight;
        let eta_seconds = (remaining > 0 && photos_per_sec > 0.1)
            .then(|| remaining as f64 / photos_per_sec);
        let avg = |sum: f64, count: u64| if count > 0 { sum / count as f64 } else { 0.0 };

        ProcessingMetrics {
            engine: active_engine_name().to_string(),
            worker_threads: self.worker_threads,
            in_flight,
            queued,
            completed,
            errors,
            photos_per_sec,
            eta_seconds,
            avg_decode_preview_ms: avg(t.sum_decode_preview_ms, t.samples),
            avg_encode_preview_ms: avg(t.sum_encode_preview_ms, t.samples),
            avg_thumb_ms: avg(t.sum_thumb_ms, t.samples),
            avg_total_ms: avg(t.sum_total_ms, t.samples),
            avg_db_flush_ms: avg(t.sum_db_flush_ms, t.db_flushes),
        }
    }
}

fn flush_outcomes_by_bundle(
    pending: &mut Vec<CompletedOutcome>,
    completed_counter: &AtomicU64,
    errors_counter: &AtomicU64,
) {
    let mut grouped: Vec<(Arc<EventBundle>, Vec<PhotoProcessUpdate>)> = Vec::new();
    for item in pending.drain(..) {
        if let Some((_, list)) = grouped
            .iter_mut()
            .find(|(b, _)| Arc::ptr_eq(b, &item.bundle))
        {
            list.push(item.update);
        } else {
            grouped.push((item.bundle, vec![item.update]));
        }
    }

    for (bundle, updates) in grouped {
        let ready_n = updates
            .iter()
            .filter(|u| matches!(u, PhotoProcessUpdate::Ready { .. }))
            .count() as u64;
        let err_n = updates.len() as u64 - ready_n;
        if bundle.mark_photos_batch(&updates).is_ok() {
            completed_counter.fetch_add(ready_n, Ordering::Relaxed);
            errors_counter.fetch_add(err_n, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb};
    use tempfile::tempdir;

    #[test]
    fn generates_thumbnail_and_preview_from_jpeg() {
        let dir = tempdir().unwrap();
        let rel = "Gym A/Session 1A/sample.jpg";
        let src_path = dir.path().join(rel);
        fs::create_dir_all(src_path.parent().unwrap()).unwrap();

        let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_fn(2000, 1000, |x, y| Rgb([(x % 255) as u8, (y % 255) as u8, 128]));
        img.save(&src_path).unwrap();

        let deriv = generate_derivatives(dir.path(), "pho_test0001aa", rel).unwrap();
        assert_eq!(deriv.original_width, 2000);
        assert_eq!(deriv.original_height, 1000);

        let thumb = image::open(dir.path().join(&deriv.thumbnail_rel_path)).unwrap();
        assert_eq!((thumb.width(), thumb.height()), (320, 160));

        let prev = image::open(dir.path().join(&deriv.preview_rel_path)).unwrap();
        assert_eq!((prev.width(), prev.height()), (1280, 640));

        let mut resizer = Resizer::new();
        let deriv_rust =
            generate_derivatives_pure_rust(dir.path(), "pho_test0002bb", rel, &mut resizer)
                .unwrap();
        assert_eq!((deriv_rust.original_width, deriv_rust.original_height), (2000, 1000));
        let prev_rust = image::open(dir.path().join(&deriv_rust.preview_rel_path)).unwrap();
        assert_eq!((prev_rust.width(), prev_rust.height()), (1280, 640));
    }
}
