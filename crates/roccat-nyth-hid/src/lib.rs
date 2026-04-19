//! # NewNyth — roccat-nyth-hid
//!
//! Low-level Linux HID communication layer for the Roccat Nyth mouse.
//!
//! This crate provides:
//!
//! - **Protocol structs** ([`protocol`]) — packed `#[repr(C)]` types matching
//!   the exact byte layout the Nyth sends and receives over USB HID feature
//!   reports. Every struct has compile-time size assertions.
//!
//! - **Hidraw communication** ([`hidraw`]) — `HIDIOCGFEATURE` / `HIDIOCSFEATURE`
//!   ioctl wrappers, the select/check-write handshake, and typed read/write
//!   helpers.
//!
//! - **Device discovery** ([`device`]) — scans `/sys/class/hidraw` for
//!   connected Nyth mice by USB vendor/product ID. No libudev dependency.
//!
//! ## Quick start
//!
//! ```rust,no_run
//! use roccat_nyth_hid::device;
//! use roccat_nyth_hid::hidraw;
//! use roccat_nyth_hid::protocol::*;
//!
//! // Find and open the first connected Nyth
//! let (info, dev) = device::open_first_device()?;
//! println!("Found: {}", info.name);
//!
//! // Read the active profile
//! let profile: ProfileReport = hidraw::read_struct(&dev, ReportId::Profile)?;
//! println!("Active profile: {}", profile.index + 1);
//!
//! // Read device info / firmware version
//! let info_report: InfoReport = hidraw::read_struct(&dev, ReportId::Info)?;
//! println!("Firmware: {}", info_report.firmware_version_string());
//! # Ok::<(), roccat_nyth_hid::error::Error>(())
//! ```

pub mod device;
pub mod display;
pub mod error;
pub mod hidraw;
pub mod protocol;
