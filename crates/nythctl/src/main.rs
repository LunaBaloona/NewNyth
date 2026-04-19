use std::process;

use clap::{Parser, Subcommand};

use roccat_nyth_hid::display;
use roccat_nyth_hid::protocol::*;
use roccat_nyth_profile::NythDevice;

/// NewNyth CLI — control tool for the Roccat Nyth mouse.
///
/// Provides read/write access to profiles, CPI levels, button
/// mappings, lighting, firmware version, and device state.
/// Run with no subcommand to see a full device status overview.
#[derive(Parser)]
#[command(name = "nythctl", version, about, long_about = None)]
struct Cli {
    /// Enable verbose debug logging (set RUST_LOG=trace for maximum detail).
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Show full device status overview (default if no command given).
    Status,

    /// Show or change the active profile.
    Profile {
        #[command(subcommand)]
        action: Option<ProfileAction>,
    },

    /// Show profile settings (CPI, sensitivity, polling rate, lighting).
    Settings {
        /// Profile number (1–5). Defaults to the active profile.
        #[arg(short, long)]
        profile: Option<u8>,
    },

    /// Show button mappings for a profile.
    Buttons {
        /// Profile number (1–5). Defaults to the active profile.
        #[arg(short, long)]
        profile: Option<u8>,

        /// Also show the EasyShift layer mappings.
        #[arg(short, long)]
        easyshift: bool,
    },

    /// Show firmware version.
    Firmware,

    /// Enable or disable driver-managed mode.
    Driver {
        #[command(subcommand)]
        action: DriverAction,
    },

    /// Reset the device to factory defaults.
    Reset {
        /// Skip the confirmation prompt.
        #[arg(short = 'y', long)]
        yes: bool,
    },

    /// List detected Nyth devices.
    Detect,
}

#[derive(Subcommand)]
enum ProfileAction {
    /// Show the currently active profile number.
    Get,
    /// Switch to a different profile.
    Set {
        /// Profile number (1–5).
        number: u8,
    },
}

#[derive(Subcommand)]
enum DriverAction {
    /// Enable driver mode (driver handles special events).
    On,
    /// Disable driver mode (device sends HID keycodes directly).
    Off,
}

fn main() {
    let cli = Cli::parse();

    if cli.verbose {
        std::env::set_var("RUST_LOG", "trace");
    } else if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "error");
    }
    env_logger::init();

    let result = match cli.command {
        None | Some(Command::Status) => cmd_status(),
        Some(Command::Profile { action }) => cmd_profile(action),
        Some(Command::Settings { profile }) => cmd_settings(profile),
        Some(Command::Buttons { profile, easyshift }) => cmd_buttons(profile, easyshift),
        Some(Command::Firmware) => cmd_firmware(),
        Some(Command::Driver { action }) => cmd_driver(action),
        Some(Command::Reset { yes }) => cmd_reset(yes),
        Some(Command::Detect) => cmd_detect(),
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        process::exit(1);
    }
}

// ─── Helpers ────────────────────────────────────────────────────────

fn open_device() -> Result<NythDevice, roccat_nyth_profile::Error> {
    NythDevice::open_first()
}

/// Resolve a user-facing 1–5 profile number to a 0–4 index.
/// If `user_num` is None, read the active profile from the device.
fn resolve_profile(
    nyth: &NythDevice,
    user_num: Option<u8>,
) -> Result<u8, roccat_nyth_profile::Error> {
    match user_num {
        Some(n) => {
            if !(1..=5).contains(&n) {
                eprintln!("Profile number must be 1–5 (got {n}).");
                process::exit(1);
            }
            Ok(n - 1)
        }
        None => nyth.read_active_profile(),
    }
}

fn format_light(light: &Light) -> String {
    format!("#{:02x}{:02x}{:02x}", light.red, light.green, light.blue)
}

/// Notify the nythd daemon about a profile change via D-Bus.
/// Silently does nothing if the daemon isn't running or D-Bus is unavailable.
fn notify_daemon_profile_changed(profile_number: u8) {
    let conn = match dbus::blocking::Connection::new_session() {
        Ok(c) => c,
        Err(_) => return, // No session bus — daemon not reachable.
    };

    let proxy = conn.with_proxy(
        "org.roccat.Nyth",
        "/org/roccat/Nyth",
        std::time::Duration::from_millis(500),
    );

    // Call ProfileChangedOutside(number) — fire-and-forget.
    let _: Result<(), _> = proxy.method_call(
        "org.roccat.Nyth",
        "ProfileChangedOutside",
        (profile_number,),
    );
}

// ─── Commands ───────────────────────────────────────────────────────

fn cmd_detect() -> Result<(), roccat_nyth_profile::Error> {
    let devices = roccat_nyth_hid::device::scan_devices();
    if devices.is_empty() {
        println!("No Roccat Nyth devices found.");
        println!();
        println!("Troubleshooting:");
        println!("  1. Is the mouse plugged in?");
        println!("  2. Are the udev rules installed?");
        println!("     sudo cp udev/90-roccat-nyth.rules /etc/udev/rules.d/");
        println!("     sudo udevadm control --reload-rules && sudo udevadm trigger");
        println!("  3. Is your user in the 'roccat' group?");
        println!("     sudo usermod -aG roccat $USER");
        println!("     (log out and back in after adding)");
    } else {
        for d in &devices {
            let iface = match d.interface_number {
                Some(0) => " [mouse]",
                Some(1) => " [keyboard]",
                Some(_) => "",
                None => " [interface unknown]",
            };
            println!(
                "  {} at {} (USB {:04x}:{:04x}){}",
                d.name,
                d.hidraw_path.display(),
                USB_VENDOR_ROCCAT,
                d.product_id,
                iface,
            );
        }
    }
    Ok(())
}

fn cmd_status() -> Result<(), roccat_nyth_profile::Error> {
    let nyth = open_device()?;
    let info = nyth.info();

    let firmware = nyth.read_firmware_version()?;
    let active = nyth.read_active_profile()?;

    println!("{}", info.name);
    println!("  Device:      {}", info.hidraw_path.display());
    println!("  Firmware:    {firmware}");
    println!("  Profile:     {} / {}", active + 1, NYTH_PROFILE_NUM);
    println!();

    // Show settings for the active profile.
    print_profile_settings(&nyth, active)?;

    Ok(())
}

fn cmd_profile(action: Option<ProfileAction>) -> Result<(), roccat_nyth_profile::Error> {
    let nyth = open_device()?;

    match action {
        None | Some(ProfileAction::Get) => {
            let active = nyth.read_active_profile()?;
            println!("{}", active + 1);
        }
        Some(ProfileAction::Set { number }) => {
            if !(1..=5).contains(&number) {
                eprintln!("Profile number must be 1–5 (got {number}).");
                process::exit(1);
            }
            let index = number - 1;
            nyth.write_active_profile(index)?;
            println!("Switched to profile {number}.");

            // Notify the daemon (if running) so it updates its state.
            notify_daemon_profile_changed(number);
        }
    }

    Ok(())
}

fn cmd_settings(profile: Option<u8>) -> Result<(), roccat_nyth_profile::Error> {
    let nyth = open_device()?;
    let index = resolve_profile(&nyth, profile)?;
    print_profile_settings(&nyth, index)?;
    Ok(())
}

fn print_profile_settings(
    nyth: &NythDevice,
    profile_index: u8,
) -> Result<(), roccat_nyth_profile::Error> {
    let settings = nyth.read_profile_settings(profile_index)?;

    println!("Profile {} settings:", profile_index + 1);

    // Polling rate
    let hz = display::polling_rate_hz(settings.polling_rate);
    println!("  Polling rate: {} Hz", hz);

    // CPI levels
    print!("  CPI levels:   ");
    for i in 0..NYTH_CPI_LEVELS_NUM {
        let cpi = settings.cpi_level(i);
        let enabled = settings.cpi_levels_enabled & (1 << i) != 0;
        let active = settings.cpi_active as usize == i;

        if i > 0 {
            print!(" / ");
        }

        if !enabled {
            print!("({cpi})");
        } else if active {
            print!("{cpi}*");
        } else {
            print!("{cpi}");
        }
    }
    println!();

    // Sensitivity
    println!(
        "  Sensitivity:  X={} Y={}{}",
        settings.sensitivity_x,
        settings.sensitivity_y,
        if settings.advanced_sensitivity != 0 {
            " (independent)"
        } else {
            ""
        }
    );

    // Lighting
    let lights_on = (settings.lights_enabled & 0x03) == 3;
    if !lights_on {
        println!("  Lighting:     Off");
    } else {
        println!(
            "  Light effect: {} (speed {})",
            display::light_effect_name(settings.light_effect),
            settings.effect_speed
        );
        println!(
            "  Colour effect: {}",
            display::color_effect_name(settings.color_effect)
        );
        for (i, light) in settings.lights.iter().enumerate() {
            let zone = match i {
                0 => "Logo",
                1 => "Bottom",
                _ => "Unknown",
            };
            let custom = settings.lights_enabled & (1 << (4 + i)) != 0;
            println!(
                "  {zone:>11}:  {}{}",
                format_light(light),
                if custom { "" } else { " (profile colour)" }
            );
        }
    }

    Ok(())
}

fn cmd_buttons(
    profile: Option<u8>,
    show_easyshift: bool,
) -> Result<(), roccat_nyth_profile::Error> {
    let nyth = open_device()?;
    let index = resolve_profile(&nyth, profile)?;
    let buttons = nyth.read_profile_buttons(index)?;

    println!("Profile {} button mappings:", index + 1);
    println!();

    // Normal layer (first 20 buttons)
    let limit = if show_easyshift {
        NYTH_PROFILE_BUTTON_NUM
    } else {
        NYTH_PHYSICAL_BUTTON_NUM
    };

    let mut printed_shift_header = false;

    for i in 0..limit {
        if i == NYTH_PHYSICAL_BUTTON_NUM && !printed_shift_header {
            println!();
            println!("EasyShift layer:");
            println!();
            printed_shift_header = true;
        }

        let btn = &buttons.buttons[i];
        let name = display::BUTTON_NAMES[i];
        let action = display::button_type_name(btn.button_type);

        let detail = match btn.button_type {
            0x05 => {
                // Shortcut: show modifier + key
                let mods = display::modifier_string(btn.modifier);
                format!("{action} ({mods}+0x{:02x})", btn.key)
            }
            _ if btn.modifier != 0 || btn.key != 0 => {
                format!("{action} [0x{:02x}, 0x{:02x}]", btn.modifier, btn.key)
            }
            _ => action.to_string(),
        };

        println!("  {name:<16} {detail}");
    }

    Ok(())
}

fn cmd_firmware() -> Result<(), roccat_nyth_profile::Error> {
    let nyth = open_device()?;
    let version = nyth.read_firmware_version()?;
    println!("{version}");
    Ok(())
}

fn cmd_driver(action: DriverAction) -> Result<(), roccat_nyth_profile::Error> {
    let nyth = open_device()?;

    match action {
        DriverAction::On => {
            nyth.enable_driver_mode()?;
            println!("Driver mode enabled.");
        }
        DriverAction::Off => {
            nyth.disable_driver_mode()?;
            println!("Driver mode disabled.");
        }
    }

    Ok(())
}

fn cmd_reset(skip_confirm: bool) -> Result<(), roccat_nyth_profile::Error> {
    if !skip_confirm {
        eprint!("This will reset the Nyth to factory defaults. Continue? [y/N] ");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).ok();
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled.");
            return Ok(());
        }
    }

    let nyth = open_device()?;
    nyth.reset_to_defaults()?;
    println!("Device reset to factory defaults.");

    Ok(())
}
