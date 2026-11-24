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
///
/// On macOS, uses Apple's native ImageIO hardware DCT shrink-on-load + CoreGraphics cascade
/// (zero external libraries), automatically falling back to the optimized pure-Rust SIMD
/// pipeline if ImageIO fails on an unusual file.
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
        match macos_imageio::generate_derivatives_imageio(event_root, photo_id, source_rel_path) {
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
/// 1. Resizes full-res RGB -> 1280px BEFORE applying EXIF orientation (~120x faster rotation
///    on portrait photos because it rotates ~1MP instead of 24MP).
/// 2. Cascades 1280px -> 320px from the already-oriented preview (~25x faster thumb resize).
/// 3. Reuses the caller's thread-local `Resizer` to avoid scratch buffer reallocations.
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
    let img = DynamicImage::from_decoder(decoder).map_err(|e| Error::Image(e.to_string()))?;
    let rgb = img.into_rgb8();
    let (raw_w, raw_h) = rgb.dimensions();

    // Compute final oriented original dimensions for DB metadata
    let (orig_w, orig_h) = match orientation {
        Orientation::Rotate90
        | Orientation::Rotate270
        | Orientation::Rotate90FlipH
        | Orientation::Rotate270FlipH => (raw_h, raw_w),
        _ => (raw_w, raw_h),
    };

    let opts = ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::CatmullRom));

    // Step 1: Resize to 1280px FIRST while still unrotated
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

    // Step 2: Apply EXIF orientation on the small 1280px preview
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

    // Step 3: Encode 1280px preview JPEG
    let s1 = Instant::now();
    encode_rgb_jpeg(
        &prev_buf,
        final_pw,
        final_ph,
        PREVIEW_QUALITY,
        &event_root.join(&prev_rel),
    )?;
    let encode_preview_ms = s1.elapsed().as_secs_f64() * 1000.0;

    // Step 4: Cascade 320px thumbnail from the already-oriented 1280px preview
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
    encode_rgb_jpeg(
        &thumb_buf,
        tw,
        th,
        THUMBNAIL_QUALITY,
        &event_root.join(&thumb_rel),
    )?;
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

/// Processes a single photo record synchronously (useful in tests and one-off calls).
pub fn process_photo_in_bundle(
    bundle: &EventBundle,
    photo_id: &str,
    relative_path: &str,
) -> Result<GeneratedDerivatives> {
    match generate_derivatives(bundle.root(), photo_id, relative_path) {
        Ok(deriv) => {
            bundle.mark_photo_ready(
                photo_id,
                &deriv.thumbnail_rel_path,
                &deriv.preview_rel_path,
                deriv.original_width,
                deriv.original_height,
            )?;
            Ok(deriv)
        }
        Err(err) => {
            let _ = bundle.mark_photo_error(photo_id, &err.to_string());
            Err(err)
        }
    }
}

fn encode_rgb_jpeg(
    rgb: &[u8],
    w: u32,
    h: u32,
    quality: u8,
    output_path: &Path,
) -> Result<()> {
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
// Dedicated Background ImagePipeline (Fixed Worker Pool + Batched DB Flusher + Live Telemetry)
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

/// High-throughput background imaging pipeline with:
/// - Dedicated OS worker threads (reusing per-thread `Resizer` buffers)
/// - Single batched SQLite flusher (commits up to 64 photo updates per transaction)
/// - Live rolling throughput, ETA, and per-stage timing telemetry
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

        // Dedicated DB batch flusher thread: groups up to 64 results (or 150ms window) per transaction
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
                        let first = if pending.is_empty() {
                            match result_rx.recv() {
                                Ok(item) => Some(item),
                                Err(_) => break,
                            }
                        } else {
                            None
                        };

                        if let Some(item) = first {
                            pending.push(item);
                        }

                        let deadline = Instant::now() + DB_FLUSH_MAX_WAIT;
                        while pending.len() < DB_FLUSH_BATCH_SIZE {
                            let remaining = deadline.saturating_duration_since(Instant::now());
                            if remaining.is_zero() {
                                break;
                            }
                            match result_rx.recv_timeout(remaining) {
                                Ok(item) => pending.push(item),
                                Err(mpsc::RecvTimeoutError::Timeout) => break,
                                Err(mpsc::RecvTimeoutError::Disconnected) => break,
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

                        // Log a crisp summary whenever the queue completely drains
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

        // Fixed pool of imaging worker threads (each keeps its own reusable Resizer)
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
        let eta_seconds = if remaining > 0 && photos_per_sec > 0.1 {
            Some(remaining as f64 / photos_per_sec)
        } else {
            None
        };

        let samples = t.samples.max(1) as f64;
        let flushes = t.db_flushes.max(1) as f64;

        ProcessingMetrics {
            engine: active_engine_name().to_string(),
            worker_threads: self.worker_threads,
            in_flight,
            queued,
            completed,
            errors,
            photos_per_sec,
            eta_seconds,
            avg_decode_preview_ms: if t.samples > 0 {
                t.sum_decode_preview_ms / samples
            } else {
                0.0
            },
            avg_encode_preview_ms: if t.samples > 0 {
                t.sum_encode_preview_ms / samples
            } else {
                0.0
            },
            avg_thumb_ms: if t.samples > 0 {
                t.sum_thumb_ms / samples
            } else {
                0.0
            },
            avg_total_ms: if t.samples > 0 {
                t.sum_total_ms / samples
            } else {
                0.0
            },
            avg_db_flush_ms: if t.db_flushes > 0 {
                t.sum_db_flush_ms / flushes
            } else {
                0.0
            },
        }
    }
}

fn flush_outcomes_by_bundle(
    pending: &mut Vec<CompletedOutcome>,
    completed_counter: &AtomicU64,
    errors_counter: &AtomicU64,
) {
    let drained: Vec<CompletedOutcome> = pending.drain(..).collect();
    let mut grouped: Vec<(Arc<EventBundle>, Vec<PhotoProcessUpdate>)> = Vec::new();

    for item in drained {
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
        let mut ready_n = 0u64;
        let mut err_n = 0u64;
        for u in &updates {
            match u {
                PhotoProcessUpdate::Ready { .. } => ready_n += 1,
                PhotoProcessUpdate::Error { .. } => err_n += 1,
            }
        }
        if bundle.mark_photos_batch(&updates).is_ok() {
            completed_counter.fetch_add(ready_n, Ordering::Relaxed);
            errors_counter.fetch_add(err_n, Ordering::Relaxed);
        }
    }
}

// ============================================================================
// macOS Native ImageIO + CoreGraphics Hardware Engine (Zero External Dependencies)
// ============================================================================

#[cfg(target_os = "macos")]
mod macos_imageio {
    use std::ffi::c_void;
    use std::fs;
    use std::os::raw::{c_char, c_int};
    use std::path::Path;
    use std::ptr;
    use std::time::Instant;

    use super::{
        fit_dimensions, GeneratedDerivatives, StageTimingsMs, PREVIEW_MAX_EDGE, PREVIEW_QUALITY,
        THUMBNAIL_MAX_EDGE, THUMBNAIL_QUALITY,
    };
    use crate::db::{BUNDLE_DIR_NAME, PREVIEWS_DIR_NAME, THUMBNAILS_DIR_NAME};
    use crate::{Error, Result};

    type CFTypeRef = *const c_void;
    type CFAllocatorRef = *const c_void;
    type CFStringRef = *const c_void;
    type CFURLRef = *const c_void;
    type CFDictionaryRef = *const c_void;
    type CFNumberRef = *const c_void;
    type CFBooleanRef = *const c_void;
    type CGImageSourceRef = *const c_void;
    type CGImageDestinationRef = *const c_void;
    type CGImageRef = *const c_void;
    type CGColorSpaceRef = *const c_void;
    type CGContextRef = *const c_void;

    const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
    const K_CF_NUMBER_SINT32_TYPE: c_int = 3;
    const K_CF_NUMBER_FLOAT64_TYPE: c_int = 6;
    const K_CG_IMAGE_ALPHA_NONE_SKIP_LAST: u32 = 5;
    const K_CG_BITMAP_BYTE_ORDER_DEFAULT: u32 = 0;
    const K_CG_INTERPOLATION_HIGH: c_int = 3;

    #[repr(C)]
    #[derive(Copy, Clone)]
    struct CGPoint {
        x: f64,
        y: f64,
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    struct CGSize {
        width: f64,
        height: f64,
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    struct CGRect {
        origin: CGPoint,
        size: CGSize,
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        static kCFBooleanTrue: CFBooleanRef;
        static kCFTypeDictionaryKeyCallBacks: c_void;
        static kCFTypeDictionaryValueCallBacks: c_void;
        fn CFRelease(cf: CFTypeRef);
        fn CFStringCreateWithBytes(
            alloc: CFAllocatorRef,
            bytes: *const u8,
            num_bytes: isize,
            encoding: u32,
            is_external: bool,
        ) -> CFStringRef;
        fn CFURLCreateFromFileSystemRepresentation(
            alloc: CFAllocatorRef,
            buffer: *const c_char,
            buf_len: isize,
            is_directory: bool,
        ) -> CFURLRef;
        fn CFNumberCreate(
            alloc: CFAllocatorRef,
            the_type: c_int,
            value_ptr: *const c_void,
        ) -> CFNumberRef;
        fn CFNumberGetValue(
            number: CFNumberRef,
            the_type: c_int,
            value_ptr: *mut c_void,
        ) -> bool;
        fn CFDictionaryCreate(
            alloc: CFAllocatorRef,
            keys: *const CFTypeRef,
            values: *const CFTypeRef,
            num_values: isize,
            key_callbacks: *const c_void,
            value_callbacks: *const c_void,
        ) -> CFDictionaryRef;
        fn CFDictionaryGetValue(dict: CFDictionaryRef, key: CFTypeRef) -> CFTypeRef;
    }

    #[link(name = "ImageIO", kind = "framework")]
    extern "C" {
        static kCGImageSourceCreateThumbnailFromImageAlways: CFStringRef;
        static kCGImageSourceCreateThumbnailWithTransform: CFStringRef;
        static kCGImageSourceThumbnailMaxPixelSize: CFStringRef;
        static kCGImageSourceShouldCacheImmediately: CFStringRef;
        static kCGImageDestinationLossyCompressionQuality: CFStringRef;
        static kCGImagePropertyPixelWidth: CFStringRef;
        static kCGImagePropertyPixelHeight: CFStringRef;
        static kCGImagePropertyOrientation: CFStringRef;

        fn CGImageSourceCreateWithURL(url: CFURLRef, options: CFDictionaryRef) -> CGImageSourceRef;
        fn CGImageSourceCopyPropertiesAtIndex(
            isrc: CGImageSourceRef,
            index: usize,
            options: CFDictionaryRef,
        ) -> CFDictionaryRef;
        fn CGImageSourceCreateThumbnailAtIndex(
            isrc: CGImageSourceRef,
            index: usize,
            options: CFDictionaryRef,
        ) -> CGImageRef;
        fn CGImageDestinationCreateWithURL(
            url: CFURLRef,
            type_uti: CFStringRef,
            count: usize,
            options: CFDictionaryRef,
        ) -> CGImageDestinationRef;
        fn CGImageDestinationAddImage(
            idst: CGImageDestinationRef,
            image: CGImageRef,
            properties: CFDictionaryRef,
        );
        fn CGImageDestinationFinalize(idst: CGImageDestinationRef) -> bool;
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGImageRelease(image: CGImageRef);
        fn CGImageGetWidth(image: CGImageRef) -> usize;
        fn CGImageGetHeight(image: CGImageRef) -> usize;
        fn CGColorSpaceCreateDeviceRGB() -> CGColorSpaceRef;
        fn CGColorSpaceRelease(space: CGColorSpaceRef);
        fn CGBitmapContextCreate(
            data: *mut c_void,
            width: usize,
            height: usize,
            bits_per_component: usize,
            bytes_per_row: usize,
            space: CGColorSpaceRef,
            bitmap_info: u32,
        ) -> CGContextRef;
        fn CGContextSetInterpolationQuality(c: CGContextRef, quality: c_int);
        fn CGContextDrawImage(c: CGContextRef, rect: CGRect, image: CGImageRef);
        fn CGBitmapContextCreateImage(c: CGContextRef) -> CGImageRef;
        fn CGContextRelease(c: CGContextRef);
    }

    struct CfGuard(CFTypeRef);
    impl Drop for CfGuard {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { CFRelease(self.0) };
            }
        }
    }

    struct CgImageGuard(CGImageRef);
    impl Drop for CgImageGuard {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { CGImageRelease(self.0) };
            }
        }
    }

    fn make_cf_url(path: &Path) -> Result<CfGuard> {
        let bytes = path.as_os_str().as_encoded_bytes();
        let url = unsafe {
            CFURLCreateFromFileSystemRepresentation(
                ptr::null(),
                bytes.as_ptr() as *const c_char,
                bytes.len() as isize,
                false,
            )
        };
        if url.is_null() {
            Err(Error::Io(format!("invalid path for CFURL: {}", path.display())))
        } else {
            Ok(CfGuard(url))
        }
    }

    fn make_cf_string(s: &str) -> Result<CfGuard> {
        let cf = unsafe {
            CFStringCreateWithBytes(
                ptr::null(),
                s.as_ptr(),
                s.len() as isize,
                K_CF_STRING_ENCODING_UTF8,
                false,
            )
        };
        if cf.is_null() {
            Err(Error::Image("failed to allocate CFString".into()))
        } else {
            Ok(CfGuard(cf))
        }
    }

    fn get_dict_i32(dict: CFDictionaryRef, key: CFStringRef) -> Option<i32> {
        unsafe {
            let val = CFDictionaryGetValue(dict, key as CFTypeRef);
            if val.is_null() {
                return None;
            }
            let mut out: i32 = 0;
            if CFNumberGetValue(val as CFNumberRef, K_CF_NUMBER_SINT32_TYPE, &mut out as *mut i32 as *mut c_void) {
                Some(out)
            } else {
                None
            }
        }
    }

    fn write_cg_jpeg(cg_img: CGImageRef, dst: &Path, quality: f64) -> Result<()> {
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| Error::Io(format!("{}: {}", parent.display(), e)))?;
        }
        let uti = make_cf_string("public.jpeg")?;
        let url = make_cf_url(dst)?;
        let dest = CfGuard(unsafe {
            CGImageDestinationCreateWithURL(url.0, uti.0, 1, ptr::null())
        });
        if dest.0.is_null() {
            return Err(Error::Image(format!(
                "CGImageDestinationCreateWithURL failed for {}",
                dst.display()
            )));
        }

        let q_num = CfGuard(unsafe {
            CFNumberCreate(
                ptr::null(),
                K_CF_NUMBER_FLOAT64_TYPE,
                &quality as *const f64 as *const c_void,
            )
        });
        let keys = unsafe { [kCGImageDestinationLossyCompressionQuality as CFTypeRef] };
        let vals = [q_num.0];
        let props = CfGuard(unsafe {
            CFDictionaryCreate(
                ptr::null(),
                keys.as_ptr(),
                vals.as_ptr(),
                1,
                &kCFTypeDictionaryKeyCallBacks,
                &kCFTypeDictionaryValueCallBacks,
            )
        });

        let ok = unsafe {
            CGImageDestinationAddImage(dest.0, cg_img, props.0);
            CGImageDestinationFinalize(dest.0)
        };
        if !ok {
            return Err(Error::Image(format!(
                "CGImageDestinationFinalize failed for {}",
                dst.display()
            )));
        }
        Ok(())
    }

    fn downscale_cg_image(src: CGImageRef, dst_w: usize, dst_h: usize) -> Result<CgImageGuard> {
        unsafe {
            let space = CGColorSpaceCreateDeviceRGB();
            if space.is_null() {
                return Err(Error::Image("CGColorSpaceCreateDeviceRGB failed".into()));
            }
            let ctx = CGBitmapContextCreate(
                ptr::null_mut(),
                dst_w,
                dst_h,
                8,
                dst_w * 4,
                space,
                K_CG_IMAGE_ALPHA_NONE_SKIP_LAST | K_CG_BITMAP_BYTE_ORDER_DEFAULT,
            );
            CGColorSpaceRelease(space);
            if ctx.is_null() {
                return Err(Error::Image("CGBitmapContextCreate failed".into()));
            }

            CGContextSetInterpolationQuality(ctx, K_CG_INTERPOLATION_HIGH);
            let rect = CGRect {
                origin: CGPoint { x: 0.0, y: 0.0 },
                size: CGSize {
                    width: dst_w as f64,
                    height: dst_h as f64,
                },
            };
            CGContextDrawImage(ctx, rect, src);
            let out = CGBitmapContextCreateImage(ctx);
            CGContextRelease(ctx);
            if out.is_null() {
                return Err(Error::Image("CGBitmapContextCreateImage failed".into()));
            }
            Ok(CgImageGuard(out))
        }
    }

    pub fn generate_derivatives_imageio(
        event_root: &Path,
        photo_id: &str,
        source_rel_path: &str,
    ) -> Result<GeneratedDerivatives> {
        let total_start = Instant::now();
        let source_path = event_root.join(source_rel_path);
        if !source_path.is_file() {
            return Err(Error::Io(format!("file not found: {}", source_path.display())));
        }

        let s0 = Instant::now();
        let url = make_cf_url(&source_path)?;
        let src = CfGuard(unsafe { CGImageSourceCreateWithURL(url.0, ptr::null()) });
        if src.0.is_null() {
            return Err(Error::Image(format!(
                "CGImageSourceCreateWithURL failed: {}",
                source_path.display()
            )));
        }

        // Fast header-only read of original pixel dimensions + EXIF orientation
        let (mut orig_w, mut orig_h) = (0u32, 0u32);
        let props = CfGuard(unsafe { CGImageSourceCopyPropertiesAtIndex(src.0, 0, ptr::null()) });
        if !props.0.is_null() {
            let raw_w = unsafe { get_dict_i32(props.0, kCGImagePropertyPixelWidth) }.unwrap_or(0);
            let raw_h = unsafe { get_dict_i32(props.0, kCGImagePropertyPixelHeight) }.unwrap_or(0);
            let orient = unsafe { get_dict_i32(props.0, kCGImagePropertyOrientation) }.unwrap_or(1);
            if matches!(orient, 5 | 6 | 7 | 8) {
                orig_w = raw_h.max(0) as u32;
                orig_h = raw_w.max(0) as u32;
            } else {
                orig_w = raw_w.max(0) as u32;
                orig_h = raw_h.max(0) as u32;
            }
        }

        // Single hardware DCT shrink-on-load decode to 1280px with EXIF orientation applied
        let max_px = PREVIEW_MAX_EDGE as i32;
        let max_num = CfGuard(unsafe {
            CFNumberCreate(
                ptr::null(),
                K_CF_NUMBER_SINT32_TYPE,
                &max_px as *const i32 as *const c_void,
            )
        });
        let keys = unsafe {
            [
                kCGImageSourceCreateThumbnailFromImageAlways as CFTypeRef,
                kCGImageSourceCreateThumbnailWithTransform as CFTypeRef,
                kCGImageSourceThumbnailMaxPixelSize as CFTypeRef,
                kCGImageSourceShouldCacheImmediately as CFTypeRef,
            ]
        };
        let vals = unsafe {
            [
                kCFBooleanTrue as CFTypeRef,
                kCFBooleanTrue as CFTypeRef,
                max_num.0,
                kCFBooleanTrue as CFTypeRef,
            ]
        };
        let opts = CfGuard(unsafe {
            CFDictionaryCreate(
                ptr::null(),
                keys.as_ptr(),
                vals.as_ptr(),
                4,
                &kCFTypeDictionaryKeyCallBacks,
                &kCFTypeDictionaryValueCallBacks,
            )
        });

        let prev_img = CgImageGuard(unsafe { CGImageSourceCreateThumbnailAtIndex(src.0, 0, opts.0) });
        if prev_img.0.is_null() {
            return Err(Error::Image(format!(
                "CGImageSourceCreateThumbnailAtIndex failed: {}",
                source_path.display()
            )));
        }
        let pw = unsafe { CGImageGetWidth(prev_img.0) } as u32;
        let ph = unsafe { CGImageGetHeight(prev_img.0) } as u32;
        if orig_w == 0 || orig_h == 0 {
            orig_w = pw;
            orig_h = ph;
        }
        let decode_preview_ms = s0.elapsed().as_secs_f64() * 1000.0;

        let thumb_rel = format!("{}/{}/{}.jpg", BUNDLE_DIR_NAME, THUMBNAILS_DIR_NAME, photo_id);
        let prev_rel = format!("{}/{}/{}.jpg", BUNDLE_DIR_NAME, PREVIEWS_DIR_NAME, photo_id);

        // Hardware JPEG encode for 1280px preview
        let s1 = Instant::now();
        write_cg_jpeg(
            prev_img.0,
            &event_root.join(&prev_rel),
            PREVIEW_QUALITY as f64 / 100.0,
        )?;
        let encode_preview_ms = s1.elapsed().as_secs_f64() * 1000.0;

        // Cascade 320px thumbnail directly from the in-memory 1280px CGImage (zero 2nd file decode!)
        let s2 = Instant::now();
        let (tw, th) = fit_dimensions(pw, ph, THUMBNAIL_MAX_EDGE);
        if tw == pw && th == ph {
            write_cg_jpeg(
                prev_img.0,
                &event_root.join(&thumb_rel),
                THUMBNAIL_QUALITY as f64 / 100.0,
            )?;
        } else {
            let thumb_img = downscale_cg_image(prev_img.0, tw as usize, th as usize)?;
            write_cg_jpeg(
                thumb_img.0,
                &event_root.join(&thumb_rel),
                THUMBNAIL_QUALITY as f64 / 100.0,
            )?;
        }
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

        // Test primary engine (ImageIO on macOS, pure Rust elsewhere)
        let deriv = generate_derivatives(dir.path(), "pho_test0001aa", rel).unwrap();
        assert_eq!(deriv.original_width, 2000);
        assert_eq!(deriv.original_height, 1000);

        let thumb = image::open(dir.path().join(&deriv.thumbnail_rel_path)).unwrap();
        assert_eq!(thumb.width(), 320);
        assert_eq!(thumb.height(), 160);

        let prev = image::open(dir.path().join(&deriv.preview_rel_path)).unwrap();
        assert_eq!(prev.width(), 1280);
        assert_eq!(prev.height(), 640);

        // Also explicitly verify the pure-Rust fallback engine
        let mut resizer = Resizer::new();
        let deriv_rust =
            generate_derivatives_pure_rust(dir.path(), "pho_test0002bb", rel, &mut resizer)
                .unwrap();
        assert_eq!(deriv_rust.original_width, 2000);
        assert_eq!(deriv_rust.original_height, 1000);
        let prev_rust = image::open(dir.path().join(&deriv_rust.preview_rel_path)).unwrap();
        assert_eq!(prev_rust.width(), 1280);
        assert_eq!(prev_rust.height(), 640);
    }
}
