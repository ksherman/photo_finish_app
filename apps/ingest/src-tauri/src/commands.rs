use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::process::Command;
use tauri::ipc::Channel;
use walkdir::WalkDir;

#[derive(Serialize)]
pub struct VolumeInfo {
    pub name: String,
    pub path: String,
    pub is_removable: bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct CardReaderInfo {
    pub reader_id: String,          // DeviceTreePath - unique identifier
    pub display_name: String,       // Human-readable name
    pub mount_point: String,        // /Volumes/xxx
    pub volume_name: String,        // Name of mounted volume
    pub bus_protocol: String,       // "USB", "Secure Digital", etc.
    pub is_internal: bool,          // Built-in vs external
    pub disk_id: String,            // disk4, disk5, etc.
    pub file_count: u32,            // Number of JPEGs on card
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
            // Ignore hidden files (starts with .)
            if e.file_name().to_string_lossy().starts_with('.') {
                return false;
            }

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

fn apply_folder_rename(relative_path: &Path, prefix: &str, re: &regex::Regex) -> std::path::PathBuf {
    // If path is just a filename (no parent folder), don't rename
    if relative_path.components().count() < 2 {
        return relative_path.to_path_buf();
    }

    let mut result = std::path::PathBuf::new();

    for (index, component) in relative_path.components().enumerate() {
        if index == 0 {
            // Rename the first component (folder name)
            if let Some(folder_name) = component.as_os_str().to_str() {
                // Extract 2-3 digit number from folder name (e.g., "105MSDCF" → "05")
                if let Some(captures) = re.captures(folder_name) {
                    if let Some(number) = captures.get(1) {
                        let num_str = number.as_str();
                        // Take last 2 digits
                        let last_two = if num_str.len() >= 2 {
                            &num_str[num_str.len() - 2..]
                        } else {
                            num_str
                        };
                        let new_name = format!("{} {}", prefix, last_two);
                        result.push(new_name);
                        continue;
                    }
                }
            }
        }
        result.push(component);
    }

    result
}

#[tauri::command]
pub async fn copy_files_to_destination(
    source_path: String,
    destination_path: String,
    on_progress: Channel<CopyProgressEvent>,
    rename_prefix: Option<String>,
    auto_rename: bool,
) -> Result<u32, String> {
    // Run blocking IO on a separate thread
    tauri::async_runtime::spawn_blocking(move || {
        let source = Path::new(&source_path);
        let dest = Path::new(&destination_path);

        // Compile regex once
        let re = regex::Regex::new(r"(\d{2,3})").map_err(|e| e.to_string())?;

        // Create destination directory
        fs::create_dir_all(dest).map_err(|e| e.to_string())?;

        // Collect all JPEG files
        let files: Vec<_> = WalkDir::new(source)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                // Ignore hidden files
                if e.file_name().to_string_lossy().starts_with('.') {
                    return false;
                }

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
        let mut last_update = std::time::Instant::now();

        for file_path in files {
            // Preserve relative path structure
            let relative = file_path.strip_prefix(source).unwrap_or(&file_path);

            // Apply folder renaming if enabled
            let dest_file = if auto_rename && rename_prefix.is_some() {
                let prefix = rename_prefix.as_ref().unwrap();
                let renamed_path = apply_folder_rename(relative, prefix, &re);
                dest.join(renamed_path)
            } else {
                dest.join(relative)
            };

            // Create parent directories
            if let Some(parent) = dest_file.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }

            // Copy file
            fs::copy(&file_path, &dest_file).map_err(|e| e.to_string())?;

            copied += 1;

            // Send progress update (throttle to every 100ms or completion)
            if copied == total || last_update.elapsed().as_millis() > 100 {
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
                last_update = std::time::Instant::now();
            }
        }

        Ok(copied)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn rename_folder(source_path: String, new_name: String) -> Result<String, String> {
    let source = Path::new(&source_path);
    let parent = source.parent().ok_or("Invalid path")?;
    let new_path = parent.join(&new_name);

    fs::rename(source, &new_path).map_err(|e| e.to_string())?;

    Ok(new_path.to_string_lossy().to_string())
}

#[derive(Deserialize)]
struct DiskutilList {
    #[serde(rename = "AllDisksAndPartitions")]
    all_disks_and_partitions: Vec<DiskEntry>,
}

#[derive(Deserialize)]
struct DiskEntry {
    #[serde(rename = "DeviceIdentifier")]
    device_identifier: String,
    #[serde(rename = "Partitions", default)]
    partitions: Vec<PartitionEntry>,
}

#[derive(Deserialize)]
struct PartitionEntry {
    #[serde(rename = "DeviceIdentifier")]
    device_identifier: String,
    #[serde(rename = "MountPoint")]
    mount_point: Option<String>,
    #[serde(rename = "VolumeName")]
    volume_name: Option<String>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct DiskutilInfo {
    #[serde(rename = "DeviceTreePath")]
    device_tree_path: Option<String>,
    #[serde(rename = "BusProtocol")]
    bus_protocol: Option<String>,
    #[serde(rename = "Internal")]
    internal: Option<bool>,
    #[serde(rename = "Removable")]
    removable: Option<bool>,
    #[serde(rename = "RemovableMedia")]
    removable_media: Option<bool>,
    #[serde(rename = "VolumeName")]
    volume_name: Option<String>,
    #[serde(rename = "MountPoint")]
    mount_point: Option<String>,
    #[serde(rename = "MediaName")]
    media_name: Option<String>,
}

#[tauri::command]
pub fn discover_card_readers() -> Result<Vec<CardReaderInfo>, String> {
    let mut readers = Vec::new();

    // Get list of all disks
    let output = Command::new("diskutil")
        .args(["list", "-plist"])
        .output()
        .map_err(|e| format!("Failed to run diskutil list: {}", e))?;

    let disk_list: DiskutilList = plist::from_bytes(&output.stdout)
        .map_err(|e| format!("Failed to parse diskutil list: {}", e))?;

    // Check each disk for removable media
    for disk in disk_list.all_disks_and_partitions {
        // Get detailed info for this disk
        let info_output = Command::new("diskutil")
            .args(["info", "-plist", &disk.device_identifier])
            .output()
            .map_err(|e| format!("Failed to run diskutil info: {}", e))?;

        let disk_info: DiskutilInfo = match plist::from_bytes(&info_output.stdout) {
            Ok(info) => info,
            Err(_) => continue,
        };

        // Skip non-removable media (we want SD cards, USB drives, etc.)
        let is_removable = disk_info.removable.unwrap_or(false)
            || disk_info.removable_media.unwrap_or(false);

        if !is_removable {
            continue;
        }

        // Find mounted partitions for this disk
        for partition in &disk.partitions {
            if let Some(mount_point) = &partition.mount_point {
                // Get detailed info for this partition
                let part_output = Command::new("diskutil")
                    .args(["info", "-plist", &partition.device_identifier])
                    .output()
                    .map_err(|e| format!("Failed to run diskutil info: {}", e))?;

                let part_info: DiskutilInfo = match plist::from_bytes(&part_output.stdout) {
                    Ok(info) => info,
                    Err(_) => continue,
                };

                // Use the disk's DeviceTreePath as the reader identifier
                let reader_id = disk_info
                    .device_tree_path
                    .clone()
                    .unwrap_or_else(|| disk.device_identifier.clone());

                let volume_name = partition
                    .volume_name
                    .clone()
                    .or(part_info.volume_name.clone())
                    .unwrap_or_else(|| "Untitled".to_string());

                let bus_protocol = disk_info
                    .bus_protocol
                    .clone()
                    .unwrap_or_else(|| "Unknown".to_string());

                let is_internal = disk_info.internal.unwrap_or(false);

                // Build a human-readable display name
                let display_name = if is_internal {
                    format!("Built-in {} Reader", bus_protocol)
                } else {
                    format!("External {} Reader", bus_protocol)
                };

                // Count JPEG files on this volume
                let file_count = count_jpeg_files(Path::new(mount_point));

                readers.push(CardReaderInfo {
                    reader_id,
                    display_name,
                    mount_point: mount_point.clone(),
                    volume_name,
                    bus_protocol,
                    is_internal,
                    disk_id: disk.device_identifier.clone(),
                    file_count,
                });
            }
        }
    }

    Ok(readers)
}