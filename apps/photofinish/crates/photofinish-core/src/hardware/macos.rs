use serde::Deserialize;
use std::path::Path;
use std::process::Command;

use super::{count_folders, count_jpeg_files, CardReaderInfo};
use crate::{Error, Result};

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

/// Looks up the USB hardware serial number for a given disk (e.g. "disk5").
///
/// The IOService tree is output via DFS, so a USB device's kUSBSerialNumberString
/// always appears before its descendant disk node's BSD Name entry.
pub fn get_usb_serial_for_disk(disk_id: &str) -> Option<String> {
    let output = Command::new("ioreg")
        .args(["-p", "IOService", "-w", "0", "-l"])
        .output()
        .ok()?;

    let text = String::from_utf8_lossy(&output.stdout);
    let disk_search = format!("\"{}\"", disk_id);
    let disk_pos = text.find(disk_search.as_str())?;

    let before = &text[..disk_pos];
    let serial_key = "\"kUSBSerialNumberString\" = \"";
    let serial_pos = before.rfind(serial_key)?;

    let after_key = &before[serial_pos + serial_key.len()..];
    let end_quote = after_key.find('"')?;

    let serial = after_key[..end_quote].to_string();
    if serial.is_empty() {
        None
    } else {
        Some(serial)
    }
}

pub fn discover_card_readers() -> Result<Vec<CardReaderInfo>> {
    let mut readers = Vec::new();

    let output = Command::new("diskutil")
        .args(["list", "-plist"])
        .output()
        .map_err(|e| Error::Io(format!("Failed to run diskutil list: {}", e)))?;

    let disk_list: DiskutilList = plist::from_bytes(&output.stdout)
        .map_err(|e| Error::Io(format!("Failed to parse diskutil list: {}", e)))?;

    for disk in disk_list.all_disks_and_partitions {
        let info_output = Command::new("diskutil")
            .args(["info", "-plist", &disk.device_identifier])
            .output()
            .map_err(|e| Error::Io(format!("Failed to run diskutil info: {}", e)))?;

        let disk_info: DiskutilInfo = match plist::from_bytes(&info_output.stdout) {
            Ok(info) => info,
            Err(_) => continue,
        };

        let is_removable = disk_info.removable.unwrap_or(false)
            || disk_info.removable_media.unwrap_or(false);
        if !is_removable {
            continue;
        }

        for partition in &disk.partitions {
            if let Some(mount_point) = &partition.mount_point {
                let part_output = Command::new("diskutil")
                    .args(["info", "-plist", &partition.device_identifier])
                    .output()
                    .map_err(|e| Error::Io(format!("Failed to run diskutil info: {}", e)))?;

                let part_info: DiskutilInfo = match plist::from_bytes(&part_output.stdout) {
                    Ok(info) => info,
                    Err(_) => continue,
                };

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

                let usb_serial = if is_internal {
                    None
                } else {
                    get_usb_serial_for_disk(&disk.device_identifier)
                };

                let reader_id = if is_internal {
                    "sdxc-builtin".to_string()
                } else if let Some(ref serial) = usb_serial {
                    serial.clone()
                } else {
                    disk_info
                        .device_tree_path
                        .clone()
                        .unwrap_or_else(|| disk.device_identifier.clone())
                };

                let display_name = if is_internal {
                    format!("Built-in {} Reader", bus_protocol)
                } else {
                    format!("External {} Reader", bus_protocol)
                };

                let mount_path = Path::new(mount_point);
                let file_count = count_jpeg_files(mount_path);
                let dcim_path = mount_path.join("DCIM");
                let folder_count = if dcim_path.exists() && dcim_path.is_dir() {
                    count_folders(&dcim_path)
                } else {
                    count_folders(mount_path)
                };

                readers.push(CardReaderInfo {
                    reader_id,
                    usb_serial,
                    display_name,
                    mount_point: mount_point.clone(),
                    volume_name,
                    bus_protocol,
                    is_internal,
                    disk_id: disk.device_identifier.clone(),
                    file_count,
                    folder_count,
                });
            }
        }
    }

    Ok(readers)
}
