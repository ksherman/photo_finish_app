use serde::Deserialize;
use std::path::Path;
use std::process::Command;

use super::{
    count_folders, count_jpeg_files, resolve_photo_source_dir, CardReaderInfo, UsbDriveInfo,
};
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

#[derive(Default, Deserialize)]
#[allow(dead_code)]
struct DiskutilInfo {
    #[serde(rename = "DeviceTreePath")]
    device_tree_path: Option<String>,
    #[serde(rename = "BusProtocol")]
    bus_protocol: Option<String>,
    #[serde(rename = "VirtualOrPhysical")]
    virtual_or_physical: Option<String>,
    #[serde(rename = "Internal")]
    internal: Option<bool>,
    #[serde(rename = "Removable")]
    removable: Option<bool>,
    #[serde(rename = "RemovableMedia")]
    removable_media: Option<bool>,
    #[serde(rename = "Ejectable")]
    ejectable: Option<bool>,
    #[serde(rename = "WritableVolume")]
    writable_volume: Option<bool>,
    #[serde(rename = "WritableMedia")]
    writable_media: Option<bool>,
    #[serde(rename = "VolumeName")]
    volume_name: Option<String>,
    #[serde(rename = "MountPoint")]
    mount_point: Option<String>,
    #[serde(rename = "MediaName")]
    media_name: Option<String>,
    #[serde(rename = "TotalSize")]
    total_size: Option<u64>,
    #[serde(rename = "Size")]
    size: Option<u64>,
    #[serde(rename = "FreeSpace")]
    free_space: Option<u64>,
    #[serde(rename = "APFSContainerFree")]
    apfs_container_free: Option<u64>,
}

fn list_diskutil_disks() -> Result<Vec<DiskEntry>> {
    let output = Command::new("diskutil")
        .args(["list", "-plist"])
        .output()
        .map_err(|e| Error::Io(format!("Failed to run diskutil list: {}", e)))?;
    let list: DiskutilList = plist::from_bytes(&output.stdout)
        .map_err(|e| Error::Io(format!("Failed to parse diskutil list: {}", e)))?;
    Ok(list.all_disks_and_partitions)
}

fn query_diskutil_info(device_id: &str) -> Result<Option<DiskutilInfo>> {
    let output = Command::new("diskutil")
        .args(["info", "-plist", device_id])
        .output()
        .map_err(|e| Error::Io(format!("Failed to run diskutil info: {}", e)))?;
    Ok(plist::from_bytes(&output.stdout).ok())
}

/// Returns true for virtual disks and mounted disk images (e.g. Docker.dmg, installer `.dmg`s)
/// so they never appear as camera card readers or USB drives.
fn is_virtual_or_disk_image(info: &DiskutilInfo) -> bool {
    info.virtual_or_physical
        .as_deref()
        .is_some_and(|v| v.eq_ignore_ascii_case("virtual"))
        || info
            .bus_protocol
            .as_deref()
            .is_some_and(|p| p.to_lowercase().contains("disk image"))
        || info
            .media_name
            .as_deref()
            .is_some_and(|m| m.to_lowercase().contains("disk image"))
}

/// Looks up the USB hardware serial number for a given disk (e.g. "disk5").
pub fn get_usb_serial_for_disk(disk_id: &str) -> Option<String> {
    let output = Command::new("ioreg")
        .args(["-p", "IOService", "-w", "0", "-l"])
        .output()
        .ok()?;

    let text = String::from_utf8_lossy(&output.stdout);
    let disk_pos = text.find(&format!("\"{}\"", disk_id))?;
    let before = &text[..disk_pos];
    let serial_key = "\"kUSBSerialNumberString\" = \"";
    let serial_pos = before.rfind(serial_key)?;
    let after_key = &before[serial_pos + serial_key.len()..];
    let serial = &after_key[..after_key.find('"')?];
    (!serial.is_empty()).then(|| serial.to_string())
}

pub fn discover_card_readers() -> Result<Vec<CardReaderInfo>> {
    let mut readers = Vec::new();

    for disk in list_diskutil_disks()? {
        let Some(disk_info) = query_diskutil_info(&disk.device_identifier)? else {
            continue;
        };
        if is_virtual_or_disk_image(&disk_info) {
            continue;
        }
        if !(disk_info.removable.unwrap_or(false) || disk_info.removable_media.unwrap_or(false)) {
            continue;
        }

        for partition in &disk.partitions {
            let Some(mount_point) = &partition.mount_point else {
                continue;
            };
            let Some(part_info) = query_diskutil_info(&partition.device_identifier)? else {
                continue;
            };

            let volume_name = partition
                .volume_name
                .clone()
                .or(part_info.volume_name)
                .unwrap_or_else(|| "Untitled".to_string());
            let bus_protocol = disk_info
                .bus_protocol
                .clone()
                .unwrap_or_else(|| "Unknown".to_string());
            let is_internal = disk_info.internal.unwrap_or(false);
            let usb_serial = (!is_internal)
                .then(|| get_usb_serial_for_disk(&disk.device_identifier))
                .flatten();

            let mut reader_id = if is_internal {
                "sdxc-builtin".to_string()
            } else if let Some(ref serial) = usb_serial {
                serial.clone()
            } else {
                disk_info
                    .device_tree_path
                    .clone()
                    .unwrap_or_else(|| disk.device_identifier.clone())
            };

            // Disambiguate multi-slot hubs or identical low-cost readers sharing a USB serial.
            if readers.iter().any(|r: &CardReaderInfo| r.reader_id == reader_id) {
                let disambiguator = disk_info
                    .device_tree_path
                    .clone()
                    .unwrap_or_else(|| disk.device_identifier.clone());
                reader_id = format!("{}@{}", reader_id, disambiguator);
            }

            let display_name = if is_internal {
                format!("Built-in {} Reader", bus_protocol)
            } else {
                format!("External {} Reader", bus_protocol)
            };

            let mount_path = Path::new(mount_point);
            let photo_root = resolve_photo_source_dir(mount_path);
            readers.push(CardReaderInfo {
                reader_id,
                usb_serial,
                display_name,
                mount_point: mount_point.clone(),
                volume_name,
                bus_protocol,
                is_internal,
                disk_id: disk.device_identifier.clone(),
                file_count: count_jpeg_files(mount_path),
                folder_count: count_folders(&photo_root),
            });
        }
    }

    Ok(readers)
}

/// Discovers mounted external USB / removable drives suitable for instant USB fulfillment.
/// Flags volumes containing a `DCIM/` folder (`has_dcim = true`) as a camera-card guard.
pub fn discover_usb_drives() -> Result<Vec<UsbDriveInfo>> {
    let mut drives = Vec::new();

    for disk in list_diskutil_disks()? {
        let Some(disk_info) = query_diskutil_info(&disk.device_identifier)? else {
            continue;
        };
        if is_virtual_or_disk_image(&disk_info) || disk_info.internal.unwrap_or(false) {
            continue;
        }
        let bus_protocol = disk_info
            .bus_protocol
            .clone()
            .unwrap_or_else(|| "Unknown".to_string());
        let is_removable = disk_info.removable.unwrap_or(false)
            || disk_info.removable_media.unwrap_or(false)
            || disk_info.ejectable.unwrap_or(false)
            || bus_protocol.to_lowercase().contains("usb");
        if !is_removable {
            continue;
        }

        for partition in &disk.partitions {
            let Some(mount_point) = &partition.mount_point else {
                continue;
            };
            if mount_point.is_empty() || mount_point == "/" || mount_point.starts_with("/System") {
                continue;
            }
            let Some(part_info) = query_diskutil_info(&partition.device_identifier)? else {
                continue;
            };
            if is_virtual_or_disk_image(&part_info)
                || part_info.writable_volume == Some(false)
                || part_info.writable_media == Some(false)
            {
                continue;
            }

            let volume_name = partition
                .volume_name
                .clone()
                .or(part_info.volume_name)
                .unwrap_or_else(|| "USB Drive".to_string());
            let total_bytes = part_info
                .total_size
                .or(part_info.size)
                .or(disk_info.total_size)
                .or(disk_info.size)
                .unwrap_or(0);
            let free_bytes = part_info
                .apfs_container_free
                .or(part_info.free_space)
                .unwrap_or(0);
            let has_dcim = Path::new(mount_point).join("DCIM").is_dir();

            drives.push(UsbDriveInfo {
                disk_id: disk.device_identifier.clone(),
                volume_name,
                mount_point: mount_point.clone(),
                bus_protocol: bus_protocol.clone(),
                total_bytes,
                free_bytes,
                has_dcim,
            });
        }
    }

    Ok(drives)
}

pub fn eject_drive(disk_id: &str) -> Result<()> {
    let trimmed = disk_id.trim();
    if trimmed.is_empty() {
        return Err(Error::InvalidInput("disk_id cannot be empty".into()));
    }
    let output = Command::new("diskutil")
        .args(["eject", trimmed])
        .output()
        .map_err(|e| Error::Io(format!("Failed to run diskutil eject {}: {}", trimmed, e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(Error::Io(format!(
            "Failed to eject {}: {} {}",
            trimmed,
            stderr.trim(),
            stdout.trim()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_out_virtual_and_disk_image_volumes() {
        let dmg = DiskutilInfo {
            bus_protocol: Some("Disk Image".into()),
            virtual_or_physical: Some("Virtual".into()),
            media_name: Some("Disk Image".into()),
            ..Default::default()
        };
        assert!(is_virtual_or_disk_image(&dmg));

        let real_sd = DiskutilInfo {
            bus_protocol: Some("Secure Digital".into()),
            virtual_or_physical: Some("Physical".into()),
            internal: Some(true),
            removable: Some(true),
            media_name: Some("Built In SDXC Reader".into()),
            ..Default::default()
        };
        assert!(!is_virtual_or_disk_image(&real_sd));
    }
}
