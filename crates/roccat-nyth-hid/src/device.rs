//! Nyth device discovery via `/sys/class/hidraw`.
//!
//! Scans all hidraw nodes and matches against the Nyth USB vendor/product
//! IDs. Identifies which HID interface each node belongs to (mouse = 0,
//! keyboard = 1) so the correct node is used for ioctls vs event reading.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::hidraw::HidrawDevice;
use crate::protocol::{NYTH_PRODUCT_IDS, USB_VENDOR_ROCCAT};

/// Information about a discovered Nyth hidraw endpoint.
#[derive(Debug, Clone)]
pub struct NythDeviceInfo {
    /// The `/dev/hidrawN` path.
    pub hidraw_path: PathBuf,
    /// USB product ID (black or white variant).
    pub product_id: u16,
    /// Human-readable device name.
    pub name: String,
    /// USB interface number: 0 = mouse (ioctls + events), 1 = keyboard.
    /// `None` if detection failed (falls back to trying both).
    pub interface_number: Option<u8>,
}

impl NythDeviceInfo {
    /// Open the hidraw device for communication.
    pub fn open(&self) -> Result<HidrawDevice> {
        HidrawDevice::open(self.hidraw_path.to_str().unwrap_or("/dev/null"))
    }
}

/// Scan for connected Roccat Nyth devices.
///
/// Iterates `/sys/class/hidraw/hidraw*`, reads the HID device's
/// vendor and product IDs from the USB device tree, and returns
/// info for each matching Nyth found.
///
/// The original C driver used libudev for this. We read sysfs directly,
/// which avoids any runtime dependency and works on all modern kernels.
pub fn scan_devices() -> Vec<NythDeviceInfo> {
    let mut found = Vec::new();

    let hidraw_dir = Path::new("/sys/class/hidraw");
    let entries = match fs::read_dir(hidraw_dir) {
        Ok(e) => e,
        Err(e) => {
            log::warn!("Cannot read /sys/class/hidraw: {e}");
            return found;
        }
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if !name_str.starts_with("hidraw") {
            continue;
        }

        // Resolve the symlink to get the real sysfs path.
        let device_path = match fs::canonicalize(entry.path().join("device")) {
            Ok(p) => p,
            Err(_) => continue,
        };

        // Walk up the sysfs tree to find the USB device with idVendor/idProduct.
        if let Some(info) = match_nyth_device(&name_str, &device_path) {
            log::info!(
                "Found {} at /dev/{} (USB {:04x}:{:04x}, interface {:?})",
                info.name,
                name_str,
                USB_VENDOR_ROCCAT,
                info.product_id,
                info.interface_number
            );
            found.push(info);
        }
    }

    // Sort: mouse interface (0) first, then keyboard (1), then unknown.
    found.sort_by_key(|d| d.interface_number.unwrap_or(255));

    found
}

/// Find the first connected Nyth **mouse interface** and open it.
///
/// Prefers interface 0 (mouse endpoint, used for feature report ioctls).
/// Falls back to the first matching hidraw node if interface detection fails.
pub fn open_first_device() -> Result<(NythDeviceInfo, HidrawDevice)> {
    let devices = scan_devices();
    // After sorting, interface 0 (mouse) is first if detected.
    let info = devices.into_iter().next().ok_or(Error::DeviceNotFound)?;
    let dev = info.open()?;
    Ok((info, dev))
}

/// Return all hidraw paths for connected Nyth devices, with the event-
/// capable node (interface 0 / mouse) listed first.
pub fn scan_device_paths() -> Vec<NythDeviceInfo> {
    scan_devices()
}

/// Walk the sysfs tree upward from a hidraw device to find USB
/// vendor/product IDs and check if it's a Nyth.
fn match_nyth_device(hidraw_name: &str, device_path: &Path) -> Option<NythDeviceInfo> {
    // Detect the USB interface number from the sysfs path.
    // Typical path: .../usb1/1-2/1-2:1.0/0003:1E7D:2E7D.0001/hidraw/hidraw7
    // The "1-2:1.0" component has the interface number after the last dot.
    let interface_number = detect_interface_number(device_path);

    // Walk upward looking for idVendor and idProduct files.
    let mut current = device_path.to_path_buf();
    loop {
        let vendor_path = current.join("idVendor");
        let product_path = current.join("idProduct");

        if vendor_path.exists() && product_path.exists() {
            let vendor = read_hex_file(&vendor_path)?;
            let product = read_hex_file(&product_path)?;

            if vendor == USB_VENDOR_ROCCAT && NYTH_PRODUCT_IDS.contains(&product) {
                let name = match product {
                    0x2e7c => "Roccat Nyth (Black)",
                    0x2e7d => "Roccat Nyth (White)",
                    _ => "Roccat Nyth",
                };
                return Some(NythDeviceInfo {
                    hidraw_path: PathBuf::from(format!("/dev/{hidraw_name}")),
                    product_id: product,
                    name: name.to_string(),
                    interface_number,
                });
            }
            // Found USB IDs but not a Nyth — stop searching.
            return None;
        }

        // Move up one directory level.
        if !current.pop() {
            return None;
        }
    }
}

/// Try to detect the USB interface number from the sysfs path.
///
/// The path typically contains a component like "1-2:1.0" where the
/// number after the last dot is the interface number (0 = mouse, 1 = keyboard).
/// USB interface components have exactly one colon and the part before it
/// is a port path (digits and dashes/dots only, no hex-style PCI addresses).
fn detect_interface_number(device_path: &Path) -> Option<u8> {
    let path_str = device_path.to_string_lossy();
    for component in path_str.split('/') {
        // USB interface directories match "1-2:1.0" or "2-1.3:1.1"
        // but NOT PCI addresses like "0000:00:14.0" (which have multiple colons
        // or hex digits in the first segment).
        let parts: Vec<&str> = component.split(':').collect();
        if parts.len() != 2 {
            continue; // Skip components with 0 or 2+ colons (PCI addresses).
        }
        let before_colon = parts[0];
        let after_colon = parts[1];

        // The USB port path before the colon contains only digits, dashes, and dots.
        // It must start with a digit (e.g. "1-2", "2-1.3").
        if before_colon.is_empty()
            || !before_colon.as_bytes()[0].is_ascii_digit()
            || !before_colon
                .bytes()
                .all(|b| b.is_ascii_digit() || b == b'-' || b == b'.')
        {
            continue;
        }

        // After the colon: "1.0" → interface is the part after the last dot.
        if let Some(dot_pos) = after_colon.rfind('.') {
            if let Ok(iface) = after_colon[dot_pos + 1..].parse::<u8>() {
                return Some(iface);
            }
        }
    }
    None
}

/// Read a sysfs file containing a hex value like "1e7d\n".
fn read_hex_file(path: &Path) -> Option<u16> {
    let content = fs::read_to_string(path).ok()?;
    u16::from_str_radix(content.trim(), 16).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_file_parsing() {
        assert_eq!(u16::from_str_radix("1e7d", 16).unwrap(), USB_VENDOR_ROCCAT);
        assert_eq!(u16::from_str_radix("2e7c", 16).unwrap(), 0x2e7c);
    }

    #[test]
    fn interface_detection_from_path() {
        let path = Path::new("/sys/devices/pci0000:00/0000:00:14.0/usb1/1-2/1-2:1.0/0003:1E7D:2E7D.0001/hidraw/hidraw7");
        assert_eq!(detect_interface_number(path), Some(0));

        let path = Path::new("/sys/devices/pci0000:00/0000:00:14.0/usb1/1-2/1-2:1.1/0003:1E7D:2E7D.0002/hidraw/hidraw8");
        assert_eq!(detect_interface_number(path), Some(1));

        // PCI addresses must NOT match.
        let path = Path::new("/sys/devices/pci0000:00/0000:00:14.0");
        assert_eq!(detect_interface_number(path), None);
    }
}
