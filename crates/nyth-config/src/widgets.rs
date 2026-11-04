//! Custom widgets in the Roccat Swarm idiom.
//!
//! The centrepiece is [`SegmentedSlider`]: Swarm never shows a bare
//! slider — its rails are chunky rows of segments filled with the
//! accent colour, tick labels underneath, and the current value in a
//! dark read-out box to the right.

use eframe::egui;
use std::ops::RangeInclusive;

/// Rail height and handle width; sized after the Swarm bars.
const RAIL_H: f32 = 14.0;
const HANDLE_W: f32 = 10.0;
/// Gap between adjacent segments.
const GAP: f32 = 1.5;
/// Extra room below the rail for tick labels.
const TICK_H: f32 = 14.0;
/// Width of the value read-out box.
const BOX_W: f32 = 56.0;
/// Chunky default segment count when the value domain is too fine to
/// show one segment per value (e.g. CPI 200–12000 in steps of 10).
const DECORATIVE_SEGMENTS: usize = 19;

/// A segmented slider in the spirit of Roccat Swarm.
///
/// The rail is drawn as `segments` chunks, filled with the theme accent
/// up to the current value; a knob sits at the value position and picks
/// up the theme's hover/active styling (accent ring, matching the
/// sliders). Optional tick labels sit under the rail, proportionally
/// positioned.
///
/// ```ignore
/// let mut v = 6400.0;
/// ui.add(
///     widgets::SegmentedSlider::new(&mut v, 200.0..=12000.0)
///         .step(10.0)
///         .tick(200.0, "200")
///         .tick(12000.0, "12000")
///         .value_box(0),
/// );
/// ```
///
/// Interaction: click-to-jump, drag the knob anywhere along the rail,
/// or focus and use the arrow keys.
pub struct SegmentedSlider<'a> {
    value: &'a mut f32,
    range: RangeInclusive<f32>,
    step: f32,
    width: f32,
    ticks: Vec<(f32, String)>,
    value_box: bool,
    decimals: usize,
    /// Grey the fill out (Swarm dims the sliders of disabled CPI
    /// levels even though they still carry a value).
    dimmed: bool,
}

impl<'a> SegmentedSlider<'a> {
    pub fn new(value: &'a mut f32, range: RangeInclusive<f32>) -> Self {
        Self {
            value,
            range,
            step: 1.0,
            width: 280.0,
            ticks: Vec::new(),
            value_box: false,
            decimals: 0,
            dimmed: false,
        }
    }

    /// Snap increment along the rail (default 1).
    pub fn step(mut self, step: f32) -> Self {
        self.step = step;
        self
    }

    /// Rail width, excluding the value box (default 280).
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Add a tick label under the rail at `value` (proportional).
    pub fn tick(mut self, value: f32, label: impl Into<String>) -> Self {
        self.ticks.push((value, label.into()));
        self
    }

    /// Show the current value in a dark read-out box right of the rail.
    pub fn value_box(mut self, decimals: usize) -> Self {
        self.value_box = true;
        self.decimals = decimals;
        self
    }

    /// Draw the accent fill grey, Swarm-style, while the setting this
    /// slider edits is disabled.
    pub fn dimmed(mut self, dimmed: bool) -> Self {
        self.dimmed = dimmed;
        self
    }

    /// One segment per value up to 26 values, decorative otherwise.
    fn auto_segments(min: f32, max: f32, step: f32) -> usize {
        if step > 0.0 && max > min {
            let values = ((max - min) / step).floor() as usize + 1;
            if (2..=26).contains(&values) {
                return values;
            }
        }
        DECORATIVE_SEGMENTS
    }
}

/// Snap `raw` to the nearest multiple of `step` counted from `min`,
/// clamped into `[min, max]`.
fn quantize(raw: f32, min: f32, max: f32, step: f32) -> f32 {
    if step <= 0.0 || max <= min {
        return raw;
    }
    let steps = ((raw - min) / step).round();
    (min + steps * step).clamp(min, max)
}

impl egui::Widget for SegmentedSlider<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let Self {
            value,
            range,
            step,
            width,
            ticks,
            value_box,
            decimals,
            dimmed,
        } = self;
        let (min, max) = (*range.start(), *range.end());
        let enabled = ui.is_enabled();
        let spacing = ui.spacing().item_spacing.x;

        let segs = Self::auto_segments(min, max, step);
        let box_w = if value_box { BOX_W + spacing } else { 0.0 };
        let ticks_h = if ticks.is_empty() { 0.0 } else { GAP + TICK_H };

        let desired = egui::vec2(
            width + box_w,
            RAIL_H.max(ui.spacing().interact_size.y) + ticks_h,
        );
        let (rect, _) = ui.allocate_at_least(desired, egui::Sense::hover());
        let rail = egui::Rect::from_min_size(rect.min, egui::vec2(width, RAIL_H));

        // Hit area: the rail plus a few px of slack; not the value box.
        let id = ui.auto_id_with("segmented_slider");
        let mut response = ui.interact(
            rail.expand2(egui::vec2(2.0, 5.0)),
            id,
            egui::Sense::click_and_drag(),
        );

        // Pointer: click jumps, drag follows (travel measured handle-centre
        // to handle-centre, like egui's Slider).
        let travel = (rail.width() - HANDLE_W).max(1.0);
        if enabled && (response.dragged() || response.clicked()) {
            if let Some(pos) = response.interact_pointer_pos() {
                let t = ((pos.x - rail.min.x - HANDLE_W / 2.0) / travel).clamp(0.0, 1.0);
                let v = quantize(min + t * (max - min), min, max, step);
                if set_value(value, v) {
                    response.mark_changed();
                }
            }
        }
        // Keyboard nudge once focused.
        if enabled && response.has_focus() {
            let (up, down) = ui.input(|i| {
                (
                    i.key_pressed(egui::Key::ArrowRight) || i.key_pressed(egui::Key::ArrowUp),
                    i.key_pressed(egui::Key::ArrowLeft) || i.key_pressed(egui::Key::ArrowDown),
                )
            });
            if up || down {
                let d = if up { step } else { -step };
                let v = quantize(*value + d, min, max, step);
                if set_value(value, v) {
                    response.mark_changed();
                }
            }
        }

        // ── Paint ────────────────────────────────────────────────────
        let ratio = if max > min {
            ((*value - min) / (max - min)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let seg_on = if dimmed {
            ui.visuals().widgets.inactive.bg_fill
        } else {
            crate::theme::ACCENT
        };
        let seg_off = ui.visuals().widgets.inactive.bg_fill;
        let seg_stroke = ui.visuals().widgets.noninteractive.bg_stroke;

        let seg_w = ((rail.width() - (segs as f32 - 1.0) * GAP).max(1.0)) / segs as f32;
        for i in 0..segs {
            let x0 = rail.min.x + i as f32 * (seg_w + GAP);
            let seg = egui::Rect::from_min_max(
                egui::pos2(x0, rail.min.y),
                egui::pos2(x0 + seg_w, rail.max.y),
            )
            .round();
            let filled = (i as f32 + 0.5) / segs as f32 <= ratio + f32::EPSILON;
            let mut fill = if filled { seg_on } else { seg_off };
            if !enabled {
                fill = fill.linear_multiply(0.5);
            }
            ui.painter().rect_filled(seg, 1.0, fill);
            ui.painter().rect(
                seg,
                1.0,
                egui::Color32::TRANSPARENT,
                seg_stroke,
                egui::StrokeKind::Inside,
            );
        }

        // Knob: theme's interact() visuals — grey at rest, accent ring
        // on hover, accent-dim while dragged.
        let knob = ui.style().interact(&response);
        let kx = rail.min.x + HANDLE_W / 2.0 + ratio * travel;
        let knob_rect = egui::Rect::from_center_size(
            egui::pos2(kx, rail.center().y),
            egui::vec2(HANDLE_W, RAIL_H + 8.0),
        )
        .round();
        ui.painter().rect(
            knob_rect,
            2.0,
            knob.bg_fill,
            knob.fg_stroke,
            egui::StrokeKind::Inside,
        );

        if !ticks.is_empty() {
            let ty = rect.min.y + RAIL_H + GAP + TICK_H / 2.0;
            let weak = ui.visuals().weak_text_color();
            for (v, label) in &ticks {
                let t = if max > min {
                    ((v - min) / (max - min)).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let x = (rail.min.x + t * rail.width()).clamp(rail.min.x + 14.0, rail.max.x - 14.0);
                ui.painter().text(
                    egui::pos2(x, ty),
                    egui::Align2::CENTER_CENTER,
                    label,
                    egui::FontId::proportional(11.5),
                    weak,
                );
            }
        }

        if value_box {
            let bx = rail.max.x + spacing;
            let brect = egui::Rect::from_min_max(
                egui::pos2(bx, rail.min.y - 4.0),
                egui::pos2(bx + BOX_W, rail.max.y + 4.0),
            )
            .round();
            ui.painter()
                .rect_filled(brect, 2.0, ui.visuals().extreme_bg_color);
            ui.painter().rect(
                brect,
                2.0,
                egui::Color32::TRANSPARENT,
                seg_stroke,
                egui::StrokeKind::Inside,
            );
            ui.painter().text(
                brect.center(),
                egui::Align2::CENTER_CENTER,
                format!("{:.*}", decimals, *value),
                egui::FontId::proportional(13.0),
                ui.visuals().widgets.inactive.fg_stroke.color,
            );
        }

        response.on_hover_cursor(egui::CursorIcon::Grab)
    }
}

/// Assign if different; returns whether the value changed.
fn set_value(value: &mut f32, new: f32) -> bool {
    if *value != new {
        *value = new;
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantize_snaps_to_steps_from_min() {
        // CPI: 200..=12000 step 10 — an odd drag position snaps to the grid.
        assert_eq!(quantize(9097.0, 200.0, 12000.0, 10.0), 9100.0);
        // Even number of values still lands on the grid, not on 0.
        assert_eq!(quantize(204.0, 200.0, 12000.0, 10.0), 200.0);
        assert_eq!(quantize(205.0, 200.0, 12000.0, 10.0), 210.0);
    }

    #[test]
    fn quantize_clamps_and_passes_through_degenerate_cases() {
        assert_eq!(quantize(-50.0, 0.0, 10.0, 1.0), 0.0);
        assert_eq!(quantize(42.0, 0.0, 10.0, 1.0), 10.0);
        assert_eq!(quantize(3.7, 0.0, 10.0, 0.0), 3.7);
    }

    #[test]
    fn auto_segments_per_value_when_coarse() {
        // sensitivity −5..=5 step 1 → 11 values → 11 segments.
        assert_eq!(SegmentedSlider::auto_segments(-5.0, 5.0, 1.0), 11);
        // CPI is way too fine → decorative chunky count.
        assert_eq!(
            SegmentedSlider::auto_segments(200.0, 12000.0, 10.0),
            DECORATIVE_SEGMENTS
        );
    }
}
