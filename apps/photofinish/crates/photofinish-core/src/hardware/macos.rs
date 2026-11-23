use serde::Deserialize;
use std::path::Path;
use std::process::Command;

use super::{count_folders, count_jpeg_files, CardReaderInfo, UsbDriveInfo};
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

/// Returns true for virtual disks and mounted disk images (e.g. Docker.dmg, installer `.dmg`s,
/// Time Machine sparsebundles) so they never appear as camera card readers or USB drives.
fn is_virtual_or_disk_image(info: &DiskutilInfo) -> bool {
    if info
        .virtual_or_physical
        .as_deref()
        .map(|v| v.eq_ignore_ascii_case("virtual"))
        .unwrap_or(false)
    {
        return true;
    }
    if info
        .bus_protocol
        .as_deref()
        .map(|p| p.to_lowercase().contains("disk image"))
        .unwrap_or(false)
    {
        return true;
    }
    if info
        .media_name
        .as_deref()
        .map(|m| m.to_lowercase().contains("disk image"))
        .unwrap_or(false)
    {
        return true;
    }
    false
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

        if is_virtual_or_disk_image(&disk_info) {
            continue;
        }

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

/// Discovers mounted external USB / removable drives suitable for instant USB fulfillment.
/// Explicitly flags volumes containing a `DCIM/` folder (`has_dcim = true`) so staff never
/// accidentally write a customer order onto a photographer's SD card.
pub fn discover_usb_drives() -> Result<Vec<UsbDriveInfo>> {
    let mut drives = Vec::new();

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

        if is_virtual_or_disk_image(&disk_info) {
            continue;
        }

        let is_internal = disk_info.internal.unwrap_or(false);
        if is_internal {
            continue;
        }
        let bus_protocol = disk_info
            .bus_protocol
            .clone()
            .unwrap_or_else(|| "Unknown".to_string());
        let is_removable = disk_info.removable.unwrap_or(false)
            || disk_info.removable_media.unwrap_or(false);
        let is_ejectable = disk_info.ejectable.unwrap_or(false);
        if !is_removable && !is_ejectable && !bus_protocol.to_lowercase().contains("usb") {
            continue;
        }

        for partition in &disk.partitions {
            let Some(mount_point) = &partition.mount_point else {
                continue;
            };
            if mount_point.is_empty() || mount_point == "/" || mount_point.starts_with("/System") {
                continue;
            }

            let part_output = Command::new("diskutil")
                .args(["info", "-plist", &partition.device_identifier])
                .output()
                .map_err(|e| Error::Io(format!("Failed to run diskutil info: {}", e)))?;

            let part_info: DiskutilInfo = match plist::from_bytes(&part_output.stdout) {
                Ok(info) => info,
                Err(_) => continue,
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
                .or(part_info.volume_name.clone())
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
            let mount_path = Path::new(mount_point);
            let has_dcim = mount_path.join("DCIM").is_dir();

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
            device_tree_path: None,
            bus_protocol: Some("Disk Image".into()),
            virtual_or_physical: Some("Virtual".into()),
            internal: Some(false),
            removable: Some(true),
            removable_media: Some(true),
            ejectable: Some(true),
            writable_volume: Some(false),
            writable_media: Some(false),
            volume_name: Some("Docker".into()),
            mount_point: Some("/Volumes/Docker".into()),
            media_name: Some("Disk Image".into()),
            total_size: None,
            size: None,
            free_space: None,
            apfs_container_free: None,
        };
        assert!(is_virtual_or_disk_image(&dmg));

        let real_sd = DiskutilInfo {
            device_tree_path: None,
            bus_protocol: Some("Secure Digital".into()),
            virtual_or_physical: Some("Physical".into()),
            internal: Some(true),
            removable: Some(true),
            removable_media: Some(true),
            ejectable: Some(true),
            writable_volume: Some(true),
            writable_media: Some(true),
            volume_name: Some("EOS_DIGITAL".into()),
            mount_point: Some("/Volumes/EOS_DIGITAL".into()),
            media_name: Some("Built In SDXC Reader".into()),
            total_size: None,
            size: None,
            free_space: None,
            apfs_container_free: None,
        };
        assert!(!is_virtual_or_disk_image(&real_sd));
    }
}
