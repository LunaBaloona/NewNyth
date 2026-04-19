//! # NewNyth, roccat-nyth-profile
//!
//! High-level API for managing Roccat Nyth profiles, settings, buttons,
//! and device state.
//!
//! This crate wraps [`roccat_nyth_hid`] with a [`NythDevice`] struct that
//! provides safe, ergonomic methods for every device operation the
//! original `roccatnythcontrol` supported.
//!
//! ## Example:
//!
//! ```rust,no_run
//! use roccat_nyth_profile::NythDevice;
//!
//! let nyth = NythDevice::open_first()?;
//! println!("Firmware: {}", nyth.read_firmware_version()?);
//! println!("Active profile: {}", nyth.read_active_profile()? + 1);
//!
//! let settings = nyth.read_profile_settings(0)?;
//! for i in 0..5 {
//!     println!("  CPI level {}: {} DPI", i + 1, settings.cpi_level(i));
//! }
//! # Ok::<(), roccat_nyth_profile::Error>(())
//! ```

use roccat_nyth_hid::device::{self, NythDeviceInfo};
use roccat_nyth_hid::hidraw::{self, HidrawDevice};
use roccat_nyth_hid::protocol::*;

/// Re-export the protocol module for direct struct access.
pub use roccat_nyth_hid::protocol;

pub mod config;

/// Errors from profile-level operations.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Hid(#[from] roccat_nyth_hid::error::Error),

    #[error("Profile index {0} out of range (must be 0–4)")]
    ProfileOutOfRange(u8),

    #[error("CPI value {value} out of range ({min}–{max}, step {step})")]
    CpiOutOfRange {
        value: u16,
        min: u16,
        max: u16,
        step: u16,
    },

    #[error("Checksum validation failed for profile settings")]
    ChecksumFailed,
}

pub type Result<T> = std::result::Result<T, Error>;

/// High-level handle to a connected Roccat Nyth mouse.
///
/// Provides safe methods for reading and writing profiles, settings,
/// button mappings, macros, lighting, and device state.
pub struct NythDevice {
    dev: HidrawDevice,
    info: NythDeviceInfo,
}

impl NythDevice {
    /// Open the first connected Nyth device.
    pub fn open_first() -> Result<Self> {
        let (info, dev) = device::open_first_device()?;
        Ok(Self { dev, info })
    }

    /// The device info (path, product ID, name).
    pub fn info(&self) -> &NythDeviceInfo {
        &self.info
    }

    // ──── Profile ─────────────────────────────────────────────────────

    /// Read the active profile index (0–4).
    pub fn read_active_profile(&self) -> Result<u8> {
        let report: ProfileReport = hidraw::read_struct(&self.dev, ReportId::Profile)?;
        Ok(report.index)
    }

    /// Set the active profile (0–4).
    pub fn write_active_profile(&self, index: u8) -> Result<()> {
        self.validate_profile_index(index)?;
        let report = ProfileReport::new(index, NYTH_PROFILE_NUM);
        hidraw::write_struct(&self.dev, &report)?;
        Ok(())
    }

    // ── Profile settings ────────────────────────────────────────────

    /// Read settings for a profile (CPI, sensitivity, polling, lighting).
    ///
    /// This sends a select command first, then reads the settings report.
    pub fn read_profile_settings(&self, profile_index: u8) -> Result<ProfileSettings> {
        self.validate_profile_index(profile_index)?;

        // Select the profile's settings data.
        hidraw::select(
            &self.dev,
            profile_index,
            ControlDataIndex::None as u8,
            ControlRequest::ProfileSettings as u8,
        )?;

        let settings: ProfileSettings = hidraw::read_struct(&self.dev, ReportId::ProfileSettings)?;

        if !settings.validate_checksum() {
            log::warn!(
                "Profile {} settings checksum mismatch (expected {:#06x}, got {:#06x})",
                profile_index,
                settings.compute_checksum(),
                settings.checksum()
            );
        }

        Ok(settings)
    }

    /// Write settings for a profile.
    ///
    /// Automatically sets the report header and recalculates the checksum.
    pub fn write_profile_settings(
        &self,
        profile_index: u8,
        settings: &mut ProfileSettings,
    ) -> Result<()> {
        self.validate_profile_index(profile_index)?;
        settings.finalize(profile_index);
        hidraw::write_struct(&self.dev, settings)?;
        Ok(())
    }

    // ── Profile buttons ────────────────────────────────────────

    /// Read button mappings for a profile.
    pub fn read_profile_buttons(&self, profile_index: u8) -> Result<ProfileButtons> {
        self.validate_profile_index(profile_index)?;

        hidraw::select(
            &self.dev,
            profile_index,
            ControlDataIndex::None as u8,
            ControlRequest::ProfileButtons as u8,
        )?;

        let buttons: ProfileButtons = hidraw::read_struct(&self.dev, ReportId::ProfileButtons)?;
        Ok(buttons)
    }

    /// Write button mappings for a profile.
    pub fn write_profile_buttons(
        &self,
        profile_index: u8,
        buttons: &mut ProfileButtons,
    ) -> Result<()> {
        self.validate_profile_index(profile_index)?;
        buttons.finalize(profile_index);
        hidraw::write_struct(&self.dev, buttons)?;
        Ok(())
    }

    // ─── Device info / firmware ───────────────────────────────────────

    /// Read the firmware version as a formatted string (e.g. "1.10").
    pub fn read_firmware_version(&self) -> Result<String> {
        let info: InfoReport = hidraw::read_struct(&self.dev, ReportId::Info)?;
        Ok(info.firmware_version_string())
    }

    /// Reset the device to factory defaults.
    pub fn reset_to_defaults(&self) -> Result<()> {
        let info = InfoReport {
            report_id: ReportId::Info as u8,
            size: InfoReport::SIZE as u8,
            firmware_or_function: InfoReport::FUNCTION_RESET,
            unknown1: 0,
            unknown2: 0,
            unknown3: 0,
        };
        hidraw::write_struct(&self.dev, &info)?;
        Ok(())
    }

    // ── Device state ────────────────────────────────────────────────

    /// Enable driver-managed mode (device sends special events to the
    /// driver instead of emitting HID multimedia keycodes directly).
    pub fn enable_driver_mode(&self) -> Result<()> {
        let report = DeviceStateReport::new(DeviceStateReport::STATE_ON);
        hidraw::write_struct(&self.dev, &report)?;
        Ok(())
    }

    /// Disable driver-managed mode (device emits HID keycodes directly).
    pub fn disable_driver_mode(&self) -> Result<()> {
        let report = DeviceStateReport::new(DeviceStateReport::STATE_OFF);
        hidraw::write_struct(&self.dev, &report)?;
        Ok(())
    }

    // ── Control unit (TCU / DCU) ────────────────────────────────────

    /// Read the current tracking/distance control unit state.
    pub fn read_control_unit(&self) -> Result<ControlUnitReport> {
        let report: ControlUnitReport = hidraw::read_struct(&self.dev, ReportId::ControlUnit)?;
        Ok(report)
    }

    // ── Button set (side-button preset) ─────────────────────────────

    /// Read the button set preset for a profile.
    pub fn read_button_set(&self, profile_index: u8) -> Result<ButtonSet> {
        self.validate_profile_index(profile_index)?;

        hidraw::select(
            &self.dev,
            profile_index,
            ControlDataIndex::None as u8,
            ControlRequest::ButtonSetPreset as u8,
        )?;

        let button_set: ButtonSet = hidraw::read_struct(&self.dev, ReportId::ButtonSet)?;
        Ok(button_set)
    }

    // ── Validation helpers ──────────────────────────────────────────

    fn validate_profile_index(&self, index: u8) -> Result<()> {
        if index >= NYTH_PROFILE_NUM {
            return Err(Error::ProfileOutOfRange(index));
        }
        Ok(())
    }
}

impl std::fmt::Debug for NythDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NythDevice")
            .field("path", &self.info.hidraw_path)
            .field("name", &self.info.name)
            .finish()
    }
}
