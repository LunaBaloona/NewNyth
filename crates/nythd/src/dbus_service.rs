//! D-Bus service for the NewNyth daemon.
//!
//! Exposes `org.roccat.Nyth` on the session bus at `/org/roccat/Nyth`.
//!
//! **Methods:**
//! - `ProfileChangedOutside(byte)` — external profile change notification
//! - `ProfileDataChangedOutside(byte)` — reload profile data
//! - `ConfigurationChangedOutside()` — reload config
//! - `OpenGui()` — launch the NewNyth GUI
//! - `WindowActivated(string)` — called by KWin script when active window changes
//! - `StartCapture()` — enter capture mode (next WindowActivated emits AppCaptured)
//!
//! **Signals:**
//! - `ProfileChanged(byte)` — profile changed on device
//! - `AppCaptured(string)` — emitted in capture mode with the captured app class

use std::sync::{Arc, Mutex};

use dbus::blocking::Connection;
use dbus::channel::{MatchingReceiver, Sender};
use dbus_crossroads::{Crossroads, IfaceBuilder};

/// Shared state between D-Bus handlers and the main daemon loop.
#[derive(Debug, Clone)]
pub struct DaemonState {
    pub active_profile: u8,
    pub profile_data_reload_requested: Option<u8>,
    pub config_reload_requested: bool,
}

impl DaemonState {
    pub fn new(active_profile: u8) -> Self {
        Self {
            active_profile,
            profile_data_reload_requested: None,
            config_reload_requested: false,
        }
    }
}

pub struct NythDbusServer {
    conn: Connection,
    state: Arc<Mutex<DaemonState>>,
}

impl NythDbusServer {
    pub fn new(initial_profile: u8) -> Option<Self> {
        let conn = match Connection::new_session() {
            Ok(c) => c,
            Err(e) => {
                log::warn!("Could not connect to session D-Bus: {}", e);
                return None;
            }
        };

        if let Err(e) = conn.request_name("org.roccat.Nyth", false, true, false) {
            log::warn!("Could not acquire D-Bus name org.roccat.Nyth: {}", e);
            return None;
        }

        let state = Arc::new(Mutex::new(DaemonState::new(initial_profile)));
        let state_clone = Arc::clone(&state);

        let mut cr = Crossroads::new();

        let iface_token = cr.register("org.roccat.Nyth", move |b: &mut IfaceBuilder<()>| {
            // ── Signals ──────────────────────────────────
            b.signal::<(u8,), _>("ProfileChanged", ("number",));
            b.signal::<(String,), _>("AppCaptured", ("app_class",));

            // ── ProfileChangedOutside ────────────────────
            let st = Arc::clone(&state_clone);
            b.method(
                "ProfileChangedOutside",
                ("number",),
                (),
                move |_, _, (number,): (u8,)| {
                    log::info!("D-Bus: ProfileChangedOutside({})", number);
                    if let Ok(mut s) = st.lock() {
                        s.active_profile = number;
                    }
                    Ok(())
                },
            );

            // ── ProfileDataChangedOutside ────────────────
            let st = Arc::clone(&state_clone);
            b.method(
                "ProfileDataChangedOutside",
                ("number",),
                (),
                move |_, _, (number,): (u8,)| {
                    log::info!("D-Bus: ProfileDataChangedOutside({})", number);
                    if let Ok(mut s) = st.lock() {
                        s.profile_data_reload_requested = Some(number);
                    }
                    Ok(())
                },
            );

            // ── ConfigurationChangedOutside ──────────────
            let st = Arc::clone(&state_clone);
            b.method("ConfigurationChangedOutside", (), (), move |_, _, _: ()| {
                log::info!("D-Bus: ConfigurationChangedOutside");
                if let Ok(mut s) = st.lock() {
                    s.config_reload_requested = true;
                }
                Ok(())
            });

            // ── OpenGui ──────────────────────────────────
            b.method("OpenGui", (), (), |_, _, _: ()| {
                log::info!("D-Bus: OpenGui requested");
                let _ = std::process::Command::new("newnyth")
                    .spawn()
                    .or_else(|_| std::process::Command::new("nythctl").spawn());
                Ok(())
            });

            // ── WindowActivated (called by KWin script) ────────────
            b.method(
                "WindowActivated",
                ("app_class",),
                (),
                move |_, _, (app_class,): (String,)| {
                    if !app_class.is_empty() {
                        log::trace!("D-Bus: WindowActivated({})", app_class);
                        // Write to file for the polling thread to pick up.
                        let path = crate::window_watcher::active_window_path();
                        let _ = std::fs::write(path, &app_class);
                    }
                    Ok(())
                },
            );

            // ── StartCapture (legacy — handled via capture_request.txt) ─
            b.method("StartCapture", (), (), move |_, _, _: ()| {
                log::trace!("D-Bus: StartCapture — handled via file");
                Ok(())
            });
        });

        cr.insert("/org/roccat/Nyth", &[iface_token], ());

        conn.start_receive(
            dbus::message::MatchRule::new_method_call(),
            Box::new(move |msg, conn| {
                cr.handle_message(msg, conn).unwrap();
                true
            }),
        );

        log::info!("D-Bus service registered at org.roccat.Nyth");
        Some(Self { conn, state })
    }

    pub fn process(&self, timeout: std::time::Duration) {
        let _ = self.conn.process(timeout);
    }

    pub fn emit_profile_changed(&self, profile_number: u8) {
        let signal = dbus::message::Message::signal(
            &dbus::strings::Path::from("/org/roccat/Nyth"),
            &dbus::strings::Interface::from("org.roccat.Nyth"),
            &dbus::strings::Member::from("ProfileChanged"),
        )
        .append1(profile_number);
        if let Err(e) = self.conn.send(signal) {
            log::warn!("Failed to emit ProfileChanged: {:?}", e);
        }
    }

    pub fn state(&self) -> DaemonState {
        self.state.lock().unwrap().clone()
    }

    pub fn clear_reload_flags(&self) {
        if let Ok(mut s) = self.state.lock() {
            s.profile_data_reload_requested = None;
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
