//! Device state management for the NewNyth GUI.

use roccat_nyth_hid::protocol::*;
use roccat_nyth_profile::NythDevice;
use roccat_nyth_profile::config::{AutoSwitchConfig, OpenersConfig, ProfileNames};

/// Cached state for a single profile.
#[derive(Clone)]
pub struct ProfileState {
    pub settings: ProfileSettings,
    pub buttons: ProfileButtons,
}

/// All device state needed by the GUI.
pub struct DeviceState {
    pub device_name: String,
    pub hidraw_path: String,
    pub firmware: String,
    pub active_profile: u8,
    pub profiles: Vec<Option<ProfileState>>,
    pub connected: bool,
    pub error_message: Option<String>,
    pub openers: OpenersConfig,
    pub autoswitch: AutoSwitchConfig,
    /// PC-side profile display names (the device has no name field).
    pub profile_names: ProfileNames,
    /// Device-level TCU/DCU state, read on (re)connect.
    pub control_unit: Option<ControlUnitReport>,
}

impl DeviceState {
    pub fn disconnected() -> Self {
        Self {
            device_name: String::new(),
            hidraw_path: String::new(),
            firmware: String::new(),
            active_profile: 0,
            profiles: vec![None; NYTH_PROFILE_NUM as usize],
            connected: false,
            error_message: None,
            openers: OpenersConfig::default(),
            autoswitch: AutoSwitchConfig::default(),
            profile_names: ProfileNames::default(),
            control_unit: None,
        }
    }

    pub fn load_from_device() -> Self {
        let nyth = match NythDevice::open_first() {
            Ok(n) => n,
            Err(e) => {
                let mut state = Self::disconnected();
                state.error_message = Some(format!("No device found: {}", e));
                return state;
            }
        };

        let info = nyth.info();
        let mut state = DeviceState {
            device_name: info.name.clone(),
            hidraw_path: info.hidraw_path.to_string_lossy().to_string(),
            firmware: nyth.read_firmware_version().unwrap_or_else(|_| "?".into()),
            active_profile: nyth.read_active_profile().unwrap_or(0),
            profiles: vec![None; NYTH_PROFILE_NUM as usize],
            connected: true,
            error_message: None,
            openers: OpenersConfig::load(),
            autoswitch: AutoSwitchConfig::load(),
            profile_names: ProfileNames::load(),
            control_unit: nyth.read_control_unit().ok(),
        };

        for i in 0..NYTH_PROFILE_NUM {
            let settings = match nyth.read_profile_settings(i) {
                Ok(s) => s,
                Err(e) => {
                    log::warn!("Could not read profile {} settings: {}", i + 1, e);
                    continue;
                }
            };
            let buttons = match nyth.read_profile_buttons(i) {
                Ok(b) => b,
                Err(e) => {
                    log::warn!("Could not read profile {} buttons: {}", i + 1, e);
                    continue;
                }
            };
            state.profiles[i as usize] = Some(ProfileState { settings, buttons });
        }

        state
    }

    pub fn apply_active_profile(&self, index: u8) -> Result<(), String> {
        let nyth = NythDevice::open_first().map_err(|e| e.to_string())?;
        nyth.write_active_profile(index).map_err(|e| e.to_string())
    }

    /// Read just the active-profile byte. Polled at ~1 Hz so the profile
    /// indicator follows external changes (e.g. the daemon's auto-switch).
    pub fn read_active_profile(&self) -> Result<u8, String> {
        let nyth = NythDevice::open_first().map_err(|e| e.to_string())?;
        nyth.read_active_profile().map_err(|e| e.to_string())
    }

    pub fn apply_profile_settings(
        &self,
        index: u8,
        settings: &mut ProfileSettings,
    ) -> Result<(), String> {
        let nyth = NythDevice::open_first().map_err(|e| e.to_string())?;
        nyth.write_profile_settings(index, settings)
            .map_err(|e| e.to_string())?;
        if index == self.active_profile {
            let _ = nyth.write_active_profile(index);
        }
        Ok(())
    }

    pub fn apply_profile_buttons(
        &self,
        index: u8,
        buttons: &mut ProfileButtons,
    ) -> Result<(), String> {
        let nyth = NythDevice::open_first().map_err(|e| e.to_string())?;
        nyth.write_profile_buttons(index, buttons)
            .map_err(|e| e.to_string())?;
        if index == self.active_profile {
            let _ = nyth.write_active_profile(index);
        }
        Ok(())
    }

    /// Re-read the device-level control unit (TCU/DCU) so the UI can
    /// reflect the confirmed state after a write.
    pub fn refresh_control_unit(&mut self) {
        self.control_unit = NythDevice::open_first()
            .and_then(|nyth| nyth.read_control_unit())
            .ok();
    }

    /// Set the lift-off distance (distance control unit). Device-level:
    /// applies immediately to every profile.
    pub fn apply_distance_control(&self, dcu: DistanceControl) -> Result<(), String> {
        let nyth = NythDevice::open_first().map_err(|e| e.to_string())?;
        nyth.write_distance_control(dcu).map_err(|e| e.to_string())
    }

    /// Turn the tracking control unit off. Device-level and immediate;
    /// switching it back on goes through the calibration wizard.
    pub fn apply_tracking_control_off(&self) -> Result<(), String> {
        let nyth = NythDevice::open_first().map_err(|e| e.to_string())?;
        nyth.write_tracking_control_off().map_err(|e| e.to_string())
    }

    /// Start the surface-calibration handshake (see the wizard).
    pub fn apply_tracking_control_start(&self) -> Result<(), String> {
        let nyth = NythDevice::open_first().map_err(|e| e.to_string())?;
        nyth.write_tracking_control_start()
            .map_err(|e| e.to_string())
    }

    /// Commit a finished calibration (answers "keep" to the wizard).
    pub fn apply_tracking_control_accept(&self) -> Result<(), String> {
        let nyth = NythDevice::open_first().map_err(|e| e.to_string())?;
        nyth.write_tracking_control_accept()
            .map_err(|e| e.to_string())
    }

    /// Abandon a calibration (answers "discard", or cancels mid-test).
    pub fn apply_tracking_control_cancel(&self) -> Result<(), String> {
        let nyth = NythDevice::open_first().map_err(|e| e.to_string())?;
        nyth.write_tracking_control_cancel()
            .map_err(|e| e.to_string())
    }

    // ── TCU calibration event watch ──────────────────────────────
    //
    // The daemon owns the event hidraw endpoints, so the wizard cannot
    // read the calibration-complete event itself. It asks the daemon to
    // forward that one event through two files in the config dir, the
    // same handshake style as the window capture above: writing the
    // request, polling for the result, deleting both on teardown.

    /// Arm a one-shot forward of the next TCU calibration event.
    pub fn start_tcu_watch() {
        let dir = roccat_nyth_profile::config::config_dir();
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::remove_file(dir.join("tcu_event.txt"));
        let _ = std::fs::write(dir.join("tcu_watch.txt"), "1");
    }

    /// True once the daemon has forwarded the calibration event
    /// (consumes the result file).
    pub fn poll_tcu_event() -> bool {
        let path = roccat_nyth_profile::config::config_dir().join("tcu_event.txt");
        std::fs::remove_file(&path).is_ok()
    }

    /// Disarm the watch and drop any unconsumed event (wizard teardown).
    pub fn cancel_tcu_watch() {
        let dir = roccat_nyth_profile::config::config_dir();
        let _ = std::fs::remove_file(dir.join("tcu_watch.txt"));
        let _ = std::fs::remove_file(dir.join("tcu_event.txt"));
    }

    /// Whether the nythd service currently owns its D-Bus name. The
    /// calibration wizard needs it to see the device's completion event.
    pub fn daemon_alive() -> bool {
        let Ok(conn) = dbus::blocking::Connection::new_session() else {
            return false;
        };
        let proxy = conn.with_proxy(
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            std::time::Duration::from_millis(500),
        );
        let result: Result<(bool,), _> =
            proxy.method_call("org.freedesktop.DBus", "NameHasOwner", ("org.roccat.Nyth",));
        result.map(|(has_owner,)| has_owner).unwrap_or(false)
    }

    pub fn save_openers(&self) -> Result<(), String> {
        self.openers.save()
    }

    pub fn save_autoswitch(&self) -> Result<(), String> {
        self.autoswitch.save()
    }

    pub fn save_profile_names(&self) -> Result<(), String> {
        self.profile_names.save()
    }

    /// Tell the daemon to reload its config files.
    ///
    /// Returns an error if the session bus or the daemon is unreachable.
    /// The daemon also polls its config files' mtime, so a failed nudge
    /// only delays pickup by roughly a second — it never loses the change.
    pub fn notify_daemon_config_changed() -> Result<(), String> {
        let conn = dbus::blocking::Connection::new_session()
            .map_err(|e| format!("no session D-Bus: {}", e))?;
        let proxy = conn.with_proxy(
            "org.roccat.Nyth",
            "/org/roccat/Nyth",
            std::time::Duration::from_millis(500),
        );
        let result: Result<(), _> =
            proxy.method_call("org.roccat.Nyth", "ConfigurationChangedOutside", ());
        result.map_err(|e| format!("daemon call failed: {}", e))
    }

    /// Ask the daemon to capture the next focused window.
    /// Creates a request file that the daemon checks in its polling loop.
    pub fn start_capture() {
        let dir = roccat_nyth_profile::config::config_dir();
        let _ = std::fs::create_dir_all(&dir);
        // Clear any previous result.
        let _ = std::fs::remove_file(dir.join("captured_app.txt"));
        // Write the request file — the daemon watches for this.
        let _ = std::fs::write(dir.join("capture_request.txt"), "1");
    }

    /// Cancel a pending capture.
    pub fn cancel_capture() {
        let dir = roccat_nyth_profile::config::config_dir();
        let _ = std::fs::remove_file(dir.join("capture_request.txt"));
        let _ = std::fs::remove_file(dir.join("captured_app.txt"));
    }

    /// Check if a captured app class is available.
    pub fn poll_captured_app() -> Option<String> {
        let path = roccat_nyth_profile::config::config_dir().join("captured_app.txt");
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                let app = content.trim().to_string();
                let _ = std::fs::remove_file(&path);
                if app.is_empty() { None } else { Some(app) }
            }
            Err(_) => None,
        }
    }
}

// ─── Button type list (Shortcut and Open application at top) ────────

pub const ASSIGNABLE_BUTTON_TYPES: &[(u8, &str)] = &[
    // User-configurable actions first
    (0x05, "Shortcut"),
    (0x0f, "Open application"),
    // Separator: standard actions
    (0x00, "Disabled"),
    // Mouse clicks
    (0x01, "Left click"),
    (0x02, "Right click / Menu"),
    (0x03, "Universal scrolling"),
    (0x04, "Double click"),
    // Scroll / tilt
    (0x0d, "Scroll up"),
    (0x0e, "Scroll down"),
    (0x09, "Tilt left"),
    (0x0a, "Tilt right"),
    // Navigation
    (0x07, "Browser forward"),
    (0x08, "Browser backward"),
    (0x62, "Home"),
    (0x63, "End"),
    // Profile
    (0x10, "Profile cycle"),
    (0x11, "Profile up"),
    (0x12, "Profile down"),
    (0xbf, "Profile 1"),
    (0xc0, "Profile 2"),
    (0xc1, "Profile 3"),
    (0xc2, "Profile 4"),
    (0xc3, "Profile 5"),
    // CPI
    (0x14, "CPI cycle"),
    (0x15, "CPI up"),
    (0x16, "CPI down"),
    // Sensitivity
    (0x17, "Sensitivity cycle"),
    (0x18, "Sensitivity up"),
    (0x19, "Sensitivity down"),
    // Media
    (0x21, "Previous track"),
    (0x22, "Next track"),
    (0x23, "Play / pause"),
    (0x24, "Stop"),
    (0x25, "Mute"),
    (0x26, "Volume down"),
    (0x27, "Volume up"),
    // EasyShift
    (0x41, "EasyShift (self)"),
    (0x51, "EasyShift (other)"),
    (0x52, "EasyShift lock (other)"),
    (0x53, "EasyShift (all)"),
    // EasyWheel
    (0x42, "EasyWheel sensitivity"),
    (0x43, "EasyWheel profile"),
    (0x44, "EasyWheel CPI"),
    (0x45, "EasyWheel volume"),
    (0x46, "EasyWheel Alt+Tab"),
    // EasyAim
    (0x33, "EasyAim 1"),
    (0x34, "EasyAim 2"),
    (0x35, "EasyAim 3"),
    (0x36, "EasyAim 4"),
    (0x37, "EasyAim 5"),
];

// ──── HID Usage ID keycode table ────────────────────────────────────
//
// Standard USB HID keyboard usage IDs. The mouse stores these in the
// Button.key field when the type is Shortcut (0x05). (NB remember re-enable extended F-keys)

pub const HID_KEYS: &[(u8, &str)] = &[
    (0x00, "(none)"),
    // Letters
    (0x04, "A"),
    (0x05, "B"),
    (0x06, "C"),
    (0x07, "D"),
    (0x08, "E"),
    (0x09, "F"),
    (0x0A, "G"),
    (0x0B, "H"),
    (0x0C, "I"),
    (0x0D, "J"),
    (0x0E, "K"),
    (0x0F, "L"),
    (0x10, "M"),
    (0x11, "N"),
    (0x12, "O"),
    (0x13, "P"),
    (0x14, "Q"),
    (0x15, "R"),
    (0x16, "S"),
    (0x17, "T"),
    (0x18, "U"),
    (0x19, "V"),
    (0x1A, "W"),
    (0x1B, "X"),
    (0x1C, "Y"),
    (0x1D, "Z"),
    // Numbers
    (0x1E, "1"),
    (0x1F, "2"),
    (0x20, "3"),
    (0x21, "4"),
    (0x22, "5"),
    (0x23, "6"),
    (0x24, "7"),
    (0x25, "8"),
    (0x26, "9"),
    (0x27, "0"),
    // Control keys
    (0x28, "Enter"),
    (0x29, "Escape"),
    (0x2A, "Backspace"),
    (0x2B, "Tab"),
    (0x2C, "Space"),
    // Symbols
    (0x2D, "- (minus)"),
    (0x2E, "= (equals)"),
    (0x2F, "[ (left bracket)"),
    (0x30, "] (right bracket)"),
    (0x31, "\\ (backslash)"),
    (0x33, "; (semicolon)"),
    (0x34, "' (apostrophe)"),
    (0x35, "` (grave)"),
    (0x36, ", (comma)"),
    (0x37, ". (period)"),
    (0x38, "/ (slash)"),
    // Lock keys
    (0x39, "Caps Lock"),
    // Function keys F1–F12
    (0x3A, "F1"),
    (0x3B, "F2"),
    (0x3C, "F3"),
    (0x3D, "F4"),
    (0x3E, "F5"),
    (0x3F, "F6"),
    (0x40, "F7"),
    (0x41, "F8"),
    (0x42, "F9"),
    (0x43, "F10"),
    (0x44, "F11"),
    (0x45, "F12"),
    // System keys
    (0x46, "Print Screen"),
    (0x47, "Scroll Lock"),
    (0x48, "Pause"),
    (0x49, "Insert"),
    (0x4A, "Home"),
    (0x4B, "Page Up"),
    (0x4C, "Delete"),
    (0x4D, "End"),
    (0x4E, "Page Down"),
    // Arrow keys
    (0x4F, "Right Arrow"),
    (0x50, "Left Arrow"),
    (0x51, "Down Arrow"),
    (0x52, "Up Arrow"),
    // Numpad
    (0x53, "Num Lock"),
    (0x54, "Numpad /"),
    (0x55, "Numpad *"),
    (0x56, "Numpad -"),
    (0x57, "Numpad +"),
    (0x58, "Numpad Enter"),
    (0x59, "Numpad 1"),
    (0x5A, "Numpad 2"),
    (0x5B, "Numpad 3"),
    (0x5C, "Numpad 4"),
    (0x5D, "Numpad 5"),
    (0x5E, "Numpad 6"),
    (0x5F, "Numpad 7"),
    (0x60, "Numpad 8"),
    (0x61, "Numpad 9"),
    (0x62, "Numpad 0"),
    (0x63, "Numpad ."),
    // Additional keys
    (0x65, "Menu / Application"),
    // Extended function keys F13–F24
    (0x68, "F13"),
    (0x69, "F14"),
    (0x6A, "F15"),
    (0x6B, "F16"),
    (0x6C, "F17"),
    (0x6D, "F18"),
    (0x6E, "F19"),
    (0x6F, "F20"),
    (0x70, "F21"),
    (0x71, "F22"),
    (0x72, "F23"),
    (0x73, "F24"),
];

/// Look up a HID key name by its usage ID.
pub fn hid_key_name(key: u8) -> &'static str {
    HID_KEYS
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, name)| *name)
        .unwrap_or("Unknown")
}
