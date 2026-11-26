//! Hidraw event reader for the Nyth.
//!
//! Opens the hidraw device node for raw reads (not feature reports) and
//! blocks on `read()` to receive 5-byte SpecialReport events. Each event
//! is parsed into a typed [`NythEvent`] and sent through a channel.

use std::io::Read;
use std::os::unix::fs::OpenOptionsExt;
use std::sync::mpsc;

use roccat_nyth_hid::protocol::{ReportId, SpecialReport};

/// Typed event parsed from a [`SpecialReport`].
#[derive(Debug, Clone)]
pub enum NythEvent {
    /// Active profile changed. `profile_index` is 0-based.
    ProfileChanged { profile_index: u8 },

    /// CPI level changed. `cpi_index` is 0-based.
    CpiChanged { cpi_index: u8 },

    /// Sensitivity changed.
    SensitivityChanged { x: u8, y: u8 },

    /// "Open application" button pressed.
    OpenApplication { button_index: u8 },

    /// Timer start requested.
    TimerStart { button_index: u8 },

    /// Timer stop requested.
    TimerStop,

    /// "Open driver / config GUI" button pressed.
    OpenDriver,

    /// Windows / folder action requested.
    WindowsAction { subtype: u8 },

    /// Open document/folder/website action.
    OpenAction { subtype: u8, button_index: u8 },

    /// EasyShift event (press/release).
    EasyShift { button_index: u8, action: u8 },

    /// Multimedia key event. `action` is the media key type.
    Multimedia { action: u8 },

    /// TCU (tracking control unit) calibration event.
    Tcu,

    /// Device disconnected (HUP on the file descriptor).
    Disconnected,
}

/// Parse a raw 5-byte SpecialReport into a typed NythEvent.
fn parse_event(report: &SpecialReport) -> Option<NythEvent> {
    match report.event_type {
        0x20 => Some(NythEvent::ProfileChanged {
            profile_index: report.data1.saturating_sub(1),
        }),
        0x30 => Some(NythEvent::Tcu),
        0x60 => {
            if report.data2 == 0x00 {
                // Press only
                Some(NythEvent::OpenApplication {
                    button_index: report.data1.saturating_sub(1),
                })
            } else {
                None
            }
        }
        0x80 => {
            if report.data2 == 0x00 {
                Some(NythEvent::TimerStart {
                    button_index: report.data1.saturating_sub(1),
                })
            } else {
                None
            }
        }
        0x90 => {
            if report.data2 == 0x00 {
                Some(NythEvent::TimerStop)
            } else {
                None
            }
        }
        0xa0 => {
            if report.data1 == 0x00 {
                Some(NythEvent::OpenDriver)
            } else {
                None
            }
        }
        0xb0 => Some(NythEvent::CpiChanged {
            cpi_index: report.data1.saturating_sub(1),
        }),
        0xc0 => Some(NythEvent::SensitivityChanged {
            x: report.data1,
            y: report.data2,
        }),
        0xf0 => {
            // Multimedia event — the mouse sends this when a media button type
            // (0x21–0x27) is pressed in driver mode. data1 encodes the action.
            if report.data2 == 0x00 {
                // Press event only (data2=0 for press, non-zero for release).
                Some(NythEvent::Multimedia {
                    action: report.data1,
                })
            } else {
                None
            }
        }
        0xf2 => {
            if report.data2 == 0x00 {
                Some(NythEvent::WindowsAction {
                    subtype: report.data1,
                })
            } else {
                None
            }
        }
        0xf3 => Some(NythEvent::OpenAction {
            subtype: report.data1,
            button_index: report.data2.saturating_sub(1),
        }),
        0xff => Some(NythEvent::EasyShift {
            button_index: report.data1.saturating_sub(1),
            action: report.data2,
        }),
        // RAD events (0xe1–0xea) — ignored
        0xe1..=0xea => None,
        other => {
            log::debug!("Unknown event type: {:#04x}", other);
            None
        }
    }
}

/// Spawn a thread that reads raw HID events and sends parsed events
/// through the returned receiver.
///
/// The thread runs until the device is disconnected (EOF/HUP) or the
/// receiver is dropped.
#[allow(dead_code)] // Convenience wrapper; daemon uses start_event_reader_with_sender directly.
pub fn start_event_reader(hidraw_path: &str) -> std::io::Result<mpsc::Receiver<NythEvent>> {
    let (tx, rx) = mpsc::channel();
    start_event_reader_with_sender(hidraw_path, tx)?;
    Ok(rx)
}

/// Spawn a thread that reads raw HID events and sends them through
/// the provided sender. Multiple readers can share one channel.
pub fn start_event_reader_with_sender(
    hidraw_path: &str,
    tx: mpsc::Sender<NythEvent>,
) -> std::io::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(0) // blocking
        .open(hidraw_path)?;

    let path_owned = hidraw_path.to_string();

    std::thread::Builder::new()
        .name(format!(
            "nythd-reader-{}",
            path_owned.rsplit('/').next().unwrap_or("?")
        ))
        .spawn(move || {
            log::debug!("Event reader thread running on {}", path_owned);
            let mut buf = [0u8; 64]; // Read up to 64 bytes; reports vary in size.

            loop {
                match file.read(&mut buf) {
                    Ok(0) => {
                        log::info!("Event reader: EOF on {}", path_owned);
                        let _ = tx.send(NythEvent::Disconnected);
                        break;
                    }
                    Ok(n) if n < SpecialReport::SIZE => {
                        // Short read — not a special report, ignore.
                        continue;
                    }
                    Ok(_) => {
                        if buf[0] != ReportId::Special as u8 {
                            continue;
                        }
                        let report: SpecialReport =
                            *bytemuck::from_bytes(&buf[..SpecialReport::SIZE]);
                        if let Some(event) = parse_event(&report) {
                            log::trace!("{}: {:?}", path_owned, event);
                            if tx.send(event).is_err() {
                                log::debug!("Event receiver dropped, stopping reader");
                                break;
                            }
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(e) => {
                        log::warn!("Event reader error on {}: {}", path_owned, e);
                        let _ = tx.send(NythEvent::Disconnected);
                        break;
                    }
                }
            }

            log::debug!("Event reader stopped on {}", path_owned);
        })?;

    Ok(())
}

/// Map a Windows subtype byte to the XDG user directory to open,
/// or None if unsupported on Linux.
pub fn xdg_folder_for_windows_subtype(subtype: u8) -> Option<&'static str> {
    match subtype {
        0x02 => Some("DESKTOP"),
        0x05 => Some("DOCUMENTS"),
        0x06 => Some("DOWNLOAD"),
        0x07 => Some("MUSIC"),
        0x08 => Some("PICTURES"),
        _ => None,
    }
}

/// Spawn a short-lived helper program and reap it on a helper thread so it
/// never lingers as a zombie. Fire-and-forget `spawn()` without a `wait()`
/// leaves the exited child as `<defunct>`; the daemon runs for weeks, so
/// unreaped children pile up until the process table complains.
pub fn spawn_reaped(program: &str, args: &[&str]) -> Result<(), std::io::Error> {
    let mut child = std::process::Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    let name = program.to_string();
    std::thread::spawn(move || match child.wait() {
        Ok(status) if !status.success() => {
            log::debug!("Helper '{name}' exited with {status}");
        }
        Ok(_) => {}
        Err(e) => log::warn!("Failed to reap helper '{name}': {e}"),
    });
    Ok(())
}

/// Launch a user-facing program (GUI, file manager, application buttons)
/// outside the daemon's systemd control group via a transient scope.
/// Without the scope the unit's default `KillMode=control-group` kills every
/// application the mouse launched whenever nythd restarts. `systemd-run`
/// still keeps the process as our child (cgroup isolation only), so it is
/// reaped like any other helper. Falls back to a plain spawn on systems
/// without systemd-run.
pub fn launch_app(program: &str, args: &[&str]) -> Result<(), std::io::Error> {
    // `systemd-run` wraps any command line - including one whose program
    // does not exist - and reports success: the spawn succeeds and
    // nothing ever runs, so a typo would look like a NewNyth bug
    // (2026-11 review round). Resolve the target first so a missing
    // program surfaces as a real error to the caller's logging.
    let resolved = resolve_program(program).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("program not found: {program}"),
        )
    })?;
    let resolved = resolved.to_string_lossy();
    if systemd_run_available() {
        let mut wrapped: Vec<&str> = Vec::with_capacity(args.len() + 5);
        wrapped.extend_from_slice(&["--user", "--scope", "--collect", "--"]);
        wrapped.push(&resolved);
        wrapped.extend_from_slice(args);
        match spawn_reaped("systemd-run", &wrapped) {
            Ok(()) => return Ok(()),
            Err(e) => log::debug!("systemd-run unavailable ({e}); spawning directly"),
        }
    }
    spawn_reaped(&resolved, args)
}

/// Locate `program` for launching: an explicit path (anything with a
/// `/`) must exist as a file; a bare name is looked up on `PATH`,
/// mirroring what a shell would resolve. `None` means nothing
/// launchable by that name exists.
fn resolve_program(program: &str) -> Option<std::path::PathBuf> {
    let path = std::path::Path::new(program);
    if program.contains('/') {
        return (path.is_file()).then(|| path.to_path_buf());
    }
    let path_var = std::env::var_os("PATH")?;
    std::env::split_paths(&path_var)
        .map(|dir| dir.join(program))
        .find(|candidate| candidate.is_file())
}

fn systemd_run_available() -> bool {
    static AVAILABLE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        std::process::Command::new("systemd-run")
            .arg("--version")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    })
}

/// Open a folder using xdg-open (works on KDE Plasma and other DEs).
pub fn open_xdg_folder(xdg_name: &str) {
    // xdg-user-dir returns the path for the given XDG directory
    let output = std::process::Command::new("xdg-user-dir")
        .arg(xdg_name)
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !path.is_empty() {
                log::info!("Opening folder: {}", path);
                if let Err(e) = launch_app("xdg-open", &[&path]) {
                    log::warn!("Failed to open {}: {}", path, e);
                }
            }
        }
        _ => {
            log::warn!("Could not determine XDG directory: {}", xdg_name);
        }
    }
}

/// Master switch for desktop notifications (the Settings tab's
/// "Desktop notifications" checkbox, persisted in `settings.json` and
/// reloaded by the daemon on config change). Status popups are handy
/// but not universal; a stuck-informative daemon should be silenced
/// without a systemd override. Defaults to on, the behaviour before
/// the switch existed.
static NOTIFICATIONS_ENABLED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(true);

/// Set the notification switch (called at daemon startup and on every
/// config reload).
pub fn set_notifications_enabled(on: bool) {
    NOTIFICATIONS_ENABLED.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// Whether desktop notifications are currently wanted.
pub fn notifications_enabled() -> bool {
    NOTIFICATIONS_ENABLED.load(std::sync::atomic::Ordering::Relaxed)
}

/// Send a desktop notification via notify-send (available on KDE Plasma).
/// Silently dropped while the Settings switch is off; callers keep
/// logging unconditionally, so nothing is lost but the popup.
pub fn send_notification(summary: &str, body: &str) {
    if !notifications_enabled() {
        return;
    }
    let _ = spawn_reaped(
        "notify-send",
        &[
            "--app-name=nythd",
            "--icon=input-mouse",
            "--expire-time=2000",
            summary,
            body,
        ],
    );
}

/// Simulate a multimedia key action.
///
/// Uses `playerctl` for playback and `pactl` for volume — these work
/// on both X11 and Wayland without any special setup.
///
/// The `action` byte comes from the 0xf0 SpecialReport.data1 field.
/// We map known values and also log unknown ones for debugging.
pub fn simulate_media_key(action: u8) {
    // The Nyth encodes multimedia actions in data1. The exact mapping
    // varies slightly between firmware versions. We handle all known
    // patterns: both sequential (1,2,3...) and button-type-based
    // (0x21,0x22,...) encodings.
    match action {
        // Play/pause
        0x01 | 0x20 | 0x23 => {
            log::info!("Media: Play/Pause");
            run_media_cmd("playerctl", &["play-pause"]);
        }
        // Stop
        0x02 | 0x30 | 0x24 => {
            log::info!("Media: Stop");
            run_media_cmd("playerctl", &["stop"]);
        }
        // Previous track
        0x03 | 0x40 | 0x21 => {
            log::info!("Media: Previous track");
            run_media_cmd("playerctl", &["previous"]);
        }
        // Next track
        0x04 | 0x50 | 0x22 => {
            log::info!("Media: Next track");
            run_media_cmd("playerctl", &["next"]);
        }
        // Volume up
        0x05 | 0x60 | 0x27 => {
            log::info!("Media: Volume up");
            run_media_cmd("pactl", &["set-sink-volume", "@DEFAULT_SINK@", "+5%"]);
        }
        // Volume down
        0x06 | 0x70 | 0x26 => {
            log::info!("Media: Volume down");
            run_media_cmd("pactl", &["set-sink-volume", "@DEFAULT_SINK@", "-5%"]);
        }
        // Mute
        0x07 | 0x80 | 0x25 => {
            log::info!("Media: Mute toggle");
            run_media_cmd("pactl", &["set-sink-mute", "@DEFAULT_SINK@", "toggle"]);
        }
        other => {
            log::info!(
                "Media: unknown action byte {:#04x} - please report this to \
                 https://github.com/LunaBaloona/NewNyth/issues so we can add support",
                other
            );
        }
    }
}

fn run_media_cmd(program: &str, args: &[&str]) {
    // P3-3: the old fire-and-forget spawn turned failing children into silent
    // no-ops (e.g. playerctl exits 1 with "No player could be found" when no
    // MPRIS player is registered, which looked like the media buttons dying
    // at random). Pipe the output and reap the child on a helper thread so
    // the HID event loop never blocks, logging anything the child reports.
    let spawned = std::process::Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();
    match spawned {
        Ok(child) => {
            let program = program.to_owned();
            std::thread::spawn(move || match child.wait_with_output() {
                Ok(out) if out.status.success() => {
                    log::debug!("Media cmd '{}' ok", program);
                }
                Ok(out) => {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    let first_line = stderr
                        .trim()
                        .lines()
                        .next()
                        .filter(|line| !line.is_empty())
                        .unwrap_or("(no stderr output)");
                    let first_line: String = first_line.chars().take(200).collect();
                    log::warn!(
                        "Media cmd '{}' failed with {}: {}",
                        program,
                        out.status,
                        first_line
                    );
                }
                Err(e) => {
                    log::warn!("Media cmd '{}': could not wait for child: {}", program, e);
                }
            });
        }
        Err(e) => {
            log::warn!("Failed to run '{}': {} - is it installed?", program, e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{NythEvent, parse_event, resolve_program, xdg_folder_for_windows_subtype};
    use roccat_nyth_hid::protocol::SpecialReport;

    fn report(event_type: u8, data1: u8, data2: u8) -> SpecialReport {
        SpecialReport {
            report_id: 0x03,
            unused: 0,
            event_type,
            data1,
            data2,
        }
    }

    #[test]
    fn profile_change_decodes_zero_based() {
        assert!(matches!(
            parse_event(&report(0x20, 3, 0)),
            Some(NythEvent::ProfileChanged { profile_index: 2 })
        ));
        // A raw 0 or an out-of-range byte saturates instead of wrapping.
        assert!(matches!(
            parse_event(&report(0x20, 0, 0)),
            Some(NythEvent::ProfileChanged { profile_index: 0 })
        ));
    }

    #[test]
    fn tcu_event_passes_through() {
        assert!(matches!(
            parse_event(&report(0x30, 0, 0)),
            Some(NythEvent::Tcu)
        ));
    }

    #[test]
    fn press_only_events_ignore_releases() {
        // Press: data2 == 0 is the gate for all of these.
        assert!(matches!(
            parse_event(&report(0x60, 5, 0)),
            Some(NythEvent::OpenApplication { button_index: 4 })
        ));
        assert!(matches!(
            parse_event(&report(0x80, 2, 0)),
            Some(NythEvent::TimerStart { button_index: 1 })
        ));
        assert!(matches!(
            parse_event(&report(0x90, 0, 0)),
            Some(NythEvent::TimerStop)
        ));
        assert!(matches!(
            parse_event(&report(0xf0, 0x23, 0)),
            Some(NythEvent::Multimedia { action: 0x23 })
        ));
        assert!(matches!(
            parse_event(&report(0xf2, 0x02, 0)),
            Some(NythEvent::WindowsAction { subtype: 0x02 })
        ));

        // Release: a non-zero data2 must not fire the action again.
        assert!(parse_event(&report(0x60, 5, 1)).is_none());
        assert!(parse_event(&report(0x80, 2, 1)).is_none());
        assert!(parse_event(&report(0x90, 0, 1)).is_none());
        assert!(parse_event(&report(0xf0, 0x23, 1)).is_none());
        assert!(parse_event(&report(0xf2, 0x02, 1)).is_none());
    }

    #[test]
    fn open_driver_requires_zero_data1() {
        assert!(matches!(
            parse_event(&report(0xa0, 0, 0)),
            Some(NythEvent::OpenDriver)
        ));
        assert!(parse_event(&report(0xa0, 1, 0)).is_none());
    }

    #[test]
    fn cpi_and_sensitivity_fields() {
        assert!(matches!(
            parse_event(&report(0xb0, 4, 0)),
            Some(NythEvent::CpiChanged { cpi_index: 3 })
        ));
        // Sensitivity is not press-gated; both axes pass through.
        assert!(matches!(
            parse_event(&report(0xc0, 7, 9)),
            Some(NythEvent::SensitivityChanged { x: 7, y: 9 })
        ));
    }

    #[test]
    fn open_action_and_easishift_fields() {
        assert!(matches!(
            parse_event(&report(0xf3, 0x01, 3)),
            Some(NythEvent::OpenAction {
                subtype: 0x01,
                button_index: 2
            })
        ));
        assert!(matches!(
            parse_event(&report(0xff, 6, 1)),
            Some(NythEvent::EasyShift {
                button_index: 5,
                action: 1
            })
        ));
    }

    #[test]
    fn rad_and_unknown_events_are_ignored() {
        for event_type in 0xe1..=0xea {
            assert!(
                parse_event(&report(event_type, 1, 0)).is_none(),
                "RAD event {event_type:#04x} must be ignored"
            );
        }
        assert!(parse_event(&report(0x00, 1, 0)).is_none());
        assert!(parse_event(&report(0x11, 1, 0)).is_none());
    }

    #[test]
    fn windows_subtypes_map_to_xdg_dirs() {
        assert_eq!(xdg_folder_for_windows_subtype(0x02), Some("DESKTOP"));
        assert_eq!(xdg_folder_for_windows_subtype(0x05), Some("DOCUMENTS"));
        assert_eq!(xdg_folder_for_windows_subtype(0x06), Some("DOWNLOAD"));
        assert_eq!(xdg_folder_for_windows_subtype(0x07), Some("MUSIC"));
        assert_eq!(xdg_folder_for_windows_subtype(0x08), Some("PICTURES"));
        assert_eq!(xdg_folder_for_windows_subtype(0x01), None);
        assert_eq!(xdg_folder_for_windows_subtype(0x09), None);
    }

    #[test]
    fn resolve_program_finds_path_executables_and_rejects_nonsense() {
        let sh = resolve_program("sh").expect("sh must resolve via PATH");
        assert!(sh.is_file());
        assert!(resolve_program("/bin/sh").is_some());
        assert!(resolve_program("/bin/definitely-not-here").is_none());
        // A bogus bare name must fail rather than be wrapped into a
        // systemd-run scope that silently runs nothing: this is what
        // makes a typo'd Open-application command report a real error.
        assert!(resolve_program("newnyth-test-missing-program").is_none());
    }
}
