//! Wire-format structs for the Roccat Nyth USB HID protocol.
//!
//! Every struct here is `#[repr(C, packed)]` and matches the exact byte
//! layout the mouse sends and expects over HID feature reports. Field
//! order, sizes, and endianness are derived from the original C source
//! in roccat-tools 5.9.1.
//!
//! Multi-byte integers are **little-endian** on the wire. Use the
//! accessor methods rather than reading fields directly.

use bytemuck::{Pod, Zeroable};

// ─── USB identifiers ────────────────────────────────────────────────

/// Roccat vendor ID.
pub const USB_VENDOR_ROCCAT: u16 = 0x1e7d;

/// Product ID: Nyth Black.
pub const USB_PID_NYTH_BLACK: u16 = 0x2e7c;

/// Product ID: Nyth White.
pub const USB_PID_NYTH_WHITE: u16 = 0x2e7d;

/// All known Nyth product IDs.
pub const NYTH_PRODUCT_IDS: &[u16] = &[USB_PID_NYTH_BLACK, USB_PID_NYTH_WHITE];

// ─── Constants ──────────────────────────────────────────────────────

/// Number of stored profiles (0–4).
pub const NYTH_PROFILE_NUM: u8 = 5;
/// Number of RGB lighting zones.
pub const NYTH_LIGHTS_NUM: usize = 2;
/// Minimum CPI value.
pub const NYTH_CPI_MIN: u16 = 200;
/// Maximum CPI value.
pub const NYTH_CPI_MAX: u16 = 12000;
/// CPI adjustment step size.
pub const NYTH_CPI_STEP: u16 = 10;
/// Number of physical buttons.
pub const NYTH_PHYSICAL_BUTTON_NUM: usize = 20;
/// Total button slots (physical × 2 for EasyShift layer).
pub const NYTH_PROFILE_BUTTON_NUM: usize = NYTH_PHYSICAL_BUTTON_NUM * 2;
/// Number of CPI levels per profile.
pub const NYTH_CPI_LEVELS_NUM: usize = 5;

/// Maximum keystrokes in a macro.
pub const NYTH_MACRO_KEYSTROKES_NUM: usize = 480;
/// Macro set name buffer length.
pub const NYTH_MACRO_MACROSET_NAME_LENGTH: usize = 40;
/// Macro name buffer length.
pub const NYTH_MACRO_MACRO_NAME_LENGTH: usize = 32;
/// Button set name buffer length.
pub const NYTH_BUTTON_SET_NAME_LENGTH: usize = 24;

// ─── HID report IDs ────────────────────────────────────────────────

/// Report IDs used in HID feature-report communication with the Nyth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ReportId {
    Mouse = 0x01,
    Special = 0x03,
    Control = 0x04,
    Profile = 0x05,
    ProfileSettings = 0x06,
    ProfileButtons = 0x07,
    Macro = 0x08,
    Info = 0x09,
    ButtonSet = 0x0a,
    SromWrite = 0x0d,
    DeviceState = 0x0e,
    ControlUnit = 0x0f,
    Talk = 0x10,
    FirmwareWrite = 0x1b,
    FirmwareWriteControl = 0x1c,
    System = 0x20,
}

/// HID interface endpoints on the Nyth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Interface {
    Mouse = 0,
    Keyboard = 1,
}

// ─── Control register ───────────────────────────────────────────────

/// Requests sent via the control register to select data for reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ControlRequest {
    Check = 0x00,
    ProfileSettings = 0x80,
    ProfileButtons = 0x90,
    ButtonSetPreset = 0xa0,
}

/// Data-index modifier ORed into the control register value byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ControlDataIndex {
    None = 0x00,
    Macro1 = 0x10,
    Macro2 = 0x20,
}

/// Control register report (3 bytes). Used for select/check-write.
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct ControlReport {
    pub report_id: u8,
    pub value: u8,
    pub request: u8,
}

// SAFETY: ControlReport is repr(C, packed), all-u8, no padding.
unsafe impl Zeroable for ControlReport {}
unsafe impl Pod for ControlReport {}

// ─── Shared sub-structs ─────────────────────────────────────────────

/// A single RGB light zone (4 bytes on wire).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C, packed)]
pub struct Light {
    pub index: u8,
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

unsafe impl Zeroable for Light {}
unsafe impl Pod for Light {}

/// A single button assignment (3 bytes on wire).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C, packed)]
pub struct Button {
    /// Button type — see [`ButtonType`].
    pub button_type: u8,
    /// Modifier flags for shortcuts, or CPI value for custom CPI.
    pub modifier: u8,
    /// HID key code for shortcuts.
    pub key: u8,
}

unsafe impl Zeroable for Button {}
unsafe impl Pod for Button {}

/// A single macro keystroke (4 bytes on wire).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C, packed)]
pub struct Keystroke {
    /// HID key code.
    pub key: u8,
    /// Press (1) or release (2).
    pub action: u8,
    /// Delay in milliseconds (little-endian).
    period_le: u16,
}

unsafe impl Zeroable for Keystroke {}
unsafe impl Pod for Keystroke {}

impl Keystroke {
    pub fn period_ms(&self) -> u16 {
        u16::from_le(self.period_le)
    }

    pub fn set_period_ms(&mut self, ms: u16) {
        self.period_le = ms.to_le();
    }
}

// ─── Profile (4 bytes) ─────────────────────────────────────────────

/// Active profile selector report (4 bytes).
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct ProfileReport {
    pub report_id: u8, // NYTH_REPORT_ID_PROFILE = 0x05
    pub size: u8,      // always 0x04
    pub index: u8,     // 0–4
    pub count: u8,
}

unsafe impl Zeroable for ProfileReport {}
unsafe impl Pod for ProfileReport {}

impl ProfileReport {
    pub const SIZE: usize = 4;

    pub fn new(index: u8, count: u8) -> Self {
        Self {
            report_id: ReportId::Profile as u8,
            size: Self::SIZE as u8,
            index,
            count,
        }
    }
}

// ─── Profile settings (33 bytes) ───────────────────────────────────

/// Per-profile hardware settings (33 bytes on wire).
///
/// Contains CPI levels, sensitivity, polling rate, and lighting.
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct ProfileSettings {
    pub report_id: u8,     // 0x06
    pub size: u8,          // 0x21 = 33
    pub profile_index: u8, // 0–4
    pub advanced_sensitivity: u8,
    pub sensitivity_x: u8,
    pub sensitivity_y: u8,
    pub cpi_levels_enabled: u8,
    cpi_levels_le: [u16; NYTH_CPI_LEVELS_NUM],
    pub cpi_active: u8, // 0–4
    pub polling_rate: u8,
    pub lights_enabled: u8,
    pub color_effect: u8,
    pub light_effect: u8,
    pub effect_speed: u8,
    pub lights: [Light; NYTH_LIGHTS_NUM],
    checksum_le: u16,
}

unsafe impl Zeroable for ProfileSettings {}
unsafe impl Pod for ProfileSettings {}

impl ProfileSettings {
    pub const SIZE: usize = 33;

    /// Default settings extracted from the original C source.
    pub const DEFAULT_BYTES: [u8; 33] = [
        0x06, 0x21, 0x00, 0x00, 0x06, 0x06, 0x1F, 0x20, 0x03, 0x40, 0x06, 0x80, 0x0C, 0x00, 0x19,
        0xE0, 0x2E, 0x00, 0x03, 0x03, 0x01, 0x03, 0x02, 0x01, 0x00, 0x64, 0xFA, 0x01, 0x00, 0x64,
        0xFA, 0x64, 0x05,
    ];

    /// Get a CPI level value (in DPI units).
    pub fn cpi_level(&self, index: usize) -> u16 {
        u16::from_le(self.cpi_levels_le[index])
    }

    /// Set a CPI level value (in DPI units).
    pub fn set_cpi_level(&mut self, index: usize, value: u16) {
        self.cpi_levels_le[index] = value.to_le();
    }

    /// Compute the checksum (byte-sum of report_id through effect_speed
    /// + lights, i.e. everything before the checksum field).
    pub fn compute_checksum(&self) -> u16 {
        let bytes = bytemuck::bytes_of(self);
        // Sum all bytes from offset 0 (report_id) up to but excluding
        // the last 2 bytes (checksum_le).
        let payload = &bytes[..Self::SIZE - 2];
        payload.iter().map(|&b| b as u16).sum()
    }

    /// Read the stored checksum.
    pub fn checksum(&self) -> u16 {
        u16::from_le(self.checksum_le)
    }

    /// Recalculate and set the checksum, report_id, size, and profile_index.
    pub fn finalize(&mut self, profile_index: u8) {
        self.report_id = ReportId::ProfileSettings as u8;
        self.size = Self::SIZE as u8;
        self.profile_index = profile_index;
        self.checksum_le = self.compute_checksum().to_le();
    }

    /// Validate the checksum.
    pub fn validate_checksum(&self) -> bool {
        self.checksum() == self.compute_checksum()
    }
}

// ─── Profile buttons (123 bytes) ───────────────────────────────────

/// Per-profile button mapping (123 bytes on wire).
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct ProfileButtons {
    pub report_id: u8,     // 0x07
    pub size: u8,          // 0x7b = 123
    pub profile_index: u8, // 0–4
    pub buttons: [Button; NYTH_PROFILE_BUTTON_NUM],
}

unsafe impl Zeroable for ProfileButtons {}
unsafe impl Pod for ProfileButtons {}

impl ProfileButtons {
    pub const SIZE: usize = 123;

    pub fn finalize(&mut self, profile_index: u8) {
        self.report_id = ReportId::ProfileButtons as u8;
        self.size = Self::SIZE as u8;
        self.profile_index = profile_index;
    }
}

// ─── Macro (variable, transferred in 2 parts of 1026 bytes) ────────

/// A macro definition. This is NOT directly a HID report — it is
/// transferred to the device in two 1026-byte chunks using report
/// ID 0x08. The struct captures the logical content.
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct Macro {
    pub profile_index: u8,
    pub button_index: u8,
    pub loop_count: u8,
    pub macroset_name: [u8; NYTH_MACRO_MACROSET_NAME_LENGTH],
    pub macro_name: [u8; NYTH_MACRO_MACRO_NAME_LENGTH],
    count_le: u16,
    pub keystrokes: [Keystroke; NYTH_MACRO_KEYSTROKES_NUM],
}

unsafe impl Zeroable for Macro {}
unsafe impl Pod for Macro {}

impl Macro {
    /// Number of valid keystrokes in this macro.
    pub fn count(&self) -> u16 {
        u16::from_le(self.count_le)
    }

    pub fn set_count(&mut self, count: u16) {
        self.count_le = count.to_le();
    }

    /// Get the macro set name as a UTF-8 string (trimmed at first NUL).
    pub fn macroset_name_str(&self) -> &str {
        let end = self
            .macroset_name
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(self.macroset_name.len());
        std::str::from_utf8(&self.macroset_name[..end]).unwrap_or("")
    }

    /// Get the macro name as a UTF-8 string (trimmed at first NUL).
    pub fn macro_name_str(&self) -> &str {
        let end = self
            .macro_name
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(self.macro_name.len());
        std::str::from_utf8(&self.macro_name[..end]).unwrap_or("")
    }
}

// ─── Info (6 bytes) ─────────────────────────────────────────────────

/// Device info / firmware version / reset report (6 bytes).
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct InfoReport {
    pub report_id: u8, // 0x09
    pub size: u8,      // 0x06
    /// When reading: firmware version. When writing: function code.
    pub firmware_or_function: u8,
    pub unknown1: u8,
    pub unknown2: u8,
    pub unknown3: u8,
}

unsafe impl Zeroable for InfoReport {}
unsafe impl Pod for InfoReport {}

impl InfoReport {
    pub const SIZE: usize = 6;

    /// Function code to reset to factory defaults.
    pub const FUNCTION_RESET: u8 = 0x01;

    /// Format firmware version as "X.YZ" string.
    pub fn firmware_version_string(&self) -> String {
        let v = self.firmware_or_function;
        format!("{}.{:02}", v / 100, v % 100)
    }
}

// ─── Button set (30 bytes) ──────────────────────────────────────────

/// Button set / side-button configuration preset (30 bytes).
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct ButtonSet {
    pub report_id: u8, // 0x0a
    pub size: u8,      // 0x1e = 30
    pub profile_index: u8,
    pub name: [u8; NYTH_BUTTON_SET_NAME_LENGTH],
    singles_le: u16,
    pub doubles: u8,
}

unsafe impl Zeroable for ButtonSet {}
unsafe impl Pod for ButtonSet {}

impl ButtonSet {
    pub const SIZE: usize = 30;

    pub fn singles(&self) -> u16 {
        u16::from_le(self.singles_le)
    }

    pub fn set_singles(&mut self, value: u16) {
        self.singles_le = value.to_le();
    }

    /// Get the name as a UTF-8 string (may not be NUL-terminated).
    pub fn name_str(&self) -> &str {
        let end = self
            .name
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(self.name.len());
        std::str::from_utf8(&self.name[..end]).unwrap_or("")
    }
}

// ─── Device state (3 bytes) ─────────────────────────────────────────

/// Device state report — toggles between HID multimedia mode and
/// driver-managed mode (3 bytes).
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct DeviceStateReport {
    pub report_id: u8, // 0x0e
    pub size: u8,      // 0x03
    pub state: u8,
}

unsafe impl Zeroable for DeviceStateReport {}
unsafe impl Pod for DeviceStateReport {}

impl DeviceStateReport {
    pub const SIZE: usize = 3;

    /// Device sends HID multimedia events directly.
    pub const STATE_OFF: u8 = 0x00;
    /// Driver handles multimedia events.
    pub const STATE_ON: u8 = 0x01;

    pub fn new(state: u8) -> Self {
        Self {
            report_id: ReportId::DeviceState as u8,
            size: Self::SIZE as u8,
            state,
        }
    }
}

// ─── Control unit / TCU / DCU (6 bytes) ─────────────────────────────

/// Tracking and distance control unit report (6 bytes).
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct ControlUnitReport {
    pub report_id: u8, // 0x0f
    pub size: u8,      // 0x06
    pub dcu: u8,
    pub tcu: u8,
    pub unused: u8,
    pub action: u8,
}

unsafe impl Zeroable for ControlUnitReport {}
unsafe impl Pod for ControlUnitReport {}

impl ControlUnitReport {
    pub const SIZE: usize = 6;
}

/// Distance control unit (lift-off distance) settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DistanceControl {
    Off = 0,
    ExtraLow = 1,
    Low = 2,
    Normal = 3,
}

/// Tracking control unit actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TcuAction {
    Off = 0x00,
    Start = 0x01,
    Accept = 0x03,
    Cancel = 0x04,
}

// ─── Talk / GFX (16 bytes) ──────────────────────────────────────────

/// Talk report used for EasyShift, EasyAim, and TalkFX lighting (16 bytes).
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct TalkReport {
    pub report_id: u8, // 0x10
    pub size: u8,      // 16
    pub easyshift: u8,
    pub easyshift_lock: u8,
    pub easyaim: u8,
    pub fx_status: u8,
    pub zone: u8,
    pub unused: u8,
    pub effect: u8,
    pub speed: u8,
    pub ambient_red: u8,
    pub ambient_green: u8,
    pub ambient_blue: u8,
    pub event_red: u8,
    pub event_green: u8,
    pub event_blue: u8,
}

unsafe impl Zeroable for TalkReport {}
unsafe impl Pod for TalkReport {}

impl TalkReport {
    pub const SIZE: usize = 16;
}

// ─── Special event (5 bytes, read-only from device) ─────────────────

/// Special event report read from the device's event hidraw endpoint.
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct SpecialReport {
    pub report_id: u8, // 0x03
    pub unused: u8,
    pub event_type: u8,
    pub data1: u8,
    pub data2: u8,
}

unsafe impl Zeroable for SpecialReport {}
unsafe impl Pod for SpecialReport {}

impl SpecialReport {
    pub const SIZE: usize = 5;
}

// ─── Button type enum ───────────────────────────────────────────────

/// All known Nyth button action types.
///
/// These values are stored in [`Button::button_type`] and determine
/// what happens when a physical button is pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[allow(non_camel_case_types)]
pub enum ButtonType {
    Disabled = 0x00,
    Click = 0x01,
    Menu = 0x02,
    UniversalScrolling = 0x03,
    DoubleClick = 0x04,
    Shortcut = 0x05,
    BrowserForward = 0x07,
    BrowserBackward = 0x08,
    TiltLeft = 0x09,
    TiltRight = 0x0a,
    ScrollUp = 0x0d,
    ScrollDown = 0x0e,
    OpenApplication = 0x0f,
    ProfileCycle = 0x10,
    ProfileUp = 0x11,
    ProfileDown = 0x12,
    CpiCycle = 0x14,
    CpiUp = 0x15,
    CpiDown = 0x16,
    SensitivityCycle = 0x17,
    SensitivityUp = 0x18,
    SensitivityDown = 0x19,
    OpenDriver = 0x1b,
    PrevTrack = 0x21,
    NextTrack = 0x22,
    PlayPause = 0x23,
    Stop = 0x24,
    MuteMaster = 0x25,
    VolumeDown = 0x26,
    VolumeUp = 0x27,
    Macro = 0x30,
    Timer = 0x31,
    TimerStop = 0x32,
    EasyAim1 = 0x33,
    EasyAim2 = 0x34,
    EasyAim3 = 0x35,
    EasyAim4 = 0x36,
    EasyAim5 = 0x37,
    EasyShiftSelf = 0x41,
    EasyWheelSensitivity = 0x42,
    EasyWheelProfile = 0x43,
    EasyWheelCpi = 0x44,
    EasyWheelVolume = 0x45,
    EasyWheelAltTab = 0x46,
    EasyWheelAeroFlip3d = 0x47,
    EasyShiftOther = 0x51,
    EasyShiftLockOther = 0x52,
    EasyShiftAll = 0x53,
    Home = 0x62,
    End = 0x63,
    Profile1 = 0xbf,
    Profile2 = 0xc0,
    Profile3 = 0xc1,
    Profile4 = 0xc2,
    Profile5 = 0xc3,
}

// ─── Special event types ────────────────────────────────────────────

/// Event types reported in [`SpecialReport::event_type`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SpecialEventType {
    Profile = 0x20,
    Tcu = 0x30,
    OpenApplication = 0x60,
    TimerStart = 0x80,
    TimerStop = 0x90,
    OpenDriver = 0xa0,
    Cpi = 0xb0,
    Sensitivity = 0xc0,
    Multimedia = 0xf0,
    Windows = 0xf2,
    Open = 0xf3,
    EasyShift = 0xff,
}

// ─── Lighting enums ─────────────────────────────────────────────────

/// Colour-change effect across lighting zones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ColorEffect {
    Off = 0,
    Simultaneously = 1,
    Up = 2,
    Down = 3,
}

/// Light animation effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum LightEffect {
    FullyLighted = 1,
    Blinking = 2,
    Breathing = 3,
    Heartbeat = 4,
}

/// Polling rate values (shared across Roccat devices).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PollingRate {
    Hz125 = 1,
    Hz250 = 2,
    Hz500 = 3,
    Hz1000 = 4,
}

// ─── Compile-time struct size checks ────────────────────────────────

const _: () = {
    assert!(std::mem::size_of::<ControlReport>() == 3);
    assert!(std::mem::size_of::<ProfileReport>() == 4);
    assert!(std::mem::size_of::<ProfileSettings>() == 33);
    assert!(std::mem::size_of::<ProfileButtons>() == 123);
    assert!(std::mem::size_of::<InfoReport>() == 6);
    assert!(std::mem::size_of::<ButtonSet>() == 30);
    assert!(std::mem::size_of::<DeviceStateReport>() == 3);
    assert!(std::mem::size_of::<ControlUnitReport>() == 6);
    assert!(std::mem::size_of::<TalkReport>() == 16);
    assert!(std::mem::size_of::<SpecialReport>() == 5);
    assert!(std::mem::size_of::<Light>() == 4);
    assert!(std::mem::size_of::<Button>() == 3);
    assert!(std::mem::size_of::<Keystroke>() == 4);
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_profile_settings_parses() {
        let settings: ProfileSettings = *bytemuck::from_bytes(&ProfileSettings::DEFAULT_BYTES);
        assert_eq!(settings.report_id, ReportId::ProfileSettings as u8);
        assert_eq!(settings.size, 0x21);
        assert_eq!(settings.profile_index, 0);
        // First CPI level: bytes 7-8 = 0x20, 0x03 → 0x0320 = 800
        assert_eq!(settings.cpi_level(0), 800);
        // Second CPI level: bytes 9-10 = 0x40, 0x06 → 0x0640 = 1600
        assert_eq!(settings.cpi_level(1), 1600);
    }

    #[test]
    fn default_profile_settings_checksum_after_finalize() {
        // The raw default bytes don't carry a valid checksum;
        // the C code always calls finalize() before writing.
        let mut settings: ProfileSettings = *bytemuck::from_bytes(&ProfileSettings::DEFAULT_BYTES);
        settings.finalize(0);
        assert!(settings.validate_checksum());
    }

    #[test]
    fn profile_settings_checksum_roundtrip() {
        let mut settings: ProfileSettings = *bytemuck::from_bytes(&ProfileSettings::DEFAULT_BYTES);
        settings.set_cpi_level(0, 1600);
        settings.finalize(2);
        assert_eq!(settings.profile_index, 2);
        assert!(settings.validate_checksum());
    }

    #[test]
    fn struct_sizes_match_wire_format() {
        // These are also checked at compile time via const assertions,
        // but having runtime tests makes failures easier to diagnose.
        assert_eq!(std::mem::size_of::<ProfileSettings>(), 33);
        assert_eq!(std::mem::size_of::<ProfileButtons>(), 123);
        assert_eq!(std::mem::size_of::<ProfileReport>(), 4);
        assert_eq!(std::mem::size_of::<InfoReport>(), 6);
        assert_eq!(std::mem::size_of::<ButtonSet>(), 30);
        assert_eq!(std::mem::size_of::<ControlUnitReport>(), 6);
        assert_eq!(std::mem::size_of::<TalkReport>(), 16);
        assert_eq!(std::mem::size_of::<SpecialReport>(), 5);
    }

    #[test]
    fn cpi_level_endianness() {
        let mut settings: ProfileSettings = *bytemuck::from_bytes(&ProfileSettings::DEFAULT_BYTES);
        settings.set_cpi_level(3, 4200);
        assert_eq!(settings.cpi_level(3), 4200);
        // Verify the raw LE bytes
        let bytes = bytemuck::bytes_of(&settings);
        // cpi_levels start at offset 7, each is 2 bytes LE
        // Level 3 is at offset 7 + (3 * 2) = 13
        assert_eq!(bytes[13], (4200 & 0xFF) as u8); // 0x68
        assert_eq!(bytes[14], ((4200 >> 8) & 0xFF) as u8); // 0x10
    }
}
