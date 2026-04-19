//! Low-level Linux hidraw ioctl communication.
//!
//! This module wraps `HIDIOCGFEATURE` (get feature report) and
//! `HIDIOCSFEATURE` (set feature report) — the same ioctls the
//! original C driver uses to read and write mouse configuration.

use std::fs::{File, OpenOptions};
use std::os::unix::io::AsRawFd;

use crate::error::{ControlStatus, Error, Result};
use crate::protocol::{ControlReport, ReportId};

// ─── ioctl definitions ──────────────────────────────────────────────
//
// From <linux/hidraw.h>:
//   #define HIDIOCGFEATURE(len)  _IOC(_IOC_WRITE|_IOC_READ, 'H', 0x07, len)
//   #define HIDIOCSFEATURE(len)  _IOC(_IOC_WRITE|_IOC_READ, 'H', 0x06, len)
//
// nix's ioctl macros use the direction + type + nr + size encoding.
// We define read_write ioctls parameterised by a byte-slice length.

/// Maximum HID feature report size we'll ever send/receive.
/// The largest Nyth report is the macro at 1026 bytes; round up.
const MAX_REPORT_SIZE: usize = 2048;

// Linux ioctl number encoding:
//   _IOC(dir, type, nr, size) = (dir << 30) | (size << 16) | (type << 8) | nr
// HIDIOCGFEATURE(len) = _IOC(_IOC_WRITE|_IOC_READ, 'H', 0x07, len)
// HIDIOCSFEATURE(len) = _IOC(_IOC_WRITE|_IOC_READ, 'H', 0x06, len)
const IOC_READ: u32 = 2;
const IOC_WRITE: u32 = 1;

const fn ioc(dir: u32, ty: u32, nr: u32, size: u32) -> nix::libc::c_ulong {
    ((dir << 30) | (size << 16) | (ty << 8) | nr) as nix::libc::c_ulong
}

fn hidiocgfeature_nr(len: usize) -> nix::libc::c_ulong {
    ioc(IOC_WRITE | IOC_READ, b'H' as u32, 0x07, len as u32)
}

fn hidiocsfeature_nr(len: usize) -> nix::libc::c_ulong {
    ioc(IOC_WRITE | IOC_READ, b'H' as u32, 0x06, len as u32)
}

// ─── HidrawDevice ───────────────────────────────────────────────────

/// A handle to an open hidraw file descriptor for one HID interface
/// endpoint of the Nyth.
#[derive(Debug)]
pub struct HidrawDevice {
    file: File,
    path: String,
}

impl HidrawDevice {
    /// Open a hidraw device node (e.g. `/dev/hidraw3`).
    pub fn open(path: &str) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map_err(|e| Error::DeviceOpen {
                path: path.to_string(),
                source: e,
            })?;

        Ok(Self {
            file,
            path: path.to_string(),
        })
    }

    /// The filesystem path this device was opened from.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Read a HID feature report.
    ///
    /// `report_id` is written into `buf[0]` before the ioctl, and the
    /// device fills the remaining bytes with the report data.
    pub fn read_feature_report(&self, report_id: u8, buf: &mut [u8]) -> Result<()> {
        assert!(!buf.is_empty() && buf.len() <= MAX_REPORT_SIZE);
        buf[0] = report_id;

        let ret = unsafe {
            nix::libc::ioctl(
                self.file.as_raw_fd(),
                hidiocgfeature_nr(buf.len()),
                buf.as_mut_ptr(),
            )
        };

        if ret < 0 {
            return Err(Error::Ioctl {
                operation: "HIDIOCGFEATURE",
                source: nix::Error::last(),
            });
        }

        log::trace!(
            "HID read  {:02x} ({} bytes) from {}",
            report_id,
            buf.len(),
            self.path
        );
        Ok(())
    }

    /// Write a HID feature report.
    ///
    /// `buf[0]` must already contain the report ID.
    pub fn write_feature_report(&self, buf: &[u8]) -> Result<()> {
        assert!(!buf.is_empty() && buf.len() <= MAX_REPORT_SIZE);

        let ret = unsafe {
            nix::libc::ioctl(
                self.file.as_raw_fd(),
                hidiocsfeature_nr(buf.len()),
                buf.as_ptr(),
            )
        };

        if ret < 0 {
            return Err(Error::Ioctl {
                operation: "HIDIOCSFEATURE",
                source: nix::Error::last(),
            });
        }

        log::trace!(
            "HID write {:02x} ({} bytes) to {}",
            buf[0],
            buf.len(),
            self.path
        );
        Ok(())
    }

    /// Read a feature report and return it as a fixed-size array.
    pub fn read_report<const N: usize>(&self, report_id: u8) -> Result<[u8; N]> {
        let mut buf = [0u8; N];
        self.read_feature_report(report_id, &mut buf)?;
        Ok(buf)
    }

    /// Write a feature report from a fixed-size array.
    pub fn write_report<const N: usize>(&self, buf: &[u8; N]) -> Result<()> {
        self.write_feature_report(buf)
    }
}

// ─── Control register helpers ───────────────────────────────────────

/// Poll the device control register until the write is acknowledged.
///
/// This mirrors the original `roccat_check_write()`:
///   1. Sleep `init_wait_ms` (gives the device time to start processing).
///   2. Read the control register in a loop.
///   3. If BUSY, wait `busy_wait_ms` (increasing each retry) and try again.
///   4. If OK, return success.
///   5. If CRITICAL or INVALID, return an error.
pub fn check_write(dev: &HidrawDevice, init_wait_ms: u64, busy_wait_ms: u64) -> Result<()> {
    std::thread::sleep(std::time::Duration::from_millis(init_wait_ms));

    let mut increasing_wait = busy_wait_ms;
    loop {
        let buf = dev.read_report::<3>(ReportId::Control as u8)?;
        let status = ControlStatus::from_byte(buf[1]);

        match status {
            ControlStatus::Ok => return Ok(()),
            ControlStatus::Busy => {
                std::thread::sleep(std::time::Duration::from_millis(increasing_wait));
                increasing_wait += busy_wait_ms;
            }
            other => return Err(Error::DeviceStatus(other)),
        }
    }
}

/// Send a select command via the control register, then wait for ACK.
///
/// This tells the device which data to prepare for a subsequent read.
/// Equivalent to the original `nyth_select()`.
pub fn select(dev: &HidrawDevice, profile_index: u8, data_index: u8, request: u8) -> Result<()> {
    let report = ControlReport {
        report_id: ReportId::Control as u8,
        value: data_index | profile_index,
        request,
    };
    dev.write_feature_report(bytemuck::bytes_of(&report))?;
    // The original uses init_wait=150ms, busy_wait=200ms for Nyth.
    check_write(dev, 150, 200)
}

/// Write a feature report and then check the control register.
///
/// This is the standard Nyth write-then-verify pattern from the
/// original `nyth_device_write()`.
pub fn write_checked(dev: &HidrawDevice, buf: &[u8]) -> Result<()> {
    dev.write_feature_report(buf)?;
    check_write(dev, 150, 200)
}

// ─── Higher-level typed read/write helpers ──────────────────────────

/// Read a typed report struct from the device.
///
/// This handles the select-then-read pattern for reports that require
/// it (profile settings, profile buttons), and direct reads for
/// reports that don't (profile, info, device state).
pub fn read_struct<T: bytemuck::Pod>(dev: &HidrawDevice, report_id: ReportId) -> Result<T> {
    let size = std::mem::size_of::<T>();
    let mut buf = vec![0u8; size];
    dev.read_feature_report(report_id as u8, &mut buf)?;
    Ok(*bytemuck::from_bytes(&buf))
}

/// Write a typed report struct to the device with check-write.
pub fn write_struct<T: bytemuck::Pod>(dev: &HidrawDevice, data: &T) -> Result<()> {
    write_checked(dev, bytemuck::bytes_of(data))
}
