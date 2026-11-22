//! Active window watcher for KDE Plasma Wayland.
//!
//! Architecture:
//! 1. A background thread retries KWin script installation until it succeeds
//! 2. When loaded, the KWin script calls our D-Bus → writes active_window.txt
//! 3. A polling thread reads the file, with xdotool as a bootstrap
//!    fallback (used only until the KWin integration has delivered its
//!    first update in this daemon session — xdotool cannot see native
//!    Wayland windows and would otherwise report wrong ones)

use std::process::Command;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use zbus::blocking::{Connection, Proxy, connection::Builder, proxy::Builder as ProxyBuilder};

use roccat_nyth_profile::config;

const POLL_INTERVAL: Duration = Duration::from_millis(300);

/// How long without any file update before the xdotool bootstrap fallback
/// is consulted (only until the KWin integration proves itself once).
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

/// Session connection for calling KWin. zbus 5 applies call timeouts per
/// connection (dbus 0.9 applied them per proxy), so each call site opens
/// its own short-lived connection with the timeout it wants.
fn kwin_connection(timeout: Duration) -> Result<Connection, zbus::Error> {
    Builder::session()
        .map(|b| b.method_timeout(timeout))
        .and_then(|b| b.build())
}

fn kwin_proxy<'a>(
    conn: &'a Connection,
    path: &'a str,
    iface: &'a str,
) -> Result<Proxy<'a>, zbus::Error> {
    let b = ProxyBuilder::new(conn).destination("org.kde.KWin")?;
    let b = b.path(path)?;
    b.interface(iface)?.build()
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

    let conn = match kwin_connection(Duration::from_secs(2)) {
        Ok(c) => c,
        Err(_) => return false,
    };

    let proxy = match kwin_proxy(&conn, "/Scripting", "org.kde.kwin.Scripting") {
        Ok(p) => p,
        Err(_) => return false,
    };

    // Unload any previous instance.
    let _: Result<bool, _> = proxy.call("unloadScript", &("newnyth",));

    // Load the script.
    let result: Result<i32, _> = proxy.call(
        "loadScript",
        &(script_path.to_string_lossy().to_string(), "newnyth"),
    );

    match result {
        Ok(script_id) => {
            log::info!("Loaded KWin watcher script (id={})", script_id);

            // Method 1: call run() on the individual script object.
            // This works even after KWin's initial startup phase has passed.
            let script_obj = format!("/Scripting/Script{}", script_id);
            let run_result: Result<(), _> =
                match kwin_proxy(&conn, &script_obj, "org.kde.kwin.Script") {
                    Ok(script_proxy) => script_proxy.call("run", &()),
                    Err(e) => Err(e),
                };

            match run_result {
                Ok(()) => {
                    log::info!("KWin watcher script started via run()");
                    return true;
                }
                Err(e) => {
                    log::debug!("run() on Script{} failed: {e} - trying start()", script_id);
                }
            }

            // Method 2: fall back to start() on /Scripting.
            let start_result: Result<(), _> = proxy.call("start", &());

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
                "KWin not ready yet - will retry every {}s for up to {}s",
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
    let Ok(conn) = kwin_connection(Duration::from_secs(1)) else {
        return;
    };
    let Ok(proxy) = kwin_proxy(&conn, "/Scripting", "org.kde.kwin.Scripting") else {
        return;
    };
    let _: Result<bool, _> = proxy.call("unloadScript", &("newnyth",));
    log::info!("Unloaded KWin watcher script");
}

/// Spawn the window poller thread.
///
/// Reads from two sources:
/// - `active_window.txt` (primary — written by the KWin script via D-Bus)
/// - `xdotool` (bootstrap fallback — used only while the KWin integration
///   has never delivered an update in this daemon session; see inline note
///   on why falling back on mere staleness corrupts Wayland sessions)
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
            // Start from the file's *current* mtime so contents left over
            // from a previous session are not reported as a fresh
            // activation — only activations written after this point count.
            let mut last_file_modified = std::fs::metadata(&file_path)
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            let mut last_file_update = Instant::now();
            let mut kwin_active = false;

            loop {
                std::thread::sleep(POLL_INTERVAL);

                let mut current_app: Option<String> = None;

                // Source 1: file written by KWin script → D-Bus handler.
                if let Ok(metadata) = std::fs::metadata(&file_path)
                    && let Ok(modified) = metadata.modified()
                    && modified > last_file_modified
                {
                    last_file_modified = modified;
                    last_file_update = Instant::now();

                    if !kwin_active {
                        log::info!("KWin script is active - native Wayland tracking working");
                        kwin_active = true;
                    }

                    if let Ok(content) = std::fs::read_to_string(&file_path) {
                        current_app = parse_active_window(&content);
                    }
                }

                // Source 2: xdotool — bootstrap only, until the KWin
                // integration has proven itself once in this session.
                // xdotool only sees XWayland windows, so with a native
                // Wayland window focused it reports a stale/wrong one;
                // re-trusting it after every 3 s of inactivity bounced
                // the profile back to the default seconds after each
                // correct auto-switch (Wayland + e.g. Brave sessions).
                if current_app.is_none()
                    && has_xdotool
                    && !kwin_active
                    && last_file_update.elapsed() > FILE_STALE_THRESHOLD
                    && let Some(app) = xdotool_get_class()
                {
                    log::debug!("Active window (xdotool fallback): {app}");
                    current_app = Some(app);
                }

                // Send if changed.
                if let Some(app) = current_app
                    && app != last_app
                {
                    last_app = app.clone();
                    log::trace!("Active window: {}", last_app);
                    if tx.send(last_app.clone()).is_err() {
                        break;
                    }
                }
            }

            log::debug!("Window poller stopped");
        })
        .expect("Failed to spawn window poller thread");

    rx
}

/// Parse the active-window file content: the trimmed app class, or
/// `None` while the file is still empty or only whitespace.
fn parse_active_window(content: &str) -> Option<String> {
    let app = content.trim().to_string();
    (!app.is_empty()).then_some(app)
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

#[cfg(test)]
mod tests {
    use super::parse_active_window;

    #[test]
    fn active_window_file_is_trimmed() {
        assert_eq!(
            parse_active_window("  brave-browser\n").as_deref(),
            Some("brave-browser")
        );
    }

    #[test]
    fn blank_active_window_reports_nothing() {
        assert_eq!(parse_active_window("").as_deref(), None);
        assert_eq!(parse_active_window(" \n\t").as_deref(), None);
    }
}
