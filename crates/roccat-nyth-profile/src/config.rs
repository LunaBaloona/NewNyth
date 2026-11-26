//! NewNyth configuration file management.
//!
//! Reads and writes the JSON files under `~/.config/newnyth/`:
//! `openers.json` (per-profile, per-button launch commands),
//! `autoswitch.json` (window app class to profile bindings),
//! `profile_names.json` (display names) and `settings.json` (app
//! preferences such as the desktop notification switch). The daemon
//! reads them all, the GUI and nythctl write them.
//!
//! Every file carries a `version` field (absent counts as 1), so a
//! later format change can migrate old files instead of failing to
//! parse them. Saves go through a sibling `.tmp` file and `rename`,
//! so a crash can never leave half-written JSON for the daemon to
//! read. A file that fails to parse is copied to `<name>.bak`
//! before anything replaces it, so a hand-edit that broke the syntax
//! is never lost without trace.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

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

/// File format version written into every config file. Bump when the
/// format changes so readers can migrate; files predating the field
/// load as this version (the promise is documented in
/// `docs/API-STABILITY.md`).
pub const CONFIG_FORMAT_VERSION: u32 = 1;

fn default_config_version() -> u32 {
    CONFIG_FORMAT_VERSION
}

/// Load a config file. A missing or unreadable file yields defaults;
/// a file that exists but will not parse is first copied to `.bak`,
/// because callers carry on with defaults and their next save would
/// otherwise overwrite the user's (possibly hand-edited, now broken)
/// file without trace.
fn load_json<T: DeserializeOwned + Default>(path: &Path, what: &str) -> T {
    match std::fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_else(|e| {
            log::warn!("Invalid {what} config at {}: {}", path.display(), e);
            backup_corrupt(path);
            T::default()
        }),
        Err(_) => T::default(),
    }
}

/// Copy an unparseable file to `<file>.bak`. Repeated loads of the
/// same broken file copy once only: an identical backup is skipped.
fn backup_corrupt(path: &Path) {
    let backup = append_suffix(path, ".bak");
    let Ok(current) = std::fs::read(path) else {
        return;
    };
    if std::fs::read(&backup).is_ok_and(|b| b == current) {
        return;
    }
    match std::fs::write(&backup, &current) {
        Ok(()) => log::warn!("Kept a copy of the unreadable file at {}", backup.display()),
        Err(e) => log::warn!("Could not back up {}: {}", path.display(), e),
    }
}

/// Serialise to pretty JSON and write atomically: the bytes land in a
/// sibling `.tmp` file and are flushed to disk, then `rename` swaps
/// them into place in one step. Readers see either the old file or
/// the new one, never a mixture.
fn save_json<T: Serialize>(value: &T, path: &Path) -> Result<(), String> {
    // A symlinked config (stow/chezmoi setups) is written through to
    // its real target, so the link itself survives the atomic rename;
    // a dangling link falls back to becoming a plain file.
    let path = &resolve_target(path);
    let dir = path
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", path.display()))?;
    std::fs::create_dir_all(dir).map_err(|e| format!("Cannot create {}: {}", dir.display(), e))?;
    let json = serde_json::to_string_pretty(value)
        .map_err(|e| format!("JSON serialisation error: {}", e))?;
    let tmp = append_suffix(path, ".tmp");
    let mut file = std::fs::File::create(&tmp)
        .map_err(|e| format!("Cannot write {}: {}", tmp.display(), e))?;
    let written = file
        .write_all(json.as_bytes())
        .and_then(|()| file.sync_all());
    drop(file);
    if let Err(e) = written {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("Cannot write {}: {}", tmp.display(), e));
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!(
            "Cannot replace {} with {}: {}",
            path.display(),
            tmp.display(),
            e
        ));
    }
    Ok(())
}

/// `<path><suffix>` without touching the extension (`openers.json`
/// becomes `openers.json.tmp`).
fn append_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

/// Where a save really lands: the canonical file a symlink points at,
/// or `path` itself when it is not a link or the link target is gone.
fn resolve_target(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Path to the openers config file.
pub fn openers_path() -> PathBuf {
    config_dir().join("openers.json")
}

/// Map of (profile_index, button_index) >> command string.
///
/// Stored as JSON: `{"version": 1, "0": {"8": "firefox", "12": "/usr/bin/dolphin"}, "1": {...}}`
/// Profile indices are 0-based, button indices match the hardware button index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenersConfig {
    /// File format version; files without the field load as
    /// [`CONFIG_FORMAT_VERSION`].
    #[serde(default = "default_config_version")]
    pub version: u32,
    /// Outer key: profile index as string ("0"-"4").
    /// Inner key: button index as string.
    /// Value: command to execute.
    #[serde(flatten)]
    pub profiles: HashMap<String, HashMap<String, String>>,
}

impl Default for OpenersConfig {
    fn default() -> Self {
        Self {
            version: CONFIG_FORMAT_VERSION,
            profiles: HashMap::new(),
        }
    }
}

impl OpenersConfig {
    /// Load from disk. Returns default (empty) if file doesn't exist
    /// or is invalid; an invalid file is first copied to `.bak`.
    pub fn load() -> Self {
        load_json(&openers_path(), "openers")
    }

    /// Save to disk atomically. Creates the config directory if needed.
    pub fn save(&self) -> Result<(), String> {
        save_json(self, &openers_path())
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoSwitchConfig {
    /// File format version; files without the field load as
    /// [`CONFIG_FORMAT_VERSION`].
    #[serde(default = "default_config_version")]
    pub version: u32,
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

impl Default for AutoSwitchConfig {
    fn default() -> Self {
        Self {
            version: CONFIG_FORMAT_VERSION,
            enabled: false,
            default_profile: None,
            profiles: HashMap::new(),
        }
    }
}

impl AutoSwitchConfig {
    pub fn load() -> Self {
        load_json(&autoswitch_path(), "autoswitch")
    }

    pub fn save(&self) -> Result<(), String> {
        save_json(self, &autoswitch_path())
    }

    /// Get the list of app classes for a profile.
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
    /// Returns the profile index (0-4) or None. Profiles are scanned in
    /// ascending order, so a class listed under two profiles (possible
    /// only in a hand-edited config, the GUI refuses it) resolves to the
    /// lowest profile number instead of to HashMap iteration order.
    pub fn profile_for_app(&self, app_class: &str) -> Option<u8> {
        self.app_owner(&app_class.to_lowercase(), None)
    }

    /// Lowest-numbered profile other than `except` whose app list contains
    /// `app_class` (case-insensitive), or None. The GUI uses this to refuse
    /// assigning an app that a different mouse profile already owns, and to
    /// name that profile in the status message.
    pub fn app_owner_other(&self, app_class: &str, except: u8) -> Option<u8> {
        self.app_owner(&app_class.to_lowercase(), Some(except))
    }

    fn app_owner(&self, app_lower: &str, except: Option<u8>) -> Option<u8> {
        (0..roccat_nyth_hid::protocol::NYTH_PROFILE_NUM).find(|&idx| {
            Some(idx) != except
                && self
                    .profiles
                    .get(&idx.to_string())
                    .is_some_and(|apps| apps.iter().any(|a| a.to_lowercase() == app_lower))
        })
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileNames {
    /// File format version; files without the field load as
    /// [`CONFIG_FORMAT_VERSION`].
    #[serde(default = "default_config_version")]
    pub version: u32,
    /// Key: profile index as string ("0"–"4"). Value: display name.
    #[serde(default)]
    pub names: HashMap<String, String>,
}

impl Default for ProfileNames {
    fn default() -> Self {
        Self {
            version: CONFIG_FORMAT_VERSION,
            names: HashMap::new(),
        }
    }
}

impl ProfileNames {
    pub fn load() -> Self {
        load_json(&profile_names_path(), "profile-names")
    }

    pub fn save(&self) -> Result<(), String> {
        save_json(self, &profile_names_path())
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

/// Path to the app settings file.
pub fn settings_path() -> PathBuf {
    config_dir().join("settings.json")
}

/// App-level preferences: not tied to a profile or the device, edited
/// by the GUI and honoured by the daemon. Currently just the desktop
/// notification switch; the daemon's announcements for profile, CPI
/// and sensitivity changes are useful but not everybody's taste, and
/// a switch that only silences the popups is friendlier than a
/// systemd override.
///
/// Stored as JSON: `{"version": 1, "notifications": true}`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppSettings {
    /// File format version; files without the field load as
    /// [`CONFIG_FORMAT_VERSION`].
    #[serde(default = "default_config_version")]
    pub version: u32,
    /// Master switch for the daemon's desktop notifications. Default
    /// on, matching the behaviour before the switch existed.
    #[serde(default = "default_notifications_on")]
    pub notifications: bool,
}

fn default_notifications_on() -> bool {
    true
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            version: CONFIG_FORMAT_VERSION,
            notifications: true,
        }
    }
}

impl AppSettings {
    /// Load from disk. Missing or invalid files mean notifications on
    /// (the historical behaviour); an invalid file is backed up first.
    pub fn load() -> Self {
        load_json(&settings_path(), "app settings")
    }

    /// Save to disk atomically. Creates the config directory if needed.
    pub fn save(&self) -> Result<(), String> {
        save_json(self, &settings_path())
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
    fn profile_for_app_prefers_lowest_profile_on_duplicates() {
        // A duplicate class can only reach the config by hand-editing
        // (the GUI refuses it); resolution must not depend on HashMap
        // iteration order, so the lowest profile number wins. Insert in
        // both orders to pin this down.
        for (first, second) in [(4u8, 2u8), (2, 4)] {
            let mut cfg = AutoSwitchConfig::default();
            cfg.apps_for_profile_mut(first)
                .push("org.kde.kwrite".into());
            cfg.apps_for_profile_mut(second)
                .push("Org.KDE.KWrite".into());
            assert_eq!(cfg.profile_for_app("org.kde.kwrite"), Some(2));
        }
    }

    #[test]
    fn app_owner_other_names_the_conflicting_profile() {
        let mut cfg = AutoSwitchConfig::default();
        cfg.apps_for_profile_mut(2).push("org.kde.kwrite".into());
        cfg.apps_for_profile_mut(4).push("org.kde.kwrite".into());
        assert_eq!(cfg.app_owner_other("org.kde.kwrite", 4), Some(2));
        assert_eq!(cfg.app_owner_other("org.kde.kwrite", 2), Some(4));
        assert_eq!(cfg.app_owner_other("ORG.KDE.KWRITE", 0), Some(2));
        assert_eq!(cfg.app_owner_other("firefox", 0), None);
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

    #[test]
    fn openers_lookup_by_profile_and_button() {
        let mut cfg = OpenersConfig::default();
        cfg.set(0, 3, "firefox --private-window");
        cfg.set(4, 11, "dolphin");
        assert_eq!(cfg.get(0, 3), Some("firefox --private-window"));
        // Neighbouring buttons and other profiles stay unconfigured.
        assert_eq!(cfg.get(0, 4), None);
        assert_eq!(cfg.get(1, 3), None);

        // An empty command removes the entry again.
        cfg.set(0, 3, "");
        assert_eq!(cfg.get(0, 3), None);

        let json = serde_json::to_string(&cfg).unwrap();
        let back: OpenersConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.get(4, 11), Some("dolphin"));
    }

    #[test]
    fn legacy_files_without_version_load_as_current() {
        let cfg: AutoSwitchConfig =
            serde_json::from_str(r#"{"enabled": true, "default_profile": null, "profiles": {}}"#)
                .unwrap();
        assert_eq!(cfg.version, CONFIG_FORMAT_VERSION);

        // The flattened openers map must not swallow the version key,
        // and old files that never had one still load whole.
        let cfg: OpenersConfig = serde_json::from_str(r#"{"0": {"8": "firefox"}}"#).unwrap();
        assert_eq!(cfg.version, CONFIG_FORMAT_VERSION);
        assert_eq!(cfg.get(0, 8), Some("firefox"));

        let cfg: ProfileNames = serde_json::from_str(r#"{"names": {"1": "Work"}}"#).unwrap();
        assert_eq!(cfg.version, CONFIG_FORMAT_VERSION);
        assert_eq!(cfg.get(1), Some("Work"));
    }

    #[test]
    fn saves_write_the_current_format_version() {
        for json in [
            serde_json::to_string(&OpenersConfig::default()).unwrap(),
            serde_json::to_string(&AutoSwitchConfig::default()).unwrap(),
            serde_json::to_string(&ProfileNames::default()).unwrap(),
            serde_json::to_string(&AppSettings::default()).unwrap(),
        ] {
            let value: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert_eq!(value["version"], serde_json::json!(CONFIG_FORMAT_VERSION));
        }
    }

    #[test]
    fn save_through_a_symlink_keeps_the_link() {
        // stow/chezmoi-style setups symlink their configs into the
        // config dir; the atomic save must write through the link and
        // never replace it with a regular file.
        let dir = scratch_dir("symlink");
        let real = dir.join("real-autoswitch.json");
        let link = dir.join("autoswitch.json");
        std::fs::write(&real, "{}\n").unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let mut cfg = AutoSwitchConfig {
            enabled: true,
            ..Default::default()
        };
        cfg.apps_for_profile_mut(0).push("kate".into());
        save_json(&cfg, &link).unwrap();

        assert!(
            std::fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink(),
            "the link itself must survive the atomic rename"
        );
        let back: AutoSwitchConfig =
            serde_json::from_str(&std::fs::read_to_string(&real).unwrap()).unwrap();
        assert!(back.enabled);
        assert_eq!(back.profile_for_app("kate"), Some(0));
        assert!(!dir.join("real-autoswitch.json.tmp").exists());
    }

    #[test]
    fn app_settings_default_to_notifications_on() {
        // The switch must not change behaviour for anybody until they
        // touch it: missing file, missing field or broken file all
        // mean "on", the behaviour before the switch existed.
        assert!(AppSettings::default().notifications);
        let fresh: AppSettings = serde_json::from_str("{}").unwrap();
        assert!(fresh.notifications);

        let dir = scratch_dir("settings");
        let path = dir.join("settings.json");
        let off = AppSettings {
            version: CONFIG_FORMAT_VERSION,
            notifications: false,
        };
        save_json(&off, &path).unwrap();
        let back: AppSettings =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(back, off);
    }

    fn scratch_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "newnyth-config-test-{}-{}",
            label,
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn corrupt_config_is_backed_up_before_defaulting() {
        let dir = scratch_dir("corrupt");
        let path = dir.join("openers.json");
        std::fs::write(&path, "{ this is not json").unwrap();

        let cfg: OpenersConfig = load_json(&path, "openers");
        assert!(cfg.profiles.is_empty());
        assert_eq!(
            std::fs::read_to_string(dir.join("openers.json.bak")).unwrap(),
            "{ this is not json"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn save_swaps_atomically_and_leaves_no_temp_file() {
        let dir = scratch_dir("atomic");
        let path = dir.join("autoswitch.json");
        let mut cfg = AutoSwitchConfig {
            enabled: true,
            ..Default::default()
        };
        cfg.apps_for_profile_mut(1).push("firefox".into());

        // Saving twice exercises both the fresh write and the rename
        // over an existing file.
        save_json(&cfg, &path).unwrap();
        save_json(&cfg, &path).unwrap();

        let back: AutoSwitchConfig =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(back.enabled);
        assert_eq!(back.profile_for_app("firefox"), Some(1));
        assert!(!dir.join("autoswitch.json.tmp").exists());

        std::fs::remove_dir_all(&dir).ok();
    }
}
