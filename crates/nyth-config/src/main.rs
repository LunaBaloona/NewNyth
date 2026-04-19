//! # NewNyth — GUI configuration tool for the Roccat Nyth mouse
//!
//! Part of the NewNyth project: https://github.com/LunaBaloona/NewNyth

mod app;
mod state;

use eframe::egui;

fn main() -> Result<(), eframe::Error> {
    // Handle --version / --help without initialising the GUI.
    let args: Vec<String> = std::env::args().collect();
    for arg in args.iter().skip(1) {
        match arg.as_str() {
            "--version" | "-V" => {
                println!("newnyth {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--help" | "-h" => {
                println!("NewNyth — Roccat Nyth Configuration GUI");
                println!();
                println!("Usage: newnyth [OPTIONS]");
                println!();
                println!("Options:");
                println!("  -V, --version    Print version information");
                println!("  -h, --help       Print this help message");
                println!();
                println!("Run without arguments to launch the GUI.");
                return Ok(());
            }
            _ => {}
        }
    }

    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "warn");
    }
    env_logger::init();

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("NewNyth — Roccat Nyth Configuration")
        .with_inner_size([960.0, 560.0])
        .with_min_inner_size([800.0, 480.0]);

    if let Some(icon) = load_icon() {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "newnyth",
        options,
        Box::new(|cc| Box::new(app::NythConfigApp::new(cc))),
    )
}

/// Try to load the application icon from standard locations.
fn load_icon() -> Option<egui::IconData> {
    let candidates = [
        "assets/newnyth.png",
        "/usr/share/icons/hicolor/128x128/apps/newnyth.png",
        "/usr/share/icons/hicolor/64x64/apps/newnyth.png",
        "/usr/share/icons/hicolor/48x48/apps/newnyth.png",
    ];
    for path in candidates {
        if let Ok(data) = std::fs::read(path) {
            if let Some(icon) = decode_png_icon(&data) {
                return Some(icon);
            }
        }
    }
    None
}

fn decode_png_icon(png_data: &[u8]) -> Option<egui::IconData> {
    let img = image::load_from_memory(png_data).ok()?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    Some(egui::IconData {
        rgba: rgba.into_raw(),
        width: w,
        height: h,
    })
}
