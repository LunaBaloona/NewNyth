//! Human-readable display formatting for Nyth protocol types.
//!
//! Provides name lookups and `Display` implementations used by the
//! CLI and any future GUI.

use crate::protocol::*;

/// Return the human-readable name for a button type byte value.
pub fn button_type_name(raw: u8) -> &'static str {
    match raw {
        0x00 => "Disabled",
        0x01 => "Left click",
        0x02 => "Menu / right click",
        0x03 => "Universal scrolling",
        0x04 => "Double click",
        0x05 => "Shortcut",
        0x07 => "Browser forward",
        0x08 => "Browser backward",
        0x09 => "Tilt left",
        0x0a => "Tilt right",
        0x0d => "Scroll up",
        0x0e => "Scroll down",
        0x0f => "Open application",
        0x10 => "Profile cycle",
        0x11 => "Profile up",
        0x12 => "Profile down",
        0x14 => "CPI cycle",
        0x15 => "CPI up",
        0x16 => "CPI down",
        0x17 => "Sensitivity cycle",
        0x18 => "Sensitivity up",
        0x19 => "Sensitivity down",
        0x1b => "Open driver",
        0x21 => "Previous track",
        0x22 => "Next track",
        0x23 => "Play / pause",
        0x24 => "Stop",
        0x25 => "Mute",
        0x26 => "Volume down",
        0x27 => "Volume up",
        0x30 => "Macro",
        0x31 => "Timer",
        0x32 => "Timer stop",
        0x33 => "EasyAim 1",
        0x34 => "EasyAim 2",
        0x35 => "EasyAim 3",
        0x36 => "EasyAim 4",
        0x37 => "EasyAim 5",
        0x41 => "EasyShift (self)",
        0x42 => "EasyWheel sensitivity",
        0x43 => "EasyWheel profile",
        0x44 => "EasyWheel CPI",
        0x45 => "EasyWheel volume",
        0x46 => "EasyWheel Alt+Tab",
        0x47 => "EasyWheel Aero Flip 3D",
        0x51 => "EasyShift (other)",
        0x52 => "EasyShift lock (other)",
        0x53 => "EasyShift (all)",
        0x62 => "Home",
        0x63 => "End",
        0xbf => "Profile 1",
        0xc0 => "Profile 2",
        0xc1 => "Profile 3",
        0xc2 => "Profile 4",
        0xc3 => "Profile 5",
        _ => "Unknown",
    }
}

/// Physical button names indexed by position (0–19 for normal,
/// 20–39 for EasyShift layer).
pub const BUTTON_NAMES: [&str; NYTH_PROFILE_BUTTON_NUM] = [
    // Normal layer (0–19)
    "Left",
    "Right",
    "Middle",
    "Wheel up",
    "Wheel down",
    "Top",
    "Fin left",
    "Fin right",
    "Side 1",
    "Side 2",
    "Side 3",
    "Side 4",
    "Side 5",
    "Side 6",
    "Side 7",
    "Side 8",
    "Side 9",
    "Side 10",
    "Side 11",
    "Side 12",
    // EasyShift layer (20–39)
    "Shift+Left",
    "Shift+Right",
    "Shift+Middle",
    "Shift+Wheel up",
    "Shift+Wheel down",
    "Shift+Top",
    "Shift+Fin left",
    "Shift+Fin right",
    "Shift+Side 1",
    "Shift+Side 2",
    "Shift+Side 3",
    "Shift+Side 4",
    "Shift+Side 5",
    "Shift+Side 6",
    "Shift+Side 7",
    "Shift+Side 8",
    "Shift+Side 9",
    "Shift+Side 10",
    "Shift+Side 11",
    "Shift+Side 12",
];

/// Return the polling rate in Hz for a raw polling rate byte.
pub fn polling_rate_hz(raw: u8) -> u16 {
    match raw {
        1 => 125,
        2 => 250,
        3 => 500,
        4 => 1000,
        _ => 0,
    }
}

/// Return the name of a light effect byte value.
pub fn light_effect_name(raw: u8) -> &'static str {
    match raw {
        0 => "Off",
        1 => "Fully lit",
        2 => "Blinking",
        3 => "Breathing",
        4 => "Heartbeat",
        _ => "Unknown",
    }
}

/// Return the name of a colour effect byte value.
pub fn color_effect_name(raw: u8) -> &'static str {
    match raw {
        0 => "Off",
        1 => "Simultaneously",
        2 => "Up",
        3 => "Down",
        _ => "Unknown",
    }
}

/// Return the name of a distance control unit setting.
pub fn dcu_name(raw: u8) -> &'static str {
    match raw {
        0 => "Off",
        1 => "Extra low",
        2 => "Low",
        3 => "Normal",
        _ => "Unknown",
    }
}

/// Format a modifier bitmask as a human-readable string (e.g. "Ctrl+Shift").
pub fn modifier_string(modifier: u8) -> String {
    let mut parts = Vec::new();
    if modifier & (1 << 2) != 0 {
        parts.push("Ctrl");
    }
    if modifier & (1 << 1) != 0 {
        parts.push("Shift");
    }
    if modifier & (1 << 3) != 0 {
        parts.push("Alt");
    }
    if modifier & (1 << 4) != 0 {
        parts.push("Win");
    }
    if parts.is_empty() {
        "None".to_string()
    } else {
        parts.join("+")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn button_names_length() {
        assert_eq!(BUTTON_NAMES.len(), NYTH_PROFILE_BUTTON_NUM);
    }

    #[test]
    fn known_button_types() {
        assert_eq!(button_type_name(0x01), "Left click");
        assert_eq!(button_type_name(0x30), "Macro");
        assert_eq!(button_type_name(0xc3), "Profile 5");
        assert_eq!(button_type_name(0xFE), "Unknown");
    }

    #[test]
    fn modifier_formatting() {
        assert_eq!(modifier_string(0), "None");
        assert_eq!(modifier_string(0b00110), "Ctrl+Shift");
    }
}
