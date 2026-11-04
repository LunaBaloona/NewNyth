//! Swarm-inspired visual theme: charcoal surfaces, cyan accents, Play font.
//!
//! Colors are sampled roughly from the original Roccat Swarm screenshots
//! (see `images/` in the workspace); the "family" is the point, not exact
//! reproduction. Gradients/photoreal shading are deliberately out of scope —
//! egui does flat colors well and that reads as "Swarm-like" already.
//!
//! The Ubuntu font (Ubuntu Font Licence 1.0, bundled in `assets/fonts/`,
//! license alongside) gives the UI a softer, rounder read than egui's
//! default; it is also the family the original Swarm era loved on Linux.

use eframe::egui;

const UBUNTU_REGULAR: &[u8] = include_bytes!("../../../assets/fonts/Ubuntu-Regular.ttf");
const UBUNTU_BOLD: &[u8] = include_bytes!("../../../assets/fonts/Ubuntu-Bold.ttf");

/// Swarm's cyan highlight, used for selections, accents and focus strokes.
pub const ACCENT: egui::Color32 = egui::Color32::from_rgb(0x4d, 0xb8, 0xe8);
const ACCENT_DIM: egui::Color32 = egui::Color32::from_rgb(0x2e, 0x6f, 0x8e);

/// Panel / card surface (charcoal).
const SURFACE: egui::Color32 = egui::Color32::from_rgb(0x1b, 0x1d, 0x20);
/// Deepest background (canvas behind cards, text edit fields).
const CANVAS: egui::Color32 = egui::Color32::from_rgb(0x12, 0x13, 0x14);
/// Control backgrounds (buttons, combo boxes).
const CONTROL: egui::Color32 = egui::Color32::from_rgb(0x2a, 0x2e, 0x33);
const CONTROL_HOVER: egui::Color32 = egui::Color32::from_rgb(0x3a, 0x40, 0x46);
/// Hairline borders between surfaces.
const EDGE: egui::Color32 = egui::Color32::from_rgb(0x3a, 0x3f, 0x44);
const TEXT: egui::Color32 = egui::Color32::from_rgb(0xe8, 0xe8, 0xe8);

/// Apply fonts and colors to the given context. Call once at startup,
/// before any widgets are drawn.
pub fn apply(ctx: &egui::Context) {
    load_fonts(ctx);
    apply_colors(ctx);
}

fn load_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "ubuntu".to_owned(),
        egui::FontData::from_static(UBUNTU_REGULAR).into(),
    );
    fonts.font_data.insert(
        "ubuntu_bold".to_owned(),
        egui::FontData::from_static(UBUNTU_BOLD).into(),
    );
    let proportional = fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default();
    proportional.insert(0, "ubuntu".to_owned());
    proportional.insert(1, "ubuntu_bold".to_owned());
    // Monospace (paths, key names) stays on egui's built-in Hack.
    ctx.set_fonts(fonts);
}

fn apply_colors(ctx: &egui::Context) {
    // Swarm is a dark application; don't follow the system preference.
    ctx.set_theme(egui::ThemePreference::Dark);

    // Keep egui's default spacing/sizes; only restyle colors.
    let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
    let mut v = egui::Visuals::dark();

    v.window_fill = SURFACE;
    v.panel_fill = SURFACE;
    v.extreme_bg_color = CANVAS;
    v.faint_bg_color = CONTROL_HOVER;
    v.hyperlink_color = ACCENT;

    v.selection.bg_fill = ACCENT.linear_multiply(0.30);
    v.selection.stroke = egui::Stroke::new(1.0, ACCENT);

    v.widgets.noninteractive.bg_fill = SURFACE;
    v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, TEXT);
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, EDGE);

    v.widgets.inactive.bg_fill = CONTROL;
    v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, TEXT);
    v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, EDGE);

    v.widgets.hovered.bg_fill = CONTROL_HOVER;
    v.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
    v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, ACCENT_DIM);

    v.widgets.active.bg_fill = ACCENT_DIM;
    v.widgets.active.fg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
    v.widgets.active.bg_stroke = egui::Stroke::new(1.0, ACCENT);

    style.visuals = v;
    ctx.set_style_of(egui::Theme::Dark, style);
}
