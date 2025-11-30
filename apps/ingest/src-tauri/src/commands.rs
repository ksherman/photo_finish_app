use serde::Serialize;
use std::fs;
use std::path::Path;
use tauri::ipc::Channel;
use walkdir::WalkDir;

#[derive(Serialize)]
pub struct VolumeInfo {
    pub name: String,
    pub path: String,
    pub is_removable: bool,
}

#[derive(Serialize)]
pub struct DirectoryEntry {
    pub name: String,
    pub path: String,
    pub is_directory: bool,
    pub file_count: u32,
}

#[derive(Serialize, Clone)]
pub struct CopyProgressEvent {
    pub total: u32,
    pub copied: u32,
    pub current_file: String,
    pub percentage: f32,
}

#[tauri::command]
pub fn list_volumes() -> Result<Vec<VolumeInfo>, String> {
    let volumes_path = Path::new("/Volumes");
    let mut volumes = Vec::new();

    if let Ok(entries) = fs::read_dir(volumes_path) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let path = entry.path().to_string_lossy().to_string();

            // Skip Macintosh HD (system volume)
            let is_removable = name != "Macintosh HD";

            volumes.push(VolumeInfo {
                name,
                path,
                is_removable,
            });
        }
    }

    Ok(volumes)
}

#[tauri::command]
pub fn list_directory(path: String) -> Result<Vec<DirectoryEntry>, String> {
    let dir_path = Path::new(&path);
    let mut entries = Vec::new();

    if let Ok(read_dir) = fs::read_dir(dir_path) {
        for entry in read_dir.flatten() {
            let metadata = entry.metadata().map_err(|e| e.to_string())?;
            let name = entry.file_name().to_string_lossy().to_string();

            // Skip hidden files
            if name.starts_with('.') {
                continue;
            }

            let file_count = if metadata.is_dir() {
                count_jpeg_files(&entry.path())
            } else {
                0
            };

            entries.push(DirectoryEntry {
                name,
                path: entry.path().to_string_lossy().to_string(),
                is_directory: metadata.is_dir(),
                file_count,
            });
        }
    }

    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

#[tauri::command]
pub fn get_file_count(path: String) -> Result<u32, String> {
    Ok(count_jpeg_files(Path::new(&path)))
}

fn count_jpeg_files(path: &Path) -> u32 {
    WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| {
                    let ext_lower = ext.to_string_lossy().to_lowercase();
                    ext_lower == "jpg" || ext_lower == "jpeg"
                })
                .unwrap_or(false)
        })
        .count() as u32
}

#[tauri::command]
pub async fn copy_files_to_destination(
    source_path: String,
    destination_path: String,
    on_progress: Channel<CopyProgressEvent>,
) -> Result<u32, String> {
    let source = Path::new(&source_path);
    let dest = Path::new(&destination_path);

    // Create destination directory
    fs::create_dir_all(dest).map_err(|e| e.to_string())?;

    // Collect all JPEG files
    let files: Vec<_> = WalkDir::new(source)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| {
                    let ext_lower = ext.to_string_lossy().to_lowercase();
                    ext_lower == "jpg" || ext_lower == "jpeg"
                })
                .unwrap_or(false)
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    let total = files.len() as u32;
    let mut copied = 0u32;

    for file_path in files {
        // Preserve relative path structure
        let relative = file_path.strip_prefix(source).unwrap_or(&file_path);
        let dest_file = dest.join(relative);

        // Create parent directories
        if let Some(parent) = dest_file.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        // Copy file
        fs::copy(&file_path, &dest_file).map_err(|e| e.to_string())?;

        copied += 1;

        // Send progress update
        let _ = on_progress.send(CopyProgressEvent {
            total,
            copied,
            current_file: file_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            percentage: (copied as f32 / total as f32) * 100.0,
        });
    }

    Ok(copied)
}

#[tauri::command]
pub fn rename_folder(source_path: String, new_name: String) -> Result<String, String> {
    let source = Path::new(&source_path);
    let parent = source.parent().ok_or("Invalid path")?;
    let new_path = parent.join(&new_name);

    fs::rename(source, &new_path).map_err(|e| e.to_string())?;

    Ok(new_path.to_string_lossy().to_string())
}
