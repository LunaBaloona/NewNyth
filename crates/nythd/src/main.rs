//! # nythd --- Roccat Nyth event daemon ---
//!
//!  Reads hardware events from the Nyth's hidraw interface and:
//!
//! - Emits D-Bus signals when the profile changes (for the GUI)
//! - Sends desktop notifications for profile, CPI, and sensitivity changes
//! - Opens folders when folder-open buttons are pressed
//! - Enables driver-managed mode so special events route through the daemon
//!
//! Designed to be started by a systemd user service triggered by our udev.

mod dbus_service;
mod events;
mod window_watcher;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use roccat_nyth_profile::NythDevice;

use crate::dbus_service::NythDbusServer;
use crate::events::NythEvent;

/// How often to poll for D-Bus messages when no events arrive (ms).
const DBUS_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// How long to wait between device scan retries.
const DEVICE_RETRY_INTERVAL: Duration = Duration::from_secs(2);

fn main() {
    // Parse simple flags (no clap needed for a daemon).
    let args: Vec<String> = std::env::args().collect();
    let foreground = args.iter().any(|a| a == "--foreground" || a == "-f");
    let verbose = args.iter().any(|a| a == "--verbose" || a == "-v");
    let show_help = args.iter().any(|a| a == "--help" || a == "-h");
    let show_version = args.iter().any(|a| a == "--version" || a == "-V");

    if show_version {
        println!("nythd {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    if show_help {
        println!("nythd — NewNyth event daemon for the Roccat Nyth mouse");
        println!();
        println!("Usage: nythd [OPTIONS]");
        println!();
        println!("Options:");
        println!("  -f, --foreground   Run in foreground (don't detach)");
        println!("  -v, --verbose      Enable verbose logging");
        println!("  -V, --version      Print version information");
        println!("  -h, --help         Show this help");
        println!();
        println!("Normally started by systemd. For testing:");
        println!("  nythd --foreground --verbose");
        return;
    }

    if verbose {
        std::env::set_var("RUST_LOG", "trace");
    } else if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", if foreground { "info" } else { "warn" });
    }
    env_logger::init();

    log::info!(
        "NewNyth daemon (nythd) {} starting",
        env!("CARGO_PKG_VERSION")
    );

    // Set up signal handler for graceful shutdown.
    let running = Arc::new(AtomicBool::new(true));
    let r = Arc::clone(&running);
    if let Err(e) = ctrlc_handler(r) {
        log::warn!("Could not set signal handler: {}", e);
    }

    // Main daemon loop — reconnects if the device is unplugged/replugged.
    while running.load(Ordering::Relaxed) {
        match run_device_session(&running) {
            Ok(()) => {
                log::info!("Device session ended cleanly");
            }
            Err(e) => {
                log::warn!("Device session error: {}", e);
            }
        }

        if running.load(Ordering::Relaxed) {
            log::info!(
                "Waiting {}s before scanning for device again...",
                DEVICE_RETRY_INTERVAL.as_secs()
            );
            // Sleep in small increments so we can respond to signals.
            for _ in 0..(DEVICE_RETRY_INTERVAL.as_millis() / 100) {
                if !running.load(Ordering::Relaxed) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }

    log::info!("nythd stopped");
}

/// Run a single device session: find device, enable driver mode, read events.
/// Returns when the device disconnects or a shutdown signal is received.
fn run_device_session(running: &Arc<AtomicBool>) -> Result<(), Box<dyn std::error::Error>> {
    // Find all Nyth hidraw nodes (sorted: interface 0/mouse first).
    let all_devices = roccat_nyth_hid::device::scan_device_paths();
    if all_devices.is_empty() {
        return Err("No Roccat Nyth devise found".into());
    }

    // Open the mouse interface (first in sorted list) for ioctls.
    let nyth = NythDevice::open_first()?;
    let info = nyth.info();
    log::info!(
        "Connected to {} at {} (interface {:?})",
        info.name,
        info.hidraw_path.display(),
        info.interface_number
    );

    // Enable driver mode so the device sends special events to us.
    if let Err(e) = nyth.enable_driver_mode() {
        log::warn!("Could not enable driver mode: {}", e);
    }

    // Read initial state.
    let active_profile = nyth.read_active_profile().unwrap_or(0);
    log::info!("Active profile: {}", active_profile + 1);

    // Start D-Bus server (optional — daemon still works without it).
    let dbus = NythDbusServer::new(active_profile + 1);

    // Start event readers on ALL Nyth hidraw nodes.
    // The device may send special events on either interface, and
    // which node is "correct" varies between kernel versions and
    // USB host controllers. Reading from all of them is safe — we
    // just ignore non-SpecialReport packets.
    let (merged_tx, event_rx) = std::sync::mpsc::channel();
    let mut reader_count = 0;
    for dev_info in &all_devices {
        let path = dev_info.hidraw_path.to_string_lossy().to_string();
        let tx = merged_tx.clone();
        match events::start_event_reader_with_sender(&path, tx) {
            Ok(()) => {
                reader_count += 1;
                log::info!(
                    "Event reader started on {} (interface {:?})",
                    path,
                    dev_info.interface_number
                );
            }
            Err(e) => {
                log::warn!("Could not start event reader on {}: {}", path, e);
            }
        }
    }
    drop(merged_tx); // Drop our copy so channel closes when all readers exit.

    if reader_count == 0 {
        return Err("Could not start any event readers".into());
    }

    events::send_notification(
        "Roccat Nyth connected",
        &format!("Profile {}", active_profile + 1),
    );

    // Track state for notifications.
    let mut current_profile = active_profile;

    // Load application opener config.
    let mut openers = roccat_nyth_profile::config::OpenersConfig::load();
    log::info!(
        "Loaded openers config from {}",
        roccat_nyth_profile::config::openers_path().display()
    );

    // Load auto-switch config and start window poller.
    let mut autoswitch = roccat_nyth_profile::config::AutoSwitchConfig::load();
    log::info!(
        "Auto-switch: {} (default profile: {:?})",
        if autoswitch.enabled {
            "enabled"
        } else {
            "disabled"
        },
        autoswitch.default_profile
    );

    // Install KWin watcher script (retries in background if KWin isn't ready yet).
    window_watcher::start_kwin_installer();

    let window_rx = window_watcher::start_window_poller();
    let capture_request_path =
        roccat_nyth_profile::config::config_dir().join("capture_request.txt");
    let capture_result_path = roccat_nyth_profile::config::config_dir().join("captured_app.txt");

    // Main event loop.
    while running.load(Ordering::Relaxed) {
        // Process D-Bus messages.
        if let Some(ref dbus) = dbus {
            dbus.process(DBUS_POLL_INTERVAL);

            let state = dbus.state();
            if state.active_profile != current_profile + 1 && state.active_profile >= 1 {
                let new_index = state.active_profile - 1;
                log::info!(
                    "D-Bus: external profile change notified (profile {})",
                    state.active_profile
                );
                current_profile = new_index;
                dbus.emit_profile_changed(state.active_profile);
                events::send_notification(
                    "Profile changed",
                    &format!("Profile {}", state.active_profile),
                );
            }
            if state.config_reload_requested {
                openers = roccat_nyth_profile::config::OpenersConfig::load();
                autoswitch = roccat_nyth_profile::config::AutoSwitchConfig::load();
                log::info!(
                    "Reloaded configs (auto-switch: {})",
                    if autoswitch.enabled {
                        "enabled"
                    } else {
                        "disabled"
                    }
                );
            }
            dbus.clear_reload_flags();
        } else {
            std::thread::sleep(DBUS_POLL_INTERVAL);
        }

        // ── Handle window changes from the poller ───────
        while let Ok(app_class) = window_rx.try_recv() {
            // Check if the GUI requested a capture.
            if capture_request_path.exists() {
                // Don't capture our own window.
                if app_class != "newnyth" && app_class != "nyth-config" {
                    log::info!("Captured app class: {}", app_class);
                    let _ = std::fs::create_dir_all(roccat_nyth_profile::config::config_dir());
                    let _ = std::fs::write(&capture_result_path, &app_class);
                    let _ = std::fs::remove_file(&capture_request_path);
                }
            } else if autoswitch.enabled {
                // Auto-switch: look up which profile this app belongs to.
                if let Some(target) = autoswitch.profile_for_app(&app_class) {
                    if target != current_profile {
                        log::info!("Auto-switch: {} → profile {}", app_class, target + 1);
                        match nyth.write_active_profile(target) {
                            Ok(()) => {
                                current_profile = target;
                                if let Some(ref dbus) = dbus {
                                    dbus.set_active_profile(target + 1);
                                    dbus.emit_profile_changed(target + 1);
                                }
                            }
                            Err(e) => log::warn!("Auto-switch failed: {}", e),
                        }
                    }
                } else if let Some(default_idx) = autoswitch.default_profile {
                    if default_idx != current_profile {
                        log::trace!(
                            "Auto-switch: {} (no match) → default profile {}",
                            app_class,
                            default_idx + 1
                        );
                        match nyth.write_active_profile(default_idx) {
                            Ok(()) => {
                                current_profile = default_idx;
                                if let Some(ref dbus) = dbus {
                                    dbus.set_active_profile(default_idx + 1);
                                    dbus.emit_profile_changed(default_idx + 1);
                                }
                            }
                            Err(e) => log::warn!("Auto-switch default failed: {}", e),
                        }
                    }
                }
            }
        }

        // Drain all pending events from the reader thread.
        loop {
            match event_rx.try_recv() {
                Ok(event) => {
                    handle_event(&event, &nyth, &dbus, &mut current_profile, &openers);

                    if matches!(event, NythEvent::Disconnected) {
                        log::info!("Device disconnected");
                        return Ok(());
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    log::info!("Event reader thread exited");
                    return Ok(());
                }
            }
        }
    }

    // Cleanup.
    window_watcher::unload_kwin_script();

    // Disable driver mode on shutdown if we still have the device.
    if let Err(e) = nyth.disable_driver_mode() {
        log::debug!("Could not disable driver mode on shutdown: {}", e);
    }

    Ok(())
}

/// Process a single event from the hardware.
fn handle_event(
    event: &NythEvent,
    nyth: &NythDevice,
    dbus: &Option<NythDbusServer>,
    current_profile: &mut u8,
    openers: &roccat_nyth_profile::config::OpenersConfig,
) {
    match event {
        NythEvent::ProfileChanged { profile_index } => {
            *current_profile = *profile_index;
            let number = profile_index + 1;
            log::info!("Profile changed to {}", number);

            if let Some(ref dbus) = dbus {
                dbus.emit_profile_changed(number);
            }

            events::send_notification("Profile changed", &format!("Profile {number}"));
        }

        NythEvent::CpiChanged { cpi_index } => {
            let cpi_str = match nyth.read_profile_settings(*current_profile) {
                Ok(settings) => {
                    let cpi = settings.cpi_level(*cpi_index as usize);
                    format!("{cpi} DPI (level {})", cpi_index + 1)
                }
                Err(_) => format!("Level {}", cpi_index + 1),
            };

            log::info!("CPI changed: {}", cpi_str);
            events::send_notification("CPI changed", &cpi_str);
        }

        NythEvent::SensitivityChanged { x, y } => {
            log::info!("Sensitivity changed: X={x} Y={y}");
            events::send_notification("Sensitivity changed", &format!("X={x} Y={y}"));
        }

        NythEvent::WindowsAction { subtype } => {
            if let Some(xdg_dir) = events::xdg_folder_for_windows_subtype(*subtype) {
                events::open_xdg_folder(xdg_dir);
            } else {
                log::debug!("Unsupported Windows action subtype: {:#04x}", subtype);
            }
        }

        NythEvent::OpenDriver => {
            log::info!("Open driver/config requested");
            let _ = std::process::Command::new("newnyth")
                .spawn()
                .or_else(|_| std::process::Command::new("nythctl").spawn());
        }

        NythEvent::OpenApplication { button_index } => {
            if let Some(command) = openers.get(*current_profile, *button_index) {
                log::info!(
                    "Launching application for button {}: {}",
                    button_index,
                    command
                );
                // Split command into program + args (shell-style).
                let parts: Vec<&str> = command.split_whitespace().collect();
                if let Some((program, args)) = parts.split_first() {
                    match std::process::Command::new(program).args(args).spawn() {
                        Ok(_) => {}
                        Err(e) => {
                            log::warn!("Failed to launch '{}': {}", command, e);
                            events::send_notification(
                                "Launch failed",
                                &format!("Could not run: {command}"),
                            );
                        }
                    }
                }
            } else {
                log::info!(
                    "Open application for button {} — no command configured (profile {})",
                    button_index,
                    *current_profile + 1
                );
            }
        }

        NythEvent::TimerStart { button_index } => {
            log::info!("Timer start for button index {}", button_index);
            events::send_notification("Timer started", "");
        }

        NythEvent::TimerStop => {
            log::info!("Timer stop");
            events::send_notification("Timer stopped", "");
        }

        NythEvent::EasyShift {
            button_index,
            action,
        } => {
            let action_str = if *action == 0 { "press" } else { "release" };
            log::trace!("EasyShift button {} {}", button_index, action_str);
        }

        NythEvent::Multimedia { action } => {
            events::simulate_media_key(*action);
        }

        NythEvent::OpenAction {
            subtype,
            button_index,
        } => {
            log::info!(
                "Open action subtype={:#04x} button={}",
                subtype,
                button_index
            );
            // In the original, this opens a document/folder/website URL
            // stored in profile data. The GUI tool (Phase 4) will handle
            // configuring these paths.
        }

        NythEvent::Tcu => {
            log::debug!("TCU calibration event");
        }

        NythEvent::Disconnected => {
            events::send_notification("Roccat Nyth", "Device disconnected");
        }
    }
}

/// Set up SIGINT/SIGTERM handler using a simple pipe-based approach.
fn ctrlc_handler(running: Arc<AtomicBool>) -> Result<(), std::io::Error> {
    // Use unsafe signal handler since we just set an atomic bool.
    unsafe {
        nix::libc::signal(nix::libc::SIGTERM, handle_signal as usize);
        nix::libc::signal(nix::libc::SIGINT, handle_signal as usize);
    }

    // Store the running flag in a static so the signal handler can reach it.
    // This is safe because we only set it once before signals can arrive.
    RUNNING_FLAG.store(Arc::into_raw(running) as usize, Ordering::SeqCst);

    Ok(())
}

static RUNNING_FLAG: AtomicUsize = AtomicUsize::new(0);
use std::sync::atomic::AtomicUsize;

extern "C" fn handle_signal(_sig: i32) {
    let ptr = RUNNING_FLAG.load(Ordering::SeqCst);
    if ptr != 0 {
        let running = unsafe { &*(ptr as *const AtomicBool) };
        running.store(false, Ordering::SeqCst);
    }
}
