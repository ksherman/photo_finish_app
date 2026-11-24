#[cfg(target_os = "macos")]
mod macos;

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::event::IngestFileInput;
use crate::util::{is_jpeg_path, unique_filename_in_dir};
use crate::{Error, Result};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CardReaderInfo {
    /// Stable identity: USB hardware serial, "sdxc-builtin", or DeviceTreePath fallback.
    pub reader_id: String,
    pub usb_serial: Option<String>,
    pub display_name: String,
    pub mount_point: String,
    pub volume_name: String,
    pub bus_protocol: String,
    pub is_internal: bool,
    pub disk_id: String,
    pub file_count: u32,
    pub folder_count: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DirectoryEntry {
    pub name: String,
    pub path: String,
    pub is_directory: bool,
    pub file_count: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CopyProgressEvent {
    pub total: u32,
    pub copied: u32,
    pub current_file: String,
    pub percentage: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UsbDriveInfo {
    pub disk_id: String,
    pub volume_name: String,
    pub mount_point: String,
    pub bus_protocol: String,
    pub total_bytes: u64,
    pub free_bytes: u64,
    /// True if the volume has a top-level `DCIM/` directory (camera SD card warning).
    pub has_dcim: bool,
}

#[derive(Debug, Clone)]
pub struct UsbCopyItem {
    pub source_abs_path: PathBuf,
    pub dest_rel_path: String,
    pub file_size_bytes: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UsbCopyProgressEvent {
    pub total_files: u32,
    pub copied_files: u32,
    pub total_bytes: u64,
    pub copied_bytes: u64,
    pub current_file: String,
    pub mb_per_sec: f64,
    pub percentage: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UsbCopyResult {
    pub destination_dir: String,
    pub files_copied: usize,
    pub bytes_copied: u64,
    pub elapsed_secs: f64,
    pub mb_per_sec: f64,
    pub ejected: bool,
    pub eject_warning: Option<String>,
    pub items_marked_fulfilled: usize,
}

#[derive(Debug, Clone)]
pub struct CopyOptions {
    pub source_path: PathBuf,
    pub event_root: PathBuf,
    pub destination_rel_path: String,
    pub rename_prefix: Option<String>,
    pub auto_rename: bool,
    pub file_rename_prefix: Option<String>,
    pub included_folders: Option<Vec<String>>,
}

pub fn discover_card_readers() -> Result<Vec<CardReaderInfo>> {
    #[cfg(target_os = "macos")]
    {
        macos::discover_card_readers()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(Vec::new())
    }
}

pub fn discover_usb_drives() -> Result<Vec<UsbDriveInfo>> {
    #[cfg(target_os = "macos")]
    {
        macos::discover_usb_drives()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(Vec::new())
    }
}

pub fn eject_drive(disk_id: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        macos::eject_drive(disk_id)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = disk_id;
        Err(Error::Io("Drive ejection is not implemented on this OS yet".into()))
    }
}

/// Copies a resolved list of order photos onto a destination USB drive or folder,
/// emitting throttled progress telemetry with live MB/s write speed.
pub fn copy_order_files_to_usb<F>(
    destination_dir: &Path,
    items: &[UsbCopyItem],
    mut on_progress: F,
) -> Result<(usize, u64, f64)>
where
    F: FnMut(UsbCopyProgressEvent),
{
    if !destination_dir.exists() {
        fs::create_dir_all(destination_dir).map_err(|e| {
            Error::Io(format!(
                "Destination '{}' could not be created: {}",
                destination_dir.display(),
                e
            ))
        })?;
    }
    if !destination_dir.is_dir() {
        return Err(Error::InvalidInput(format!(
            "Destination '{}' is not a directory",
            destination_dir.display()
        )));
    }

    let total_files = items.len() as u32;
    let total_bytes: u64 = items.iter().map(|i| i.file_size_bytes).sum();
    let started = std::time::Instant::now();
    let mut last_emit = std::time::Instant::now();
    let mut copied_files = 0u32;
    let mut copied_bytes = 0u64;

    for item in items {
        let mut dest_path = destination_dir.join(&item.dest_rel_path);
        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| Error::Io(format!("{}: {}", parent.display(), e)))?;
        }

        // Avoid silent overwrites if two photos collide on the same destination path
        if dest_path.exists() {
            let parent = dest_path.parent().unwrap_or(destination_dir);
            let filename = dest_path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "photo.jpg".into());
            dest_path = parent.join(unique_filename_in_dir(parent, &filename));
        }

        let bytes = fs::copy(&item.source_abs_path, &dest_path).map_err(|e| {
            Error::Io(format!(
                "Failed to copy {} -> {}: {}",
                item.source_abs_path.display(),
                dest_path.display(),
                e
            ))
        })?;

        copied_files += 1;
        copied_bytes += bytes;

        if copied_files == total_files || last_emit.elapsed().as_millis() >= 80 {
            let elapsed = started.elapsed().as_secs_f64().max(0.001);
            let mb_per_sec = (copied_bytes as f64 / (1024.0 * 1024.0)) / elapsed;
            let percentage = if total_bytes > 0 {
                ((copied_bytes as f64 / total_bytes as f64) * 100.0).min(100.0) as f32
            } else if total_files > 0 {
                (copied_files as f32 / total_files as f32) * 100.0
            } else {
                100.0
            };
            on_progress(UsbCopyProgressEvent {
                total_files,
                copied_files,
                total_bytes,
                copied_bytes,
                current_file: dest_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default(),
                mb_per_sec,
                percentage,
            });
            last_emit = std::time::Instant::now();
        }
    }

    let elapsed_secs = started.elapsed().as_secs_f64().max(0.001);
    Ok((copied_files as usize, copied_bytes, elapsed_secs))
}

/// Resolves the photo folder root for a card mount or local source directory:
/// automatically enters `DCIM/` (or `dcim/`) when present on a camera card,
/// otherwise uses `path` directly.
pub fn resolve_photo_source_dir(path: &Path) -> PathBuf {
    for name in ["DCIM", "dcim"] {
        let dcim = path.join(name);
        if dcim.is_dir() {
            return dcim;
        }
    }
    path.to_path_buf()
}

pub fn list_directory(path: &Path) -> Result<Vec<DirectoryEntry>> {
    let mut entries = Vec::new();
    if let Ok(read_dir) = fs::read_dir(path) {
        for entry in read_dir.flatten() {
            let metadata = entry
                .metadata()
                .map_err(|e| Error::Io(format!("{}: {}", entry.path().display(), e)))?;
            let name = entry.file_name().to_string_lossy().to_string();

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

pub(crate) fn count_folders(path: &Path) -> u32 {
    if let Ok(read_dir) = fs::read_dir(path) {
        read_dir
            .flatten()
            .filter(|entry| {
                if let Ok(metadata) = entry.metadata() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    metadata.is_dir() && !name.starts_with('.')
                } else {
                    false
                }
            })
            .count() as u32
    } else {
        0
    }
}

pub(crate) fn count_jpeg_files(path: &Path) -> u32 {
    WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.') && is_jpeg_path(e.path()))
        .count() as u32
}

fn apply_folder_rename(relative_path: &Path, prefix: &str, re: &regex::Regex) -> PathBuf {
    if relative_path.components().count() < 2 {
        return relative_path.to_path_buf();
    }

    let mut result = PathBuf::new();
    for (index, component) in relative_path.components().enumerate() {
        if index == 0 {
            if let Some(folder_name) = component.as_os_str().to_str() {
                if let Some(captures) = re.captures(folder_name) {
                    if let Some(number) = captures.get(1) {
                        let num_str = number.as_str();
                        let last_two = if num_str.len() >= 2 {
                            &num_str[num_str.len() - 2..]
                        } else {
                            num_str
                        };
                        result.push(format!("{} {}", prefix, last_two));
                        continue;
                    }
                }
            }
        }
        result.push(component);
    }

    result
}

/// Copies JPEG files from a card/source folder into `<event_root>/<destination_rel_path>`
/// while applying optional folder toggles and folder/file auto-renaming.
/// Returns `IngestFileInput` entries with paths relative to `event_root` so the caller
/// can immediately register an `ingest_batch` and queue thumbnailing.
pub fn copy_files_into_event<F>(
    opts: &CopyOptions,
    mut on_progress: F,
) -> Result<Vec<IngestFileInput>>
where
    F: FnMut(CopyProgressEvent),
{
    let resolved_source = resolve_photo_source_dir(&opts.source_path);
    let source = &resolved_source;
    let clean_rel = opts.destination_rel_path.trim().trim_matches('/');
    let dest = if clean_rel.is_empty() {
        opts.event_root.clone()
    } else {
        opts.event_root.join(clean_rel)
    };

    let re_folder = regex::Regex::new(r"(\d{2,3})").map_err(|e| Error::Io(e.to_string()))?;
    let re_file = regex::Regex::new(r"(\d+)").map_err(|e| Error::Io(e.to_string()))?;

    fs::create_dir_all(&dest).map_err(|e| Error::Io(format!("{}: {}", dest.display(), e)))?;

    let files: Vec<PathBuf> = WalkDir::new(source)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            if e.file_name().to_string_lossy().starts_with('.') {
                return false;
            }
            if let Some(ref folders) = opts.included_folders {
                let relative = e.path().strip_prefix(source).unwrap_or(e.path());
                let mut components = relative.components();
                if let Some(first) = components.next() {
                    if components.next().is_some() {
                        let folder_name = first.as_os_str().to_string_lossy().to_string();
                        if !folders.contains(&folder_name) {
                            return false;
                        }
                    }
                }
            }
            e.file_type().is_file() && is_jpeg_path(e.path())
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    let total = files.len() as u32;
    let mut copied = 0u32;
    let mut last_update = std::time::Instant::now();
    let mut ingested = Vec::with_capacity(files.len());

    for file_path in files {
        let original_filename = file_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let relative = file_path.strip_prefix(source).unwrap_or(&file_path);

        let mut dest_path_buf = if opts.auto_rename && opts.rename_prefix.is_some() {
            let prefix = opts.rename_prefix.as_ref().unwrap();
            dest.join(apply_folder_rename(relative, prefix, &re_folder))
        } else {
            dest.join(relative)
        };

        if let Some(prefix) = opts
            .file_rename_prefix
            .as_deref()
            .filter(|s| !s.trim().is_empty())
        {
            if let Some(file_name) = dest_path_buf.file_name().and_then(|n| n.to_str()) {
                if let Some(caps) = re_file.find_iter(file_name).last() {
                    let number = caps.as_str();
                    let ext = dest_path_buf
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("");
                    let new_name = if ext.is_empty() {
                        format!("{}_{}", prefix, number)
                    } else {
                        format!("{}_{}.{}", prefix, number, ext)
                    };
                    dest_path_buf.set_file_name(new_name);
                }
            }
        }

        if let Some(parent) = dest_path_buf.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| Error::Io(format!("{}: {}", parent.display(), e)))?;
        }

        let bytes = fs::copy(&file_path, &dest_path_buf)
            .map_err(|e| Error::Io(format!("copy {}: {}", file_path.display(), e)))?;

        let event_rel = dest_path_buf
            .strip_prefix(&opts.event_root)
            .unwrap_or(&dest_path_buf)
            .to_string_lossy()
            .replace('\\', "/");

        ingested.push(IngestFileInput {
            relative_path: event_rel,
            original_filename,
            file_size_bytes: bytes as i64,
        });

        copied += 1;
        if copied == total || last_update.elapsed().as_millis() > 100 {
            on_progress(CopyProgressEvent {
                total,
                copied,
                current_file: dest_path_buf
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
                percentage: if total > 0 {
                    (copied as f32 / total as f32) * 100.0
                } else {
                    100.0
                },
            });
            last_update = std::time::Instant::now();
        }
    }

    Ok(ingested)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn copies_filters_and_renames_into_event_bundle() {
        let src = tempdir().unwrap();
        let event = tempdir().unwrap();

        let f1 = src.path().join("105MSDCF");
        let f2 = src.path().join("106MSDCF");
        fs::create_dir_all(&f1).unwrap();
        fs::create_dir_all(&f2).unwrap();
        fs::write(f1.join("IMG_0042.JPG"), b"fakejpg1").unwrap();
        fs::write(f2.join("IMG_0099.JPG"), b"fakejpg2").unwrap();

        let results = copy_files_into_event(
            &CopyOptions {
                source_path: src.path().to_path_buf(),
                event_root: event.path().to_path_buf(),
                destination_rel_path: "Gym A/Session 1A/Group 1A/Beam".into(),
                rename_prefix: Some("Gymnast".into()),
                auto_rename: true,
                file_rename_prefix: Some("Beam".into()),
                included_folders: Some(vec!["105MSDCF".into()]),
            },
            |_| {},
        )
        .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].relative_path,
            "Gym A/Session 1A/Group 1A/Beam/Gymnast 05/Beam_0042.JPG"
        );
        assert!(event.path().join(&results[0].relative_path).is_file());
    }
}
