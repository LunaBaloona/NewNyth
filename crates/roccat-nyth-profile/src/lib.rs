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

/// Program version plus the short git commit this binary was built
/// from, e.g. "0.9.5 #4d0e5a7". Shared by every NewNyth binary so any
/// version display can prove which revision is actually running.
/// Builds outside a git checkout (source tarballs) fall back to the
/// plain package version.
pub fn version_with_commit() -> String {
    match option_env!("GIT_HASH") {
        Some(hash) if !hash.is_empty() => format!("{} #{}", env!("CARGO_PKG_VERSION"), hash),
        _ => env!("CARGO_PKG_VERSION").to_string(),
    }
}

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
///
/// Every operation takes the exclusive device lock from
/// [`roccat_nyth_hid::hidraw`], so select+read and write+verify
/// sequences never interleave with another NewNyth process driving the
/// same mouse. Reads of select-armed reports also verify the echoed
/// `profile_index`, rejecting replies that belong to a different slot.
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
        let _guard = self.dev.lock()?;
        let report: ProfileReport = hidraw::read_struct(&self.dev, ReportId::Profile)?;
        Ok(report.index)
    }

    /// Set the active profile (0–4).
    pub fn write_active_profile(&self, index: u8) -> Result<()> {
        self.validate_profile_index(index)?;
        let _guard = self.dev.lock()?;
        let report = ProfileReport::new(index, NYTH_PROFILE_NUM);
        hidraw::write_struct(&self.dev, &report)?;
        Ok(())
    }

    // ── Profile settings ────────────────────────────────────────────

    /// Read settings for a profile (CPI, sensitivity, polling, lighting).
    ///
    /// This sends a select command first, then reads the settings report.
    /// The lock is held across select+read so a concurrent process cannot
    /// re-point the read at another profile's data.
    pub fn read_profile_settings(&self, profile_index: u8) -> Result<ProfileSettings> {
        self.validate_profile_index(profile_index)?;
        let _guard = self.dev.lock()?;

        // Select the profile's settings data.
        hidraw::select(
            &self.dev,
            profile_index,
            ControlDataIndex::None as u8,
            ControlRequest::ProfileSettings as u8,
        )?;

        let settings: ProfileSettings = hidraw::read_struct(&self.dev, ReportId::ProfileSettings)?;

        // The device echoes the profile index inside the report (it is part
        // of the checksummed payload). A mismatch means the selection was
        // changed between select and read by a tool that does not honour
        // the lock, so the bytes are for the wrong profile: reject them
        // rather than edit one profile while displaying another.
        check_profile_index(profile_index, settings.profile_index)?;

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
        let _guard = self.dev.lock()?;
        settings.finalize(profile_index);
        hidraw::write_struct(&self.dev, settings)?;
        Ok(())
    }

    // ── Profile buttons ────────────────────────────────────────

    /// Read button mappings for a profile.
    pub fn read_profile_buttons(&self, profile_index: u8) -> Result<ProfileButtons> {
        self.validate_profile_index(profile_index)?;
        let _guard = self.dev.lock()?;

        hidraw::select(
            &self.dev,
            profile_index,
            ControlDataIndex::None as u8,
            ControlRequest::ProfileButtons as u8,
        )?;

        let buttons: ProfileButtons = hidraw::read_struct(&self.dev, ReportId::ProfileButtons)?;
        check_profile_index(profile_index, buttons.profile_index)?;
        Ok(buttons)
    }

    /// Write button mappings for a profile.
    pub fn write_profile_buttons(
        &self,
        profile_index: u8,
        buttons: &mut ProfileButtons,
    ) -> Result<()> {
        self.validate_profile_index(profile_index)?;
        let _guard = self.dev.lock()?;
        buttons.finalize(profile_index);
        hidraw::write_struct(&self.dev, buttons)?;
        Ok(())
    }

    // ─── Device info / firmware ───────────────────────────────────────

    /// Read the firmware version as a formatted string (e.g. "1.10").
    pub fn read_firmware_version(&self) -> Result<String> {
        let _guard = self.dev.lock()?;
        let info: InfoReport = hidraw::read_struct(&self.dev, ReportId::Info)?;
        Ok(info.firmware_version_string())
    }

    /// Reset the device to factory defaults.
    pub fn reset_to_defaults(&self) -> Result<()> {
        let _guard = self.dev.lock()?;
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
        let _guard = self.dev.lock()?;
        let report = DeviceStateReport::new(DeviceStateReport::STATE_ON);
        hidraw::write_struct(&self.dev, &report)?;
        Ok(())
    }

    /// Disable driver-managed mode (device emits HID keycodes directly).
    pub fn disable_driver_mode(&self) -> Result<()> {
        let _guard = self.dev.lock()?;
        let report = DeviceStateReport::new(DeviceStateReport::STATE_OFF);
        hidraw::write_struct(&self.dev, &report)?;
        Ok(())
    }

    // ── Control unit (TCU / DCU) ────────────────────────────────────

    /// Read the current tracking/distance control unit state.
    pub fn read_control_unit(&self) -> Result<ControlUnitReport> {
        let _guard = self.dev.lock()?;
        let report: ControlUnitReport = hidraw::read_struct(&self.dev, ReportId::ControlUnit)?;
        Ok(report)
    }

    /// Set the lift-off distance (distance control unit).
    ///
    /// Takes effect immediately and is device-level, not per-profile.
    /// The TCU field is sent as [`ControlUnitReport::UNCHANGED`] so the
    /// tracking unit stays untouched, mirroring the original driver's
    /// `nyth_distance_control_unit_set()`.
    pub fn write_distance_control(&self, dcu: DistanceControl) -> Result<()> {
        let _guard = self.dev.lock()?;
        let report = ControlUnitReport::new(
            dcu as u8,
            ControlUnitReport::UNCHANGED,
            TcuAction::Off as u8,
        );
        hidraw::write_struct(&self.dev, &report)?;
        Ok(())
    }

    /// Write a tracking-unit command (the `tcu`/`action` pair of a
    /// control-unit report, with the distance field left unchanged).
    ///
    /// Mirrors the original driver's `nyth_tracking_control_unit()` in
    /// `nyth_control_unit.c`.
    fn write_tracking_control(&self, tcu: u8, action: TcuAction) -> Result<()> {
        let _guard = self.dev.lock()?;
        let report = ControlUnitReport::new(ControlUnitReport::UNCHANGED, tcu, action as u8);
        hidraw::write_struct(&self.dev, &report)?;
        Ok(())
    }

    /// Start the surface-calibration handshake (turn the TCU on).
    ///
    /// This is the only way the device switches the TCU on: it calibrates
    /// the current surface and then reports back with a special event
    /// (type `0x30`), which the caller must follow up with either
    /// [`Self::write_tracking_control_accept`] or
    /// [`Self::write_tracking_control_cancel`] (see roccat-tools
    /// `nyth_tcu_calibration_assistant.c`).
    pub fn write_tracking_control_start(&self) -> Result<()> {
        self.write_tracking_control(1, TcuAction::Start)
    }

    /// Commit the calibration started by
    /// [`Self::write_tracking_control_start`].
    pub fn write_tracking_control_accept(&self) -> Result<()> {
        self.write_tracking_control(1, TcuAction::Accept)
    }

    /// Abandon the calibration started by
    /// [`Self::write_tracking_control_start`] (also answers the
    /// keep-or-discard question with "discard").
    pub fn write_tracking_control_cancel(&self) -> Result<()> {
        self.write_tracking_control(1, TcuAction::Cancel)
    }

    /// Turn the tracking control unit (lift detection) off.
    pub fn write_tracking_control_off(&self) -> Result<()> {
        self.write_tracking_control(0, TcuAction::Off)
    }

    // ── Button set (side-button preset) ─────────────────────────────

    /// Read the button set preset for a profile.
    pub fn read_button_set(&self, profile_index: u8) -> Result<ButtonSet> {
        self.validate_profile_index(profile_index)?;
        let _guard = self.dev.lock()?;

        hidraw::select(
            &self.dev,
            profile_index,
            ControlDataIndex::None as u8,
            ControlRequest::ButtonSetPreset as u8,
        )?;

        let button_set: ButtonSet = hidraw::read_struct(&self.dev, ReportId::ButtonSet)?;
        check_profile_index(profile_index, button_set.profile_index)?;
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

/// Reject a device reply whose echoed `profile_index` does not match
/// what was requested.
///
/// A Nyth `select` arms exactly one report for the next read. If the
/// reply names a different slot, something that did not take the
/// device lock changed the selection mid-sequence, so the bytes belong
/// to another profile. Failing loudly beats silently editing the wrong
/// profile.
fn check_profile_index(expected: u8, actual: u8) -> Result<()> {
    if actual != expected {
        log::error!(
            "Device reply for profile {} carried index {} (selection race?)",
            expected,
            actual
        );
        return Err(Error::Hid(roccat_nyth_hid::error::Error::ProfileMismatch {
            expected,
            got: actual,
        }));
    }
    Ok(())
}

impl std::fmt::Debug for NythDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NythDevice")
            .field("path", &self.info.hidraw_path)
            .field("name", &self.info.name)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_echoed_profile_index_accepted() {
        assert!(check_profile_index(2, 2).is_ok());
    }

    #[test]
    fn mismatched_echoed_profile_index_rejected() {
        // Simulates losing the device selection to another process: the
        // reply bytes are profile 3's although profile 1 was requested.
        let err = check_profile_index(1, 3).unwrap_err();
        assert!(matches!(
            err,
            Error::Hid(roccat_nyth_hid::error::Error::ProfileMismatch {
                expected: 1,
                got: 3
            })
        ));
        // Both indices appear in the message so journals name the race.
        let msg = err.to_string();
        assert!(msg.contains("3"), "{msg}");
        assert!(msg.contains("1"), "{msg}");
    }
}
