//! NewNyth configuration file management.
//!
//! Stores per-profile, per-button application launch commands at
//! `~/.config/newnyth/openers.json`. This file is read by the daemon
//! and written by the GUI.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// The config directory: `~/.config/newnyth/`
pub fn config_dir() -> PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
            PathBuf::from(home).join(".config")
        });
    base.join("newnyth")
}

/// Path to the openers config file.
pub fn openers_path() -> PathBuf {
    config_dir().join("openers.json")
}

/// Map of (profile_index, button_index) >> command string.
///
/// Stored as JSON: `{"0": {"8": "firefox", "12": "/usr/bin/dolphin"}, "1": {...}}`
/// Profile indices are 0-based, button indices match the hardware button index.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OpenersConfig {
    /// Outer key: profile index as string ("0"-"4").
    /// Inner key: button index as string.
    /// Value: command to execute.
    #[serde(flatten)]
    pub profiles: HashMap<String, HashMap<String, String>>,
}

impl OpenersConfig {
    /// Load from disk. Returns default (empty) if file doesn't exist or is invalid.
    pub fn load() -> Self {
        let path = openers_path();
        match std::fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_else(|e| {
                log::warn!("Invalid openers config at {}: {}", path.display(), e);
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    /// Save to disk. Creates the config  directory if needed.
    pub fn save(&self) -> Result<(), String> {
        let dir = config_dir();
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("Cannot create {}: {}", dir.display(), e))?;

        let path = openers_path();
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("JSON serialization error: {}", e))?;

        std::fs::write(&path, json).map_err(|e| format!("Cannot write {}: {}", path.display(), e))
    }

    /// Get the command for a (profile, button) pair.
    pub fn get(&self, profile_index: u8, button_index: u8) -> Option<&str> {
        self.profiles
            .get(&profile_index.to_string())
            .and_then(|buttons| buttons.get(&button_index.to_string()))
            .map(|s| s.as_str())
    }

    /// Set the command for a (profile, button) pair.
    pub fn set(&mut self, profile_index: u8, button_index: u8, command: &str) {
        let profile_map = self.profiles.entry(profile_index.to_string()).or_default();
        if command.is_empty() {
            profile_map.remove(&button_index.to_string());
        } else {
            profile_map.insert(button_index.to_string(), command.to_string());
        }
    }
}

// ────────────── Auto-switch config ─────────────────────────────────────────────

/// Maximum number of app bindings per profile. Bumped from 8 for public release. May extend this later.
pub const MAX_APPS_PER_PROFILE: usize = 10;

/// Path to the auto-switch config file.
pub fn autoswitch_path() -> PathBuf {
    config_dir().join("autoswitch.json")
}

/// Auto-switch configuration: maps window app classes to profiles.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AutoSwitchConfig {
    /// Whether auto-switching is enabled.
    pub enabled: bool,
    /// Default profile index (0–4) when no matched app is active.
    /// `None` means "don't switch when no match" (i.e. stay on current).
    pub default_profile: Option<u8>,
    /// Per-profile list of app class names.
    /// Key: profile index as string ("0"–"4").
    /// Value: list of window class strings (e.g. "firefox", "kate").
    pub profiles: HashMap<String, Vec<String>>,
}

impl AutoSwitchConfig {
    pub fn load() -> Self {
        let path = autoswitch_path();
        match std::fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_else(|e| {
                log::warn!("Invalid autoswitch config at {}: {}", path.display(), e);
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let dir = config_dir();
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("Cannot create {}: {}", dir.display(), e))?;
        let path = autoswitch_path();
        let json = serde_json::to_string_pretty(self).map_err(|e| format!("JSON error: {}", e))?;
        std::fs::write(&path, json).map_err(|e| format!("Cannot write {}: {}", path.display(), e))
    }

    /// Get the list of app classes for a profile (NB ret from v3.18sep23).
    pub fn apps_for_profile(&self, profile_index: u8) -> &[String] {
        self.profiles
            .get(&profile_index.to_string())
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Get a mutable reference to the app list for a profile, creating it if needed.
    pub fn apps_for_profile_mut(&mut self, profile_index: u8) -> &mut Vec<String> {
        self.profiles.entry(profile_index.to_string()).or_default()
    }

    /// Find which profile (if any) matches a given app class.
    /// Returns the profile index (0–4) or None.
    pub fn profile_for_app(&self, app_class: &str) -> Option<u8> {
        let app_lower = app_class.to_lowercase();
        for (key, apps) in &self.profiles {
            if apps.iter().any(|a| a.to_lowercase() == app_lower)
                && let Ok(idx) = key.parse::<u8>()
            {
                return Some(idx);
            }
        }
        None
    }
}

// ────────────── Profile names ────────────────────────────────────────────────

/// Path to the profile-names config file.
pub fn profile_names_path() -> PathBuf {
    config_dir().join("profile_names.json")
}

/// Maximum length of a profile display name, in characters.
pub const MAX_PROFILE_NAME_LEN: usize = 32;

/// Profile display names, typed in the GUI and stored on the PC.
///
/// The Nyth hardware has no name field, so these names live in this file
/// only: tooltips and the auto-switch cards show them, nothing device
/// facing depends on them.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProfileNames {
    /// Key: profile index as string ("0"–"4"). Value: display name.
    #[serde(default)]
    pub names: HashMap<String, String>,
}

impl ProfileNames {
    pub fn load() -> Self {
        let path = profile_names_path();
        match std::fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_else(|e| {
                log::warn!("Invalid profile-names config at {}: {}", path.display(), e);
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let dir = config_dir();
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("Cannot create {}: {}", dir.display(), e))?;
        let path = profile_names_path();
        let json = serde_json::to_string_pretty(self).map_err(|e| format!("JSON error: {}", e))?;
        std::fs::write(&path, json).map_err(|e| format!("Cannot write {}: {}", path.display(), e))
    }

    /// Display name for a profile, or None when unset or blank.
    pub fn get(&self, profile_index: u8) -> Option<&str> {
        self.names
            .get(&profile_index.to_string())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
    }

    /// Set the display name; an empty or blank name clears the entry.
    pub fn set(&mut self, profile_index: u8, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            self.names.remove(&profile_index.to_string());
        } else {
            self.names
                .insert(profile_index.to_string(), name.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_for_app_matches_case_insensitively() {
        let mut cfg = AutoSwitchConfig::default();
        cfg.apps_for_profile_mut(1).push("brave-browser".into());
        assert_eq!(cfg.profile_for_app("brave-browser"), Some(1));
        assert_eq!(cfg.profile_for_app("Brave-Browser"), Some(1));
        assert_eq!(cfg.profile_for_app("firefox"), None);
    }

    #[test]
    fn json_roundtrip_preserves_bindings() {
        let mut cfg = AutoSwitchConfig {
            enabled: true,
            default_profile: Some(0),
            ..Default::default()
        };
        cfg.apps_for_profile_mut(1).push("brave-browser".into());
        cfg.apps_for_profile_mut(4).push("org.kde.dolphin".into());

        let json = serde_json::to_string(&cfg).unwrap();
        let back: AutoSwitchConfig = serde_json::from_str(&json).unwrap();

        assert!(back.enabled);
        assert_eq!(back.default_profile, Some(0));
        assert_eq!(back.profile_for_app("brave-browser"), Some(1));
        assert_eq!(back.profile_for_app("org.kde.dolphin"), Some(4));
    }

    #[test]
    fn profile_names_roundtrip_and_clearing() {
        let mut names = ProfileNames::default();
        names.set(0, "Gaming");
        names.set(2, "  Work  ");
        assert_eq!(names.get(0), Some("Gaming"));
        assert_eq!(names.get(2), Some("Work"));
        assert_eq!(names.get(1), None);

        let json = serde_json::to_string(&names).unwrap();
        let back: ProfileNames = serde_json::from_str(&json).unwrap();
        assert_eq!(back.get(0), Some("Gaming"));

        // Blank names remove the entry again.
        names.set(0, "   ");
        assert_eq!(names.get(0), None);
        assert!(!names.names.contains_key("0"));
    }
}
