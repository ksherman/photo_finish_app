#[cfg(target_os = "macos")]
mod macos;

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::event::IngestFileInput;
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
        .filter(|e| {
            if e.file_name().to_string_lossy().starts_with('.') {
                return false;
            }
            is_jpeg_path(e.path())
        })
        .count() as u32
}

fn is_jpeg_path(path: &Path) -> bool {
    path.extension()
        .map(|ext| {
            let ext_lower = ext.to_string_lossy().to_lowercase();
            ext_lower == "jpg" || ext_lower == "jpeg"
        })
        .unwrap_or(false)
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
    let source = &opts.source_path;
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
