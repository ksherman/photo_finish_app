use std::process::Command;

/// Looks up the USB hardware serial number for a given disk (e.g. "disk5").
///
/// The IOService tree is output via DFS, so a USB device's kUSBSerialNumberString
/// always appears before its descendant disk node's BSD Name entry. We find the
/// disk's position in the output and scan backward for the nearest serial.
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
    if serial.is_empty() { None } else { Some(serial) }
}
