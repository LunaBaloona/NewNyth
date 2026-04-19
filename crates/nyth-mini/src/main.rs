//! # NewNyth Mini compact profile switcher added in v2.16mar24
//!
//! Just a tiny floating window for quick profile switching, until I can get auto profile-switch working.

use eframe::egui;
use roccat_nyth_hid::protocol::NYTH_PROFILE_NUM;
use roccat_nyth_profile::NythDevice;

fn main() -> Result<(), eframe::Error> {
    let args: Vec<String> = std::env::args().collect();
    for arg in args.iter().skip(1) {
        match arg.as_str() {
            "--version" | "-V" => {
                println!("newnyth-mini {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--help" | "-h" => {
                println!("NewNyth Mini — Compact profile switcher");
                println!();
                println!("Usage: newnyth-mini [OPTIONS]");
                println!();
                println!("Options:");
                println!("  -V, --version    Print version information");
                println!("  -h, --help       Print this help message");
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
        .with_title("NewNyth Mini")
        .with_inner_size([220.0, 110.0])
        .with_min_inner_size([200.0, 100.0])
        .with_resizable(false);

    if let Some(icon) = load_icon() {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "newnyth-mini",
        options,
        Box::new(|_cc| Box::new(MiniApp::new())),
    )
}

struct MiniApp {
    active_profile: u8,
    connected: bool,
    status: Option<String>,
}

impl MiniApp {
    fn new() -> Self {
        let (active, connected) = match NythDevice::open_first() {
            Ok(nyth) => {
                let profile = nyth.read_active_profile().unwrap_or(0);
                (profile, true)
            }
            Err(_) => (0, false),
        };

        Self {
            active_profile: active,
            connected,
            status: None,
        }
    }

    fn switch_profile(&mut self, index: u8) {
        match NythDevice::open_first() {
            Ok(nyth) => match nyth.write_active_profile(index) {
                Ok(()) => {
                    self.active_profile = index;
                    self.connected = true;
                    self.status = None;
                }
                Err(e) => {
                    self.status = Some(format!("Error: {e}"));
                }
            },
            Err(_) => {
                self.connected = false;
                self.status = Some("No device".into());
            }
        }
    }
}

impl eframe::App for MiniApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new("NewNyth Mini").strong().size(16.0));
                ui.add_space(8.0);

                if !self.connected {
                    ui.colored_label(egui::Color32::from_rgb(200, 80, 80), "No device");
                    if ui.small_button("Retry").clicked() {
                        *self = MiniApp::new();
                    }
                } else {
                    // Profile buttons.
                    ui.horizontal(|ui| {
                        ui.add_space((ui.available_width() - (5.0 * 34.0 + 4.0 * 4.0)) / 2.0);
                        for i in 0..NYTH_PROFILE_NUM {
                            let is_active = i == self.active_profile;
                            let btn = if is_active {
                                egui::Button::new(
                                    egui::RichText::new(format!("{}", i + 1)).strong(),
                                )
                                .fill(ui.visuals().selection.bg_fill)
                            } else {
                                egui::Button::new(format!("{}", i + 1))
                            };

                            let response = ui.add_sized([34.0, 28.0], btn);
                            if response.clicked() && !is_active {
                                self.switch_profile(i);
                            }
                            if is_active {
                                let rect = response.rect;
                                ui.painter().line_segment(
                                    [rect.left_bottom(), rect.right_bottom()],
                                    ui.visuals().widgets.active.fg_stroke,
                                );
                            }
                        }
                    });

                    if let Some(ref status) = self.status {
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new(status).small().weak());
                    }
                }

                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                        .small()
                        .weak(),
                );
            });
        });
    }
}

fn load_icon() -> Option<egui::IconData> {
    let candidates = [
        "assets/newnyth.png",
        "/usr/share/icons/hicolor/128x128/apps/newnyth.png",
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
