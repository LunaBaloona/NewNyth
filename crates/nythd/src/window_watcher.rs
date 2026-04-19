//! Active window watcher for KDE Plasma Wayland.
//!
//! Architecture:
//! 1. A background thread retries KWin script installation until it succeeds
//! 2. When loaded, the KWin script calls our D-Bus → writes active_window.txt
//! 3. A polling thread reads the file, with xdotool as a smart fallback
//!    (xdotool is only used when the file hasn't been updated recently,
//!    i.e. when the KWin script isn't working yet)

use std::process::Command;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use dbus::blocking::Connection;

use roccat_nyth_profile::config;

const POLL_INTERVAL: Duration = Duration::from_millis(300);

/// How long the file can be stale before we fall back to xdotool.
const FILE_STALE_THRESHOLD: Duration = Duration::from_secs(3);

/// How often to retry KWin script installation.
const KWIN_RETRY_INTERVAL: Duration = Duration::from_secs(5);

/// How long to keep retrying KWin script installation.
const KWIN_RETRY_TIMEOUT: Duration = Duration::from_secs(120);

const KWIN_SCRIPT: &str = r#"
workspace.windowActivated.connect(function(window) {
    if (window) {
        var appClass = window.resourceClass || window.resourceName || "";
        if (appClass !== "") {
            callDBus("org.roccat.Nyth", "/org/roccat/Nyth",
                     "org.roccat.Nyth", "WindowActivated", appClass.toString());
        }
    }
});
"#;

/// Path to the active window file.
pub fn active_window_path() -> std::path::PathBuf {
    config::config_dir().join("active_window.txt")
}

/// Try to install the KWin script once. Returns true on success.
fn try_install_kwin_script() -> bool {
    let dir = config::config_dir();
    if std::fs::create_dir_all(&dir).is_err() {
        return false;
    }

    let script_path = dir.join("kwin_watcher.js");
    if std::fs::write(&script_path, KWIN_SCRIPT).is_err() {
        return false;
    }

    let conn = match Connection::new_session() {
        Ok(c) => c,
        Err(_) => return false,
    };

    let proxy = conn.with_proxy("org.kde.KWin", "/Scripting", Duration::from_secs(2));

    // Unload any previous instance.
    let _: Result<(bool,), _> = proxy.method_call(
        "org.kde.kwin.Scripting",
        "unloadScript",
        ("newnyth".to_string(),),
    );

    // Load the script.
    let result: Result<(i32,), _> = proxy.method_call(
        "org.kde.kwin.Scripting",
        "loadScript",
        (
            script_path.to_string_lossy().to_string(),
            "newnyth".to_string(),
        ),
    );

    match result {
        Ok((script_id,)) => {
            log::info!("Loaded KWin watcher script (id={})", script_id);

            // Method 1: call run() on the individual script object.
            // This works even after KWin's initial startup phase has passed.
            let script_proxy = conn.with_proxy(
                "org.kde.KWin",
                format!("/Scripting/Script{}", script_id),
                Duration::from_secs(2),
            );
            let run_result: Result<(), _> =
                script_proxy.method_call("org.kde.kwin.Script", "run", ());

            match run_result {
                Ok(()) => {
                    log::info!("KWin watcher script started via run()");
                    return true;
                }
                Err(e) => {
                    log::debug!("run() on Script{} failed: {e} — trying start()", script_id);
                }
            }

            // Method 2: fall back to start() on /Scripting.
            let start_result: Result<(), _> =
                proxy.method_call("org.kde.kwin.Scripting", "start", ());

            if let Err(e) = start_result {
                log::warn!("Both run() and start() failed: {e}");
                return false;
            }

            log::info!("KWin watcher script started via start()");
            true
        }
        Err(e) => {
            log::debug!("KWin script load failed (will retry): {e}");
            false
        }
    }
}

/// Spawn a background thread that keeps trying to install the KWin script
/// until it succeeds or times out. Non-blocking — returns immediately.
pub fn start_kwin_installer() {
    std::thread::Builder::new()
        .name("newnyth-kwin-installer".into())
        .spawn(|| {
            // First attempt — may succeed if KWin is already running.
            if try_install_kwin_script() {
                return;
            }

            log::info!(
                "KWin not ready yet — will retry every {}s for up to {}s",
                KWIN_RETRY_INTERVAL.as_secs(),
                KWIN_RETRY_TIMEOUT.as_secs()
            );

            let start = Instant::now();
            loop {
                std::thread::sleep(KWIN_RETRY_INTERVAL);

                if start.elapsed() > KWIN_RETRY_TIMEOUT {
                    log::warn!(
                        "KWin script installation timed out after {}s. \
                         Auto-switch will rely on xdotool (XWayland apps only).",
                        KWIN_RETRY_TIMEOUT.as_secs()
                    );
                    return;
                }

                if try_install_kwin_script() {
                    return;
                }
            }
        })
        .expect("Failed to spawn KWin installer thread");
}

/// Unload the KWin script on shutdown.
pub fn unload_kwin_script() {
    let conn = match Connection::new_session() {
        Ok(c) => c,
        Err(_) => return,
    };
    let proxy = conn.with_proxy("org.kde.KWin", "/Scripting", Duration::from_secs(1));
    let _: Result<(bool,), _> = proxy.method_call(
        "org.kde.kwin.Scripting",
        "unloadScript",
        ("newnyth".to_string(),),
    );
    log::info!("Unloaded KWin watcher script");
}

/// Spawn the window poller thread.
///
/// Reads from two sources with smart fallback:
/// - `active_window.txt` (primary — written by KWin script via D-Bus)
/// - `xdotool` (fallback — only used when the file is stale, meaning
///   the KWin script isn't working yet or isn't installed)
pub fn start_window_poller() -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::channel();

    let has_xdotool = Command::new("xdotool")
        .args(["getactivewindow", "getwindowclassname"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if has_xdotool {
        log::info!("xdotool available as fallback for when KWin script isn't ready");
    }

    let file_path = active_window_path();

    std::thread::Builder::new()
        .name("newnyth-window-poller".into())
        .spawn(move || {
            let mut last_app = String::new();
            let mut last_file_modified = std::time::SystemTime::UNIX_EPOCH;
            let mut last_file_update = Instant::now();
            let mut kwin_announced = false;

            loop {
                std::thread::sleep(POLL_INTERVAL);

                let mut current_app: Option<String> = None;

                // Source 1: file written by KWin script → D-Bus handler.
                if let Ok(metadata) = std::fs::metadata(&file_path) {
                    if let Ok(modified) = metadata.modified() {
                        if modified > last_file_modified {
                            last_file_modified = modified;
                            last_file_update = Instant::now();

                            if !kwin_announced {
                                log::info!(
                                    "KWin script is active — native Wayland tracking working"
                                );
                                kwin_announced = true;
                            }

                            if let Ok(content) = std::fs::read_to_string(&file_path) {
                                let app = content.trim().to_string();
                                if !app.is_empty() {
                                    current_app = Some(app);
                                }
                            }
                        }
                    }
                }

                // Source 2: xdotool — ONLY if the file is stale (KWin not working).
                let file_is_stale = last_file_update.elapsed() > FILE_STALE_THRESHOLD;
                if current_app.is_none() && has_xdotool && file_is_stale {
                    if let Some(app) = xdotool_get_class() {
                        current_app = Some(app);
                    }
                }

                // Send if changed.
                if let Some(app) = current_app {
                    if app != last_app {
                        last_app = app.clone();
                        log::trace!("Active window: {}", last_app);
                        if tx.send(last_app.clone()).is_err() {
                            break;
                        }
                    }
                }
            }

            log::debug!("Window poller stopped");
        })
        .expect("Failed to spawn window poller thread");

    rx
}

fn xdotool_get_class() -> Option<String> {
    let output = Command::new("xdotool")
        .args(["getactivewindow", "getwindowclassname"])
        .output()
        .ok()?;

    if output.status.success() {
        let class = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !class.is_empty() {
            return Some(class);
        }
    }

    None
}
