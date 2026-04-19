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
                let _ = std::process::Command::new("xdg-open").arg(&path).spawn();
            }
        }
        _ => {
            log::warn!("Could not determine XDG directory: {}", xdg_name);
        }
    }
}

/// Send a desktop notification via notify-send (available on KDE Plasma).
pub fn send_notification(summary: &str, body: &str) {
    let _ = std::process::Command::new("notify-send")
        .arg("--app-name=nythd")
        .arg("--icon=input-mouse")
        .arg("--expire-time=2000")
        .arg(summary)
        .arg(body)
        .spawn();
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
                "Media: unknown action byte {:#04x} — please report this to \
                 https://github.com/LunaBaloona/NewNyth/issues so we can add support",
                other
            );
        }
    }
}

fn run_media_cmd(program: &str, args: &[&str]) {
    match std::process::Command::new(program).args(args).spawn() {
        Ok(_) => {}
        Err(e) => {
            log::warn!("Failed to run '{}': {} — is it installed?", program, e);
        }
    }
}
