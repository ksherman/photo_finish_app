#[cfg(target_os = "macos")]
mod macos;

/// Returns the USB hardware serial number for the given disk identifier (e.g. "disk5").
/// Returns None for non-USB devices or platforms without an implementation.
pub fn get_usb_serial_for_disk(disk_id: &str) -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        macos::get_usb_serial_for_disk(disk_id)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = disk_id;
        None
    }
}
