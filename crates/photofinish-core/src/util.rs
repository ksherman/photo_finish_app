use chrono::Utc;
use std::path::Path;

use crate::id;

/// Returns `Some(trimmed)` when `opt` contains non-whitespace characters.
pub fn trim_non_empty(opt: Option<&str>) -> Option<&str> {
    opt.map(str::trim).filter(|s| !s.is_empty())
}

/// Owned version of [`trim_non_empty`].
pub fn trim_owned(opt: Option<&str>) -> Option<String> {
    trim_non_empty(opt).map(str::to_string)
}

/// Current UTC timestamp formatted as RFC 3339.
pub fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

/// Returns true if `path` has a `.jpg` or `.jpeg` extension (case-insensitive).
pub fn is_jpeg_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("jpg") || ext.eq_ignore_ascii_case("jpeg"))
        .unwrap_or(false)
}

/// Splits a relative folder path into `(parent_path, leaf_folder)`.
pub fn split_parent_and_leaf(folder_path: &str) -> (&str, &str) {
    folder_path.rsplit_once('/').unwrap_or(("", folder_path))
}

/// Returns an exclusive B-tree range `[start, end)` matching all descendants under `prefix/`.
/// Relies on ASCII `'0'` being `'/' + 1`.
pub fn folder_prefix_range(prefix: &str) -> (String, String) {
    (format!("{}/", prefix), format!("{}0", prefix))
}

/// Replaces filesystem-unsafe characters in a single folder or filename component.
pub fn sanitize_folder_component(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// Returns a collision-free filename inside `dir`, appending `_2`, `_3`, ... if needed.
pub fn unique_filename_in_dir(dir: &Path, filename: &str) -> String {
    if !dir.join(filename).exists() {
        return filename.to_string();
    }
    let path = Path::new(filename);
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| filename.to_string());
    let ext = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    for n in 2..10_000 {
        let candidate = format!("{}_{}{}", stem, n, ext);
        if !dir.join(&candidate).exists() {
            return candidate;
        }
    }
    format!("{}_{}{}", stem, id::generate("dup_"), ext)
}
