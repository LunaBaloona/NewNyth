//! D-Bus service for the NewNyth daemon.
//!
//! Exposes `org.roccat.Nyth` on the session bus at `/org/roccat/Nyth`.
//!
//! **Methods:**
//! - `ProfileChangedOutside(byte)` - another program (GUI, Mini or
//!   nythctl) switched the profile; keeps the daemon's view in step
//! - `ConfigurationChangedOutside()` - reload the JSON config files
//! - `OpenGui()` - launch the NewNyth GUI
//! - `WindowActivated(string)` - called by the KWin script whenever the
//!   active window changes; this is what drives auto-switching
//!
//! **Signals:**
//! - `ProfileChanged(byte)` - the active profile changed on the device
//!
//! The interface is deliberately small, because everything exported at
//! 1.0 must stay stable for all of 1.x (see `docs/API-STABILITY.md`).
//! `StartCapture`, the `AppCaptured` signal and
//! `ProfileDataChangedOutside` were removed before 1.0: the first two
//! did nothing at all and the third set a flag nothing ever read.
//!
//! **Bus name:** the name is intentionally shared with the
//! (unmaintained) roccat-tools daemons, which use the very same
//! `org.roccat.Nyth`. If both are installed the two clash; the name is
//! requested without replacement, so whichever service starts first
//! keeps the bus and the other logs a warning and runs without D-Bus.
//! The name is kept for compatibility with existing integrations and
//! the KWin script.
//!
//! Implemented with the zbus blocking API. Method dispatch runs on zbus's
//! own executor thread, so the main loop no longer pumps the connection.
//! Handlers are `async fn` per the `#[interface]` macro but must keep
//! sync bodies: blocking zbus connections are driven by an async-io
//! reactor, so tokio-specific calls would panic inside them.

use std::sync::{Arc, Mutex};

use zbus::blocking::{Connection, connection::Builder};
use zbus::object_server::SignalEmitter;
use zbus::{Message, interface};

/// Shared state between D-Bus handlers and the main daemon loop.
#[derive(Debug, Clone)]
pub struct DaemonState {
    pub active_profile: u8,
    pub config_reload_requested: bool,
}

impl DaemonState {
    pub fn new(active_profile: u8) -> Self {
        Self {
            active_profile,
            config_reload_requested: false,
        }
    }
}

/// The exported `org.roccat.Nyth` interface object.
struct NythIface {
    state: Arc<Mutex<DaemonState>>,
}

#[interface(name = "org.roccat.Nyth")]
impl NythIface {
    #[zbus(name = "ProfileChangedOutside")]
    async fn profile_changed_outside(&self, number: u8) {
        log::info!("D-Bus: ProfileChangedOutside({})", number);
        // The Nyth has exactly five profiles (1-based on the bus). A
        // nonsense byte must not poison the daemon's view of the active
        // profile (the main loop would later try to act on it).
        if !(1..=5).contains(&number) {
            log::warn!("D-Bus: ignoring out-of-range profile number {number} (expected 1..=5)");
            return;
        }
        if let Ok(mut s) = self.state.lock() {
            s.active_profile = number;
        }
    }

    #[zbus(name = "ConfigurationChangedOutside")]
    async fn configuration_changed_outside(&self) {
        log::info!("D-Bus: ConfigurationChangedOutside");
        if let Ok(mut s) = self.state.lock() {
            s.config_reload_requested = true;
        }
    }

    #[zbus(name = "OpenGui")]
    async fn open_gui(&self) {
        log::info!("D-Bus: OpenGui requested");
        // No nythctl fallback: run with no arguments it only prints
        // help, which the requester would never see. A warning is the
        // honest failure (and launch_app now really fails when the
        // binary is missing).
        if let Err(e) = crate::events::launch_app("newnyth", &[]) {
            log::warn!("OpenGui: could not launch the NewNyth GUI: {e}");
        }
    }

    #[zbus(name = "WindowActivated")]
    async fn window_activated(&self, app_class: String) {
        if !app_class.is_empty() {
            log::trace!("D-Bus: WindowActivated({})", app_class);
            // Write to file for the polling thread to pick up.
            let path = crate::window_watcher::active_window_path();
            let _ = std::fs::write(path, &app_class);
        }
    }

    #[zbus(signal)]
    async fn profile_changed(emitter: &SignalEmitter<'_>, number: u8) -> zbus::Result<()>;
}

pub struct NythDbusServer {
    conn: Connection,
    state: Arc<Mutex<DaemonState>>,
}

impl NythDbusServer {
    pub fn new(initial_profile: u8) -> Option<Self> {
        let state = Arc::new(Mutex::new(DaemonState::new(initial_profile)));

        // Object export and name acquisition in one step. The name is
        // requested only after the interface is registered, so no method
        // call can arrive against a not-yet-exported object. Like the old
        // dbus 0.9 request (replace = false), a taken name does not
        // replace the current owner.
        let built = Builder::session()
            .and_then(|b| {
                b.serve_at(
                    "/org/roccat/Nyth",
                    NythIface {
                        state: Arc::clone(&state),
                    },
                )
            })
            .and_then(|b| b.name("org.roccat.Nyth"))
            .and_then(|b| b.build());

        let conn = match built {
            Ok(c) => c,
            Err(e) => {
                log::warn!("Could not register D-Bus service org.roccat.Nyth: {}", e);
                return None;
            }
        };

        log::info!("D-Bus service registered at org.roccat.Nyth");
        Some(Self { conn, state })
    }

    pub fn emit_profile_changed(&self, profile_number: u8) {
        let signal = match Message::signal("/org/roccat/Nyth", "org.roccat.Nyth", "ProfileChanged")
            .and_then(|b| b.build(&(profile_number,)))
        {
            Ok(msg) => msg,
            Err(e) => {
                log::warn!("Failed to build ProfileChanged signal: {}", e);
                return;
            }
        };
        if let Err(e) = self.conn.send(&signal) {
            log::warn!("Failed to emit ProfileChanged: {}", e);
        }
    }

    pub fn state(&self) -> DaemonState {
        self.state.lock().unwrap().clone()
    }

    pub fn clear_reload_flags(&self) {
        if let Ok(mut s) = self.state.lock() {
            s.config_reload_requested = false;
        }
    }

    /// Update the internal active profile tracking (1-based).
    /// Must be called whenever the daemon changes the profile on the device,
    /// otherwise the D-Bus state check will revert the change.
    pub fn set_active_profile(&self, profile_number: u8) {
        if let Ok(mut s) = self.state.lock() {
            s.active_profile = profile_number;
        }
    }
}
