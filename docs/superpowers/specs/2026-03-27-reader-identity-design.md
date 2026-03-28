# Card Reader Identity — Design Spec
**Date:** 2026-03-27

## Problem

The ingest app currently identifies card readers using `DeviceTreePath` from `diskutil info` — a port-based identifier that changes when a reader is plugged into a different USB port. This causes the saved configuration (photographer, destination, camera brand) to be lost on reconnect.

## Goal

Make reader identity stable across port changes by using the USB device's hardware serial number as `reader_id`.

## Scope

- Rust backend (`commands.rs`, new `platform/` module)
- `CardReaderInfo` struct gains `usb_serial: Option<String>`
- No changes to Vue/Pinia stores or UI

Out of scope: card-level tracking, cross-platform support (noted below), UI changes.

## Design

### Stable Identity Rules

| Reader type       | `reader_id` source                          | Example                  |
|-------------------|---------------------------------------------|--------------------------|
| External USB      | `kUSBSerialNumberString` from IOKit/ioreg   | `"000000063931"`         |
| Built-in SDXC     | Fixed constant                              | `"sdxc-builtin"`         |
| USB, no serial    | Fall back to `DeviceTreePath` (status quo)  | `"IODeviceTree:/..."` |

### Module Structure

```
src-tauri/src/
  commands.rs        ← discover_card_readers() calls platform::get_usb_serial_for_disk()
  platform/
    mod.rs           ← pub fn get_usb_serial_for_disk(disk_id: &str) -> Option<String>
    macos.rs         ← ioreg implementation, #[cfg(target_os = "macos")]
```

### macOS Implementation

`macos.rs` runs:
```
ioreg -p IOService -w 0 -l
```
Parses the output to find the USB device node that owns the given BSD name (e.g. `disk5`), then extracts its `kUSBSerialNumberString` value. Returns `None` if not found (built-in, or reader without a serial).

### CardReaderInfo Changes

```rust
pub struct CardReaderInfo {
    pub reader_id: String,       // NOW: usb serial, "sdxc-builtin", or DeviceTreePath fallback
    pub usb_serial: Option<String>, // NEW: raw serial for display/debugging
    // ... all existing fields unchanged
}
```

### Migration

Existing persisted `readerMappings` (keyed on old `DeviceTreePath` strings) will be orphaned silently on first run. Users reconfigure once. No migration code needed.

## Cross-Platform Note

`ioreg` is macOS-only, as is the existing `diskutil` usage. The platform module boundary (`platform::get_usb_serial_for_disk`) makes future Linux (`/sys/bus/usb/devices/*/serial`) or Windows (WMI) implementations a contained addition rather than a refactor.

## Success Criteria

- Plugging a reader into a different USB port restores its saved configuration automatically
- Built-in SDXC slot retains its config across app restarts
- No changes required to the Vue/Pinia layer
