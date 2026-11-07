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
pub const EDGE: egui::Color32 = egui::Color32::from_rgb(0x3a, 0x3f, 0x44);
/// Inset backing behind the mouse illustrations on the Buttons tab: the
/// artwork is transparent-background vector art and sits in a canvas-dark
/// well (the old light "photo card" glare fought the dark theme).
pub const ILLU_BG: egui::Color32 = CANVAS;
const TEXT: egui::Color32 = egui::Color32::from_rgb(0xe8, 0xe8, 0xe8);
/// Normal-state interactive colour — button/combo text, check marks
/// and the resting scrollbar handle (egui paints the handle with
/// `widgets.*.fg_stroke` when the bar uses foreground colours). Full
/// TEXT brightness made the solid scrollbar glare, so interactive
/// chrome sits a notch dimmer; plain labels stay at TEXT.
const INTERACTIVE: egui::Color32 = egui::Color32::from_rgb(0xbf, 0xbf, 0xbf);
/// Section-card header strip: lighter than the card body so the title band
/// stands out (the Swarm boxes have tinted, centred headings).
const HEADER: egui::Color32 = egui::Color32::from_rgb(0x26, 0x2a, 0x2f);
const HEADER_HEIGHT: f32 = 26.0;
/// Width of the band pinned to the left edge of a card header when it
/// carries a widget (room for one small icon button plus a margin).
const HEADER_LEFT_W: f32 = 34.0;

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
    v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, INTERACTIVE);
    v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, EDGE);

    // Hover/active lean on the accent blue: the plain-grey tint egui
    // defaults to was nearly invisible on slider handles.
    v.widgets.hovered.bg_fill = CONTROL_HOVER;
    v.widgets.hovered.fg_stroke = egui::Stroke::new(1.5, ACCENT);
    v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, ACCENT);

    v.widgets.active.bg_fill = ACCENT_DIM;
    v.widgets.active.fg_stroke = egui::Stroke::new(2.0, ACCENT);
    v.widgets.active.bg_stroke = egui::Stroke::new(1.0, ACCENT);

    style.visuals = v;

    // Scrollbars: egui's default "floating" bars fade to an almost
    // invisible hairline at rest (they only appear properly on hover,
    // which made whole screens unreachable-looking at small window
    // sizes). A non-floating bar is laid out solidly and painted at
    // full opacity; the foreground colour gives the same crisp handle
    // the floating bar only showed under the mouse.
    style.spacing.scroll = egui::style::ScrollStyle {
        floating: false,
        foreground_color: true,
        bar_width: 10.5,
        handle_min_length: 40.0,
        ..Default::default()
    };

    ctx.set_style_of(egui::Theme::Dark, style);
}

// ─── Section cards ─────────────────────────────────────────────────

/// Framed card with a tinted header strip and centred title, Swarm-style.
///
/// ```ignore
/// theme::card(ui, "CPI levels", |ui| { /* body widgets */ });
/// ```
pub fn card<R>(
    ui: &mut egui::Ui,
    title: &str,
    add_body: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    card_inner(ui, title, title, None, None, None, add_body)
}

/// Full-width ACCENT-blue horizontal rule (keypad row separation in the
/// side-button grids).
pub fn accent_rule(ui: &mut egui::Ui) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_at_least(egui::vec2(width, 2.0), egui::Sense::hover());
    ui.painter().rect_filled(
        egui::Rect::from_center_size(rect.center(), egui::vec2(width, 1.5)),
        0.75,
        ACCENT,
    );
}

/// Like [`card`], but the caller draws the header content
/// itself instead of the default centred title (e.g. a title with an
/// inline rename field), plus widgets pinned to the left edge of the
/// header strip (e.g. a rename button) and extra widgets right-aligned
/// in it (e.g. a "Default" checkbox). The centred title and the caller-
/// drawn header only span the space between the two bands. `id` salts
/// the header's child [`egui::Ui`]s and must stay stable across frames
/// while header widgets hold keyboard focus - unlike a displayed title,
/// it must not embed volatile text.
pub fn card_with_header_and_actions<R>(
    ui: &mut egui::Ui,
    id: &str,
    header_left: &mut dyn FnMut(&mut egui::Ui),
    header: &mut dyn FnMut(&mut egui::Ui),
    header_actions: &mut dyn FnMut(&mut egui::Ui),
    add_body: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    card_inner(
        ui,
        id,
        "",
        Some(header_left),
        Some(header),
        Some(header_actions),
        add_body,
    )
}

fn card_inner<R>(
    ui: &mut egui::Ui,
    id: &str,
    title: &str,
    mut header_left: Option<&mut dyn FnMut(&mut egui::Ui)>,
    mut header: Option<&mut dyn FnMut(&mut egui::Ui)>,
    mut header_actions: Option<&mut dyn FnMut(&mut egui::Ui)>,
    add_body: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    egui::Frame::new()
        .fill(SURFACE)
        .stroke(egui::Stroke::new(1.0, EDGE))
        .inner_margin(0.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());

            // ── Header strip (lighter band, centred title) ──
            let strip = egui::Rect::from_min_size(
                ui.cursor().min,
                egui::vec2(ui.available_width(), HEADER_HEIGHT),
            );
            ui.allocate_space(strip.size());
            ui.painter().rect_filled(strip, 0.0, HEADER);

            // Reserve bands on both ends when the header carries widgets.
            let left_w = if header_left.is_some() {
                HEADER_LEFT_W
            } else {
                0.0
            };
            let actions_w = if header_actions.is_some() { 150.0 } else { 0.0 };
            let title_rect = egui::Rect::from_min_max(
                egui::pos2(strip.left() + left_w, strip.top()),
                egui::pos2(strip.right() - actions_w, strip.bottom()),
            );
            if let Some(left) = header_left.as_mut() {
                let left_rect = egui::Rect::from_min_max(
                    strip.min + egui::vec2(4.0, 0.0),
                    egui::pos2(strip.left() + left_w, strip.bottom()),
                );
                let mut left_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt(("card_header_left", id))
                        .max_rect(left_rect)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                );
                left(&mut left_ui);
            }
            let mut title_ui = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt(("card_header", id))
                    .max_rect(title_rect)
                    .layout(egui::Layout::centered_and_justified(
                        egui::Direction::LeftToRight,
                    )),
            );
            match header.as_mut() {
                Some(draw_header) => draw_header(&mut title_ui),
                None => {
                    title_ui.add(
                        egui::Label::new(egui::RichText::new(title).strong())
                            .halign(egui::Align::Center),
                    );
                }
            }
            if let Some(actions) = header_actions.as_mut() {
                let acts_rect = egui::Rect::from_min_max(
                    egui::pos2(strip.right() - actions_w, strip.top()),
                    strip.max,
                );
                let mut acts_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt(("card_actions", id))
                        .max_rect(acts_rect)
                        .layout(egui::Layout::right_to_left(egui::Align::Center)),
                );
                actions(&mut acts_ui);
            }

            // ── Body ──
            egui::Frame::new()
                .inner_margin(8.0)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    add_body(ui)
                })
                .inner
        })
}

/// Horizontal row with a fixed width, centred inside its parent.
///
/// Pair with [`egui::Ui::vertical_centered`]: unlike [`egui::Ui::horizontal`]
/// (which always fills the available width and therefore sits flush left
/// even in centring layouts), the finite width lets that layout actually
/// centre the row. Pick a width close to the row's natural size; widgets
/// that stretch (sliders) will fill whatever is left.
pub fn centered_row<R>(
    ui: &mut egui::Ui,
    width: f32,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let height = ui.spacing().interact_size.y;
    centered_row_sized(ui, width, height, add_contents)
}

/// Like [`centered_row`], with an explicit height — needed for rows
/// containing taller widgets (e.g. segmented sliders with tick labels).
pub fn centered_row_sized<R>(
    ui: &mut egui::Ui,
    width: f32,
    height: f32,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let width = width.min(ui.available_width());
    ui.allocate_ui_with_layout(
        egui::vec2(width, height),
        egui::Layout::left_to_right(egui::Align::Center),
        add_contents,
    )
}

/// A centred pair of buttons, e.g. the answers in the calibration
/// dialogs. [`centered_row`] centres its fixed-width strip but packs the
/// buttons against the strip's left edge, so a narrower group visibly
/// drifts left; here each button instead gets one half of the row - the
/// first laid out right-to-left into the left half, the second
/// left-to-right into the right half - so the pair always hugs the
/// centre, at natural button widths and any font scale.
///
/// Returns `(first_clicked, second_clicked)`.
pub fn centered_pair(ui: &mut egui::Ui, first: &str, second: &str) -> (bool, bool) {
    let gap = ui.spacing().item_spacing.x;
    let height = ui.spacing().interact_size.y;
    let strip =
        egui::Rect::from_min_size(ui.cursor().min, egui::vec2(ui.available_width(), height));
    let mid = strip.center().x;
    let first_clicked = {
        let mut left = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("centered_pair", first))
                .max_rect(egui::Rect::from_min_max(
                    strip.min,
                    egui::pos2(mid - gap / 2.0, strip.bottom()),
                ))
                .layout(egui::Layout::right_to_left(egui::Align::Center)),
        );
        left.button(first).clicked()
    };
    let second_clicked = {
        let mut right = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("centered_pair", second))
                .max_rect(egui::Rect::from_min_max(
                    egui::pos2(mid + gap / 2.0, strip.min.y),
                    strip.max,
                ))
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        right.button(second).clicked()
    };
    ui.allocate_space(strip.size());
    (first_clicked, second_clicked)
}
