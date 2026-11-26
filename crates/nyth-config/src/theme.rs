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
//!
//! The positioning and alignment rules this module encodes (centring,
//! truncation, header band geometry, and the egui 0.37 quirks behind
//! them) are recorded in `docs/GUI-LAYOUT.md`. Read it before adding
//! or changing any layout helper here.

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

/// Two-line variant of the header strip (the Auto-Switch profile cards
/// stack "Mouse Profile N" over the custom name).
const HEADER_HEIGHT_TALL: f32 = 46.0;
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

/// Body inset of [`card`] (the body [`egui::Frame`]'s inner margin).
/// Row bands extend by this much on both sides to reach the card edges.
pub const CARD_BODY_MARGIN: f32 = 8.0;

/// Alternating-row band spanning the full card width, flush with the
/// card edges (egui's own `Grid::striped` only paints behind the grid's
/// columns, so the band stops short of the frame). Capture once per
/// card from the card body ui, then `paint` each banded row *before*
/// adding the row's widgets so the content paints on top; the extent
/// and painter must be captured outside layouts like `egui::Grid`,
/// whose per-row available rect is only as wide as the current column.
/// Same colour egui's striped rows use (`faint_bg_color`), keeping
/// hand-banded rows consistent.
pub struct CardRowBand {
    left: f32,
    right: f32,
    painter: egui::Painter,
    fill: egui::Color32,
}

impl CardRowBand {
    /// Capture from a [`card`] body ui (its full content width plus the
    /// body margin, reaching the card edges).
    pub fn for_card(ui: &egui::Ui) -> Self {
        let inner = ui.available_rect_before_wrap();
        Self {
            left: inner.left() - CARD_BODY_MARGIN,
            right: inner.right() + CARD_BODY_MARGIN,
            painter: ui.painter().clone(),
            fill: ui.visuals().faint_bg_color,
        }
    }

    /// Paint one band from `top` (the row's cursor y) down `height`.
    pub fn paint(&self, top: f32, height: f32) {
        let rect = egui::Rect::from_min_max(
            egui::pos2(self.left, top),
            egui::pos2(self.right, top + height),
        );
        self.painter.rect_filled(rect, 0.0, self.fill);
    }

    /// Paint a thin full-width rule (rounded ends), inset slightly from
    /// the card edges: closes off the add-application sub-header inside
    /// the Auto-Switch cards.
    pub fn paint_bar(&self, top: f32, height: f32, fill: egui::Color32) {
        let rect = egui::Rect::from_min_max(
            egui::pos2(self.left + 4.0, top),
            egui::pos2(self.right - 4.0, top + height),
        );
        self.painter.rect_filled(rect, height / 2.0, fill);
    }
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
            // Auto-Switch profile cards carry a two-line title (profile
            // number over custom name); every other header is one line.
            let tall = header.is_some();
            let strip = egui::Rect::from_min_size(
                ui.cursor().min,
                egui::vec2(
                    ui.available_width(),
                    if tall {
                        HEADER_HEIGHT_TALL
                    } else {
                        HEADER_HEIGHT
                    },
                ),
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
            // The title gets all the space between the pencil and the
            // action bands; its text centres inside that area.
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
                    // Centred both ways inside the header band. A tall
                    // band stacks the title's two lines top-down (the
                    // header closure adds leading space so the block sits
                    // on the bar's middle line); the single-line band
                    // justifies horizontally as before.
                    .layout(if tall {
                        egui::Layout::top_down(egui::Align::Center)
                    } else {
                        egui::Layout::centered_and_justified(egui::Direction::LeftToRight)
                    }),
            );
            match header.as_mut() {
                Some(draw_header) => draw_header(&mut title_ui),
                None => {
                    title_ui.add(
                        egui::Label::new(egui::RichText::new(title).strong())
                            .halign(egui::Align::Center)
                            // One line, ever: a title that cannot fit the
                            // band is ellipsised rather than wrapped onto
                            // two lines (which would spill out of the
                            // fixed-height header bar).
                            .truncate(),
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
                .inner_margin(CARD_BODY_MARGIN)
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
    centered_pair_enabled(ui, true, first, true, second)
}

/// Same half-strip centring as [`centered_pair`], but either button can
/// be greyed until there is something to do - used by the Apply/Save and
/// Revert bars pinned to the foot of every tab.
///
/// Returns `(first_clicked, second_clicked)`.
pub fn centered_pair_enabled(
    ui: &mut egui::Ui,
    first_enabled: bool,
    first: &str,
    second_enabled: bool,
    second: &str,
) -> (bool, bool) {
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
        left.add_enabled(first_enabled, egui::Button::new(first))
            .clicked()
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
        right
            .add_enabled(second_enabled, egui::Button::new(second))
            .clicked()
    };
    ui.allocate_space(strip.size());
    (first_clicked, second_clicked)
}

/// A horizontal row centred on its parent's centre line at its natural
/// width, whatever that turns out to be.
///
/// [`egui::Ui::horizontal`] always fills its parent, so inside a
/// centring layout ([`egui::Ui::vertical_centered`], a card body) its
/// content packs against the left edge; [`centered_row_sized`] can
/// centre, but must be handed a strip width that fits the content - a
/// strip that is narrower or wider shifts the group off the centre line.
/// This measures the row instead: the closure runs once in a throwaway
/// child parked far outside the clip rect (no pointer can reach it, so
/// nothing is hovered, clicked or focused and `clicked()`/`changed()`
/// stay false there), then runs again for real inside a strip exactly as
/// wide as the measurement, which the centring parent parks on the
/// centre line at any font scale.
///
/// The closure must therefore be a plain row of widgets that only
/// mutate through user interaction; it runs twice per frame.
pub fn centered_row_fit<R>(
    ui: &mut egui::Ui,
    add_contents: impl FnMut(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let height = ui.spacing().interact_size.y;
    centered_row_fit_sized(ui, height, add_contents)
}

/// Like [`centered_row_fit`], with an explicit height - needed for rows
/// containing taller widgets (e.g. segmented sliders with tick labels).
pub fn centered_row_fit_sized<R>(
    ui: &mut egui::Ui,
    height: f32,
    mut add_contents: impl FnMut(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    // Measuring pass: the same widgets far off-screen, where the parent
    // clip rect hides them and no pointer can land. The y coordinate
    // keeps the child ids of sibling rows distinct from each other.
    let probe = egui::Rect::from_min_size(
        egui::pos2(ui.max_rect().max.x + 1.0e6, ui.cursor().min.y.round()),
        ui.available_size(),
    );
    let width = {
        let mut meas = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("centered_row_fit", probe.min.y as i32))
                .max_rect(probe)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        add_contents(&mut meas);
        meas.min_size().x
    };
    centered_row_sized(ui, width, height, add_contents)
}

/// Headless layout tests, one per rule proven during the GUI polish
/// rounds and recorded in `docs/GUI-LAYOUT.md`. They render the real
/// helpers (cards, centred bars, row bands) and a replica of the
/// Buttons tab's pinned bottom bar plus three-strip block through
/// `egui::Context::run_pass` at fixed viewports, then assert on the
/// painted shapes - no window, no device, runs in CI with the normal
/// `cargo test --workspace`.
///
/// The replica of `NythConfigApp::ui_buttons()` mirrors that function's
/// width math on purpose: if the math in `app.rs` changes, this
/// replica must move with it in the same PR. That is deliberate: the
/// suite then keeps failing loudly if the strips ever overflow again.
/// (The original throwaway probe this grew from was named "barspy",
/// written for the v0.9.2 scrollbar-sliver bug.)
#[cfg(test)]
mod layout_tests {
    use super::*;

    /// Viewport height for every scene; tall enough to hold any pane.
    const VIEWPORT_H: f32 = 800.0;
    /// Position tolerance (points) for centring assertions.
    const TOL: f32 = 1.5;

    /// Render `pass` for a few frames with the real theme applied at
    /// the given viewport width and text-size multiplier, then return
    /// the last frame's shapes. Text scaling stands in for the user's
    /// fractional display scaling: the centring math must hold at any
    /// label size, so label widths grow with the multiplier.
    fn scene(width: f32, text_scale: f32, pass: impl Fn(&mut egui::Ui)) -> egui::FullOutput {
        let ctx = egui::Context::default();
        apply(&ctx);
        if (text_scale - 1.0).abs() > f32::EPSILON {
            ctx.style_mut_of(egui::Theme::Dark, |style| {
                for font in style.text_styles.values_mut() {
                    font.size *= text_scale;
                }
            });
        }
        let mut out = egui::FullOutput::default();
        for frame in 0..12 {
            let raw = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width, VIEWPORT_H),
                )),
                time: Some(f64::from(frame) / 60.0),
                ..Default::default()
            };
            out = ctx.run_pass(raw, |ctx| {
                ctx.root_ui(|root| {
                    egui::CentralPanel::default().show(root, |ui| pass(ui));
                });
            });
        }
        out
    }

    /// Rects painted with the exact given fill colour. The theme's
    /// colours discriminate the widgets apart: HEADER marks header
    /// strips, CONTROL marks button frames, CONTROL_HOVER marks row
    /// bands, INTERACTIVE marks resting scrollbar handles.
    fn filled(out: &egui::FullOutput, color: egui::Color32) -> Vec<egui::Rect> {
        out.shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::epaint::Shape::Rect(rect) if rect.fill == color => Some(rect.rect),
                _ => None,
            })
            .collect()
    }

    /// Button frames paint with egui's default `weak_bg_fill` (the
    /// theme overrides `bg_fill` but not the weak variant, so that is
    /// the colour buttons actually land on; recorded here so the
    /// filter is honest about what it matches).
    fn button_fill() -> egui::Color32 {
        egui::Visuals::dark().widgets.inactive.weak_bg_fill
    }

    /// Every painted text with its bounding rect.
    fn texts(out: &egui::FullOutput) -> Vec<(String, egui::Rect)> {
        out.shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::epaint::Shape::Text(shape) => Some((
                    shape.galley.text().to_owned(),
                    egui::Rect::from_min_size(shape.pos, shape.galley.size()),
                )),
                _ => None,
            })
            .collect()
    }

    /// Rects of all painted text starting with `needle`.
    fn text_rects(out: &egui::FullOutput, needle: &str) -> Vec<egui::Rect> {
        texts(out)
            .into_iter()
            .filter(|(text, _)| text.starts_with(needle))
            .map(|(_, rect)| rect)
            .collect()
    }

    /// Number of laid-out lines in the first painted text starting
    /// with `needle`.
    fn row_count(out: &egui::FullOutput, needle: &str) -> usize {
        out.shapes
            .iter()
            .find_map(|clipped| match &clipped.shape {
                egui::epaint::Shape::Text(shape) if shape.galley.text().starts_with(needle) => {
                    Some(shape.galley.rows.len())
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("no painted text starts with {needle:?}"))
    }

    /// The outer frame rect of a [`card`] (surface fill, edge stroke).
    fn card_frame(out: &egui::FullOutput) -> egui::Rect {
        out.shapes
            .iter()
            .find_map(|clipped| match &clipped.shape {
                egui::epaint::Shape::Rect(rect)
                    if rect.fill == SURFACE && rect.stroke.color == EDGE =>
                {
                    Some(rect.rect)
                }
                _ => None,
            })
            .expect("a card frame was painted")
    }

    /// Button frames inside the vertical range `band`, sorted left to
    /// right.
    fn buttons_in(out: &egui::FullOutput, band: std::ops::Range<f32>) -> Vec<egui::Rect> {
        let mut found = filled(out, button_fill());
        found.retain(|rect| {
            rect.width() > 12.0
                && rect.height() > 6.0
                && rect.min.y >= band.start
                && rect.max.y <= band.end
        });
        found.sort_by(|a, b| a.min.x.total_cmp(&b.min.x));
        found
    }

    // Rules 2 + 3: the quirk that motivates every helper in this file.
    #[test]
    fn horizontal_rows_pack_left_while_direct_items_centre() {
        let width = 600.0;
        let rect_cell = std::cell::Cell::new(egui::Rect::NOTHING);
        let out = scene(width, 1.0, |ui| {
            ui.vertical_centered(|ui| {
                rect_cell.set(ui.label("direct item").rect);
                ui.horizontal(|ui| {
                    ui.label("row item");
                    let _ = ui.button("B");
                });
            });
        });
        let centre = width / 2.0;

        let direct = rect_cell.get();
        assert!(
            (direct.center().x - centre).abs() <= TOL,
            "rule 3: a direct item of vertical_centered must centre, got {:?}",
            direct
        );

        let row = text_rects(&out, "row item")
            .pop()
            .expect("row label painted");
        assert!(
            row.center().x < centre - 50.0,
            "rule 2: a horizontal() child fills the width and packs its \
             content left, so the label must sit far off centre, got {}",
            row.center().x
        );
    }

    // Rule 4: the half-strip pair hugs the centre line whatever the
    // labels' widths are (the Apply/Revert bars).
    #[test]
    fn centered_pair_hugs_the_centre_line_at_any_text_scale() {
        let width = 640.0;
        for scale in [1.0, 1.5, 2.0] {
            let gap = std::cell::Cell::new(0.0f32);
            let out = scene(width, scale, |ui| {
                gap.set(ui.spacing().item_spacing.x);
                centered_pair(ui, "A", "Apply button mappings");
            });
            let centre = width / 2.0;

            let mut pair = filled(&out, button_fill())
                .into_iter()
                .filter(|rect| rect.width() > 12.0 && rect.height() > 6.0)
                .collect::<Vec<_>>();
            pair.sort_by(|a, b| a.min.x.total_cmp(&b.min.x));
            assert_eq!(pair.len(), 2, "two button frames at scale {scale}");
            let (left, right) = (pair[0], pair[1]);

            assert!(
                left.max.x <= centre && right.min.x >= centre,
                "rule 4: each button gets one half-strip, scale {scale}: \
                 left={left:?} right={right:?} centre={centre}"
            );
            let seen_gap = right.min.x - left.max.x;
            assert!(
                (seen_gap - gap.get()).abs() <= TOL,
                "rule 4: the buttons keep one item-spacing gap at scale \
                 {scale}, got {seen_gap} expected {}",
                gap.get()
            );
            let pair_mid = (left.max.x + right.min.x) / 2.0;
            assert!(
                (pair_mid - centre).abs() <= TOL,
                "rule 4: the pair must hug the centre line at scale \
                 {scale}, got {pair_mid} expected {centre}"
            );
        }
    }

    // Rule 5: centered_row_fit measures in an off-screen pass, then
    // paints one exactly-sized centred copy. The measuring copy parks
    // itself a million points to the right; whatever egui does with
    // its shapes there, none of it may ever appear inside the
    // viewport, and the visible row must sit centred.
    #[test]
    fn centered_row_fit_probes_offscreen_then_centres_the_row() {
        for scale in [1.0, 1.5, 2.0] {
            for width in [700.0, 460.0] {
                let gap = std::cell::Cell::new(0.0f32);
                let out = scene(width, scale, |ui| {
                    gap.set(ui.spacing().item_spacing.x);
                    ui.vertical_centered(|ui| {
                        centered_row_fit(ui, |ui| {
                            ui.label("Sensitivity");
                            let _ = ui.button("Minus");
                            let _ = ui.button("Plus");
                        });
                    });
                });

                let label = text_rects(&out, "Sensitivity");
                assert_eq!(
                    label.len(),
                    1,
                    "rule 5: exactly one visible copy of the row at \
                     scale {scale} width {width} (the measuring pass \
                     must stay out of the viewport)"
                );
                let real = label[0];

                let mut real_buttons: Vec<egui::Rect> = filled(&out, button_fill())
                    .into_iter()
                    .filter(|rect| {
                        rect.width() > 12.0 && rect.height() > 6.0 && rect.max.x <= width + 0.5
                    })
                    .collect();
                real_buttons.sort_by(|a, b| a.min.x.total_cmp(&b.min.x));
                assert_eq!(real_buttons.len(), 2, "the visible row has both buttons");
                let seen_gap = real_buttons[1].min.x - real_buttons[0].max.x;
                assert!(
                    (seen_gap - gap.get()).abs() <= TOL,
                    "rule 5: painted row keeps its spacing at scale {scale}, got {seen_gap}"
                );

                let bbox = real.union(real_buttons[0]).union(real_buttons[1]);
                assert!(
                    (bbox.center().x - width / 2.0).abs() <= TOL + 1.0,
                    "rule 5: the row must sit centred at width {width} \
                     scale {scale}, got {} (bbox {bbox:?})",
                    bbox.center().x
                );
                assert!(
                    bbox.max.x <= width + 0.5 && bbox.min.x >= -0.5,
                    "rule 5: nothing of the visible row paints outside \
                     the viewport at width {width}: {bbox:?}"
                );
            }
        }
    }

    // Rule 8: WrapMode::Truncate forces a single line, so header
    // labels never spill out of fixed-height bars.
    #[test]
    fn truncate_pins_every_label_to_one_line() {
        let out = scene(600.0, 1.0, |ui| {
            ui.add(egui::Label::new("Alpha line\nBravo line").truncate());
            ui.add(egui::Label::new("Alpha line").truncate());
            ui.add(egui::Label::new("Bravo line").truncate());
        });
        assert_eq!(texts(&out).len(), 3, "exactly the three labels");
        assert_eq!(
            row_count(&out, "Alpha line\nBravo"),
            1,
            "rule 8: a multi-line string with truncate() collapses to \
             one ellipsised line (which is why two-line headers use two labels)"
        );
        assert_eq!(row_count(&out, "Bravo line"), 1);
    }

    // Rules 11 + 12: default card header - fixed-height strip, title
    // centred inside it, long titles ellipsised, never spilled.
    #[test]
    fn card_header_title_centres_inside_the_strip() {
        for scale in [1.0, 1.25] {
            let width = 640.0;
            let out = scene(width, scale, |ui| {
                card(ui, "Centre Of Attention", |ui| {
                    ui.label("body");
                });
            });

            let frame = card_frame(&out);
            let strips = filled(&out, HEADER);
            assert_eq!(strips.len(), 1, "one header strip at scale {scale}");
            let strip = strips[0];
            assert!(
                (strip.height() - HEADER_HEIGHT).abs() <= 0.01,
                "the header strip keeps its fixed height at scale {scale}"
            );
            assert!(
                (strip.width() - frame.width()).abs() <= 2.0,
                "rule 11: the strip spans the card (strip {strip:?} frame {frame:?})"
            );

            let title = text_rects(&out, "Centre Of Attention")
                .pop()
                .expect("title painted");
            assert!(
                title.min.x >= strip.min.x - 2.5 && title.max.x <= strip.max.x + 2.5,
                "rule 12: the title stays inside the strip ({title:?} in {strip:?})"
            );
            // (Horizontal centring inside the justified band itself is
            // not reliably measurable headlessly - galley origins in
            // justified layouts lie in this harness. The tall-header
            // test pins title-region centring via allocation rects,
            // and the anti-regression that matters here, never
            // spilling out of the bar, is asserted above.)
        }

        // A title far too long for the card is ellipsised onto one
        // line inside the bar, not wrapped out of it.
        let out = scene(400.0, 1.0, |ui| {
            card(ui, &"Long ".repeat(60), |ui| {
                ui.label("body");
            });
        });
        let strip = filled(&out, HEADER).pop().expect("header strip");
        let title = text_rects(&out, "Long").pop().expect("long title painted");
        assert_eq!(row_count(&out, "Long "), 1, "rule 8 at card level");
        assert!(
            title.width() <= strip.width() + 3.0,
            "the title is cut down to the band width - one ellipsised \
             line that cannot wrap out of the bar: galley {title:?} vs \
             strip {strip:?}"
        );
    }

    // Rules 11 + 13: the tall header's title region spans the space
    // between the two widget bands, and the padded two-line block
    // sits on the bar's middle line.
    #[test]
    fn tall_card_header_centres_the_two_line_block() {
        let first_cell = std::cell::Cell::new(egui::Rect::NOTHING);
        let second_cell = std::cell::Cell::new(egui::Rect::NOTHING);
        let out = scene(640.0, 1.0, |ui| {
            card_with_header_and_actions(
                ui,
                "mp0",
                &mut |ui| {
                    let _ = ui.small_button("rename");
                },
                &mut |ui| {
                    // The rule 9 padding, as the Auto-Switch profile
                    // cards do it in app.rs.
                    let font_id = ui
                        .style()
                        .text_styles
                        .get(&egui::TextStyle::Body)
                        .cloned()
                        .unwrap_or_else(|| egui::FontId::proportional(14.0));
                    let row_h = ui.fonts_mut(|f| f.row_height(&font_id));
                    let block_h = 2.0 * row_h + ui.spacing().item_spacing.y;
                    let pad = ((ui.max_rect().height() - block_h) * 0.5).max(0.0);
                    ui.add_space(pad);
                    first_cell.set(ui.add(egui::Label::new("Mouse Profile 1").truncate()).rect);
                    second_cell.set(ui.add(egui::Label::new("Workbench").truncate()).rect);
                },
                &mut |ui| {
                    let _ = ui.button("Default");
                },
                |ui| {
                    ui.label("body");
                },
            );
        });

        let strips = filled(&out, HEADER);
        assert_eq!(strips.len(), 1, "one tall header strip");
        let strip = strips[0];
        assert!(
            (strip.height() - HEADER_HEIGHT_TALL).abs() <= 0.01,
            "a header closure switches the strip to the tall variant"
        );

        // Title region = between the left band and the actions band.
        // The 150 pt actions band mirrors card_inner.
        let region = egui::Rect::from_min_max(
            egui::pos2(strip.min.x + HEADER_LEFT_W, strip.min.y),
            egui::pos2(strip.max.x - 150.0, strip.max.y),
        );
        let (first, second) = (first_cell.get(), second_cell.get());
        assert!(
            first.is_finite() && second.is_finite(),
            "both lines laid out"
        );
        for line in [first, second] {
            assert!(
                line.min.x >= region.min.x - TOL && line.max.x <= region.max.x + TOL,
                "rule 11: title lines stay between the pencil and the \
                 action bands, got {line:?} in {region:?}"
            );
            assert!(
                (line.center().x - region.center().x).abs() <= TOL,
                "rule 13: the lines centre inside the title region, got {line:?}"
            );
            assert!(
                line.min.y >= strip.min.y - TOL && line.max.y <= strip.max.y + TOL,
                "rule 9: padded lines stay inside the tall bar, got {line:?} in {strip:?}"
            );
        }
        // row_height (the padding metric) runs a hair taller than the
        // laid-out lines, so the block sits slightly above the exact
        // middle - that is the approved look; 4 pt bounds it.
        let block_mid = (first.min.y + second.max.y) / 2.0;
        assert!(
            (block_mid - strip.center().y).abs() <= 4.0,
            "rule 13: the two-line block sits on the bar's middle line, \
             got {block_mid} vs {}",
            strip.center().y
        );
    }

    // Rule 14: row bands captured from the card body reach the card
    // edges, unlike the grid's per-column clip.
    #[test]
    fn card_row_band_reaches_the_card_edges() {
        let out = scene(640.0, 1.0, |ui| {
            card(ui, "Banded rows", |ui| {
                let band = CardRowBand::for_card(ui);
                band.paint(ui.cursor().min.y, 12.0);
                ui.label("row 0");
                band.paint(ui.cursor().min.y, 12.0);
                ui.label("row 1");
            });
        });

        let frame = card_frame(&out);
        let bands = filled(&out, CONTROL_HOVER);
        assert_eq!(bands.len(), 2, "both banded rows painted");
        for band in bands {
            assert!(
                band.width() >= frame.width() - 5.0,
                "rule 14: the band spans the whole card (band {band:?} \
                 vs frame {frame:?})"
            );
            assert!(
                (band.center().x - frame.center().x).abs() <= TOL,
                "rule 14: the band is flush with the card frame"
            );
        }
    }

    // Rule 15 (and the v0.9.2 regression): the promoted barspy probe.
    // The Buttons tab's pinned bottom bar and the exact three-strip
    // width math of `NythConfigApp::ui_buttons()` (app.rs), checked at
    // every width the polish rounds were approved at. If the math in
    // app.rs changes, update the replica here in the same commit.
    #[test]
    fn buttons_pane_keeps_the_scrollbar_and_the_strip_inside() {
        // 880 pt is where the illustration column hits its 220 pt
        // clamp floor; below that the block is legitimately wider than
        // the viewport and a horizontal bar is expected, so widths
        // under 880 are out of scope (same as the original probe).
        for width in [1275.0, 1132.0, 1000.0, 900.0, 880.0] {
            let out = scene(width, 1.0, |ui| {
                // Pinned bottom strip: separator + detail row +
                // Apply/Revert pair (the structure of
                // `ui_buttons_actions` + `apply_revert_bar`, at the
                // panel's natural width - the app never forces one).
                egui::Panel::bottom("buttons_actions").show(ui, |ui| {
                    ui.separator();
                    ui.label("Editing: 3: Middle (type: Shortcut)");
                    centered_pair_enabled(ui, true, "Apply button mappings", true, "Revert");
                });
                egui::ScrollArea::vertical()
                    .id_salt("section_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for i in 0..6 {
                            ui.label(format!("profile row {i}"));
                        }
                        // ── Mirror of ui_buttons() math ──
                        let avail_w = ui.available_width();
                        let gap = ui.spacing().item_spacing.x.max(10.0);
                        let junction = gap + ui.spacing().item_spacing.x;
                        let combo_w = 292.0f32;
                        let img_w =
                            (avail_w - 2.0 * combo_w - 2.0 * junction - 2.0).clamp(220.0, 560.0);
                        let block_w = 2.0 * combo_w + img_w + 2.0 * junction;
                        let pad = ((avail_w - block_w) / 2.0).max(0.0);

                        ui.horizontal(|ui| {
                            ui.add_space(pad);
                            ui.vertical(|ui| {
                                ui.set_width(combo_w);
                                card(ui, "Standard | Main buttons", |ui| {
                                    for i in 0..40 {
                                        ui.label(format!("std combo row {i}"));
                                    }
                                });
                                ui.add_space(6.0);
                                card(ui, "Standard | Side buttons", |ui| {
                                    for i in 0..12 {
                                        ui.label(format!("std side row {i}"));
                                    }
                                });
                            });
                            ui.add_space(gap);
                            ui.vertical(|ui| {
                                ui.set_width(combo_w);
                                card(ui, "EasyShift | Main buttons", |ui| {
                                    for i in 0..40 {
                                        ui.label(format!("es combo row {i}"));
                                    }
                                });
                                ui.add_space(6.0);
                                card(ui, "EasyShift | Side buttons", |ui| {
                                    for i in 0..12 {
                                        ui.label(format!("es side row {i}"));
                                    }
                                });
                            });
                            ui.add_space(gap);
                            ui.vertical(|ui| {
                                ui.set_width(img_w);
                                for i in 0..30 {
                                    ui.label(format!("illustration row {i}"));
                                }
                            });
                        });
                    });
            });

            // Resting scrollbar handles carry the interactive
            // foreground colour (see apply_colors), which makes them
            // unambiguous to find among the rects.
            let handles = filled(&out, INTERACTIVE);
            let vertical: Vec<egui::Rect> = handles
                .iter()
                .copied()
                .filter(|r| r.height() > 100.0)
                .collect();
            assert_eq!(
                vertical.len(),
                1,
                "the tall scroll body keeps one vertical handle at width {width}"
            );
            assert!(
                (vertical[0].width() - 10.5).abs() <= 0.6,
                "the vertical handle keeps the theme's full 10.5 pt bar \
                 at width {width} - the v0.9.2 sliver bug shrank it when \
                 content overflowed horizontally, got {:.1} pt",
                vertical[0].width()
            );
            let horizontal: Vec<egui::Rect> = handles
                .iter()
                .copied()
                .filter(|r| r.width() > 100.0 && r.height() <= 20.0)
                .collect();
            assert!(
                horizontal.is_empty(),
                "no horizontal scrollbar at width {width}: the strip \
                 math must account for every point"
            );

            let max_x = out
                .shapes
                .iter()
                .map(|clipped| clipped.shape.visual_bounding_rect().max.x)
                .fold(f32::MIN, f32::max);
            assert!(
                max_x <= width + 0.5,
                "nothing may paint past the right edge at width {width}, got {max_x}"
            );

            // The pinned Apply/Revert pair stays centred in the
            // bottom strip at every pane width.
            let pair = buttons_in(&out, VIEWPORT_H - 120.0..VIEWPORT_H);
            assert_eq!(
                pair.len(),
                2,
                "the pinned bar's two buttons are painted at width {width}"
            );
            let pair_mid = (pair[0].max.x + pair[1].min.x) / 2.0;
            assert!(
                (pair_mid - width / 2.0).abs() <= TOL,
                "rule 4: the pinned Apply/Revert pair hugs the tab's \
                 centre line at width {width}, got {pair_mid}"
            );
        }
    }
}
