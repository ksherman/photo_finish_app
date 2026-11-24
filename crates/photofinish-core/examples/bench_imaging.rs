use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use fast_image_resize::Resizer;
use photofinish_core::imaging::{
    active_engine_name, generate_derivatives_pure_rust, generate_derivatives_with_resizer,
    StageTimingsMs,
};
use photofinish_core::util::is_jpeg_path;
use walkdir::WalkDir;

fn collect_jpegs(dir: &Path, limit: usize) -> Vec<PathBuf> {
    WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && is_jpeg_path(e.path()))
        .map(|e| e.path().to_path_buf())
        .take(limit)
        .collect()
}

fn run_bench<F>(label: &str, files: &[PathBuf], threads: usize, f: F)
where
    F: Fn(&Path, usize, &mut Resizer) -> StageTimingsMs + Send + Sync,
{
    let idx = Arc::new(AtomicUsize::new(0));
    let start = Instant::now();
    let mut sum_decode = 0.0;
    let mut sum_encode = 0.0;
    let mut sum_thumb = 0.0;
    let mut sum_total = 0.0;

    std::thread::scope(|s| {
        let mut handles = Vec::new();
        for _ in 0..threads {
            let idx = Arc::clone(&idx);
            let f = &f;
            handles.push(s.spawn(move || {
                let mut resizer = Resizer::new();
                let (mut d, mut e, mut t, mut tot) = (0.0, 0.0, 0.0, 0.0);
                loop {
                    let i = idx.fetch_add(1, Ordering::Relaxed);
                    if i >= files.len() {
                        break;
                    }
                    let st = f(&files[i], i, &mut resizer);
                    d += st.decode_preview_ms;
                    e += st.encode_preview_ms;
                    t += st.thumb_ms;
                    tot += st.total_ms;
                }
                (d, e, t, tot)
            }));
        }
        for h in handles {
            let (d, e, t, tot) = h.join().unwrap();
            sum_decode += d;
            sum_encode += e;
            sum_thumb += t;
            sum_total += tot;
        }
    });

    let wall = start.elapsed().as_secs_f64();
    let n = files.len() as f64;
    let pps = n / wall;
    println!(
        "\n=== {label} ({} photos on {threads} threads) ===\n  Wall: {wall:.2}s ({pps:.1} photos/sec | est. 7,000 photos: {:.1}s)\n  Per-photo avg CPU: {:.1} ms\n    decode+1280px: {:6.1} ms\n    encode 1280px: {:6.1} ms\n    cascade 320px: {:6.1} ms",
        files.len(),
        7000.0 / pps,
        sum_total / n,
        sum_decode / n,
        sum_encode / n,
        sum_thumb / n,
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let src_dir = PathBuf::from(
        args.get(1)
            .map(String::as_str)
            .unwrap_or("../../test-images/2024-xcel-regional/Gym A/Session 1A"),
    );
    let limit: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(200);
    let threads: usize = args
        .get(3)
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get().saturating_sub(2).max(2))
                .unwrap_or(4)
        });

    let files: Vec<PathBuf> = collect_jpegs(&src_dir, limit)
        .into_iter()
        .map(|p| std::fs::canonicalize(&p).unwrap_or(p))
        .collect();
    let out_primary = tempfile::tempdir().unwrap();
    let out_rust = tempfile::tempdir().unwrap();

    run_bench(
        &format!("Production Primary Engine ({})", active_engine_name()),
        &files,
        threads,
        |p, i, r| {
            generate_derivatives_with_resizer(
                out_primary.path(),
                &format!("pho_{i:06}"),
                p.to_str().unwrap(),
                r,
            )
            .map(|d| d.timings)
            .unwrap_or_else(|e| panic!("failed on {}: {e}", p.display()))
        },
    );

    run_bench(
        "Production Pure-Rust SIMD Fallback",
        &files,
        threads,
        |p, i, r| {
            generate_derivatives_pure_rust(out_rust.path(), &format!("pho_{i:06}"), p.to_str().unwrap(), r)
                .map(|d| d.timings)
                .unwrap()
        },
    );
}
