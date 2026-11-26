# GUI Layout Guide (egui 0.37)

Rules proven while polishing the NewNyth GUI in November 2026 (the rounds
that produced the centred cards, pinned action bars and two-line profile
headers). Every statement below was verified against egui 0.37 behaviour
in the running app, not just against its docs. Future GUI work should
follow these, and the planned headless layout test should encode them as
assertions.

## Centering

1. `Ui::horizontal_centered` centres on the cross axis, which for a
   horizontal layout is the *vertical* axis. It never centres content
   horizontally. Do not reach for it to centre a row.
2. `Ui::vertical_centered` (a top-down layout with `Align::Center`
   cross-align) centres the *direct children* of that ui. A
   `Ui::horizontal(...)` child fills the available width, so widgets
   added inside it pack against its left edge and the row looks
   left-aligned even though the layout claims to centre.
3. A single widget centres by being a direct item of a centring layout
   (e.g. `top_down(Align::Center)` or a card body); egui centres each
   direct item at its natural width. Wrapping it in a horizontal row is
   what breaks it (rule 2).
4. Two widgets that must hug the centre line (the Apply/Save pair, the
   Revert bar) use `theme::centered_pair_enabled`: each widget gets its
   own half-width strip anchored at the centre, so the pair stays
   symmetric about the centre line at any font scale no matter how wide
   either label is.
5. Rows of arbitrary content (DPI chips, sensitivity sliders, radio
   rows) use `theme::centered_row_fit`. It measures the row in a
   throwaway child ui placed 1e6 px to the right (outside the clip
   rect, so it is never visible and no pointer can ever land on it),
   then re-adds the same row in a strip of exactly the measured width,
   which the centring layout can then place. Constraints:
   - The closure runs twice per frame. It must be side-effect free: no
     mutation, no state-dependent widgets. The probe pass can never be
     hovered or clicked, so widgets must not depend on interaction.
   - `theme::centered_row` and `centered_row_sized` take a fixed strip
     width and pack narrower content against its left edge. Use them
     only when the content is known to fill the strip or stretches to
     it (rule 6).
6. Sliders stretch to fill whatever width they are given, so slider rows
   keep their fixed-width strips on purpose. Measuring such a row with
   `centered_row_fit` would just measure the available width.

## Text

7. `egui::Label` has no `vertical_align` in 0.37. Vertical centring of
   text comes from the layout: a horizontal layout with cross-align
   `Align::Center` draws single-line labels on the row's middle line.
8. `Label::truncate()` sets `WrapMode::Truncate`, which forces
   `max_rows = 1` for the whole galley. It is only safe on single-line
   labels: text containing `\n` silently collapses to one ellipsised
   line. To show two lines, add two labels and truncate each on its
   own.
9. Multi-line text inside a fixed-height bar overflows the bar. Stack
   the lines as separate labels and pad the block manually:
   `pad = (bar_h - lines * row_height - item_spacing.y * (lines - 1)) / 2`,
   applied with `ui.add_space(pad)`. Row height comes from
   `ui.fonts_mut(|f| f.row_height(&font_id))`; 0.37 needs the mutable
   fonts handle for this call.
10. `RichText::strong()` paints with `widgets.active.fg_stroke`, which
    the theme sets to `ACCENT`; `RichText::weak()` paints dim grey;
    plain labels use the near-white text colour.

## Card headers

11. `theme::card_inner` (reached through `card`, `card_with_header` and
    `card_with_header_and_actions`) draws the header strip as pinned
    bands: a left widget band (`HEADER_LEFT_W`), a right action band
    and the title region spanning everything between them. The title
    centres inside that region. All cards share identical header
    geometry, so the titles line up across cards. Do not trim the title
    region to a symmetric worst-case width "to be safe": that just
    steals horizontal room from long titles and makes them ellipsise
    early.
12. A single-line header band lays its title out with
    `Layout::centered_and_justified(Direction::LeftToRight)`. Justified
    means the label fills the band, and only then does
    `halign(Align::Center)` centre the text inside it; `halign` on a
    naturally short galley does nothing.
13. The two-line header band (`HEADER_HEIGHT_TALL`, used when a card
    carries a header closure) is a `top_down(Align::Center)` region and
    the closure pads its own content with the rule from item 9, so a
    one-line block and a two-line block both sit on the bar's middle
    line.
14. `egui::Grid` row callbacks only see the current column's available
    rect. Anything that needs the full card width (striped row bands,
    the add-app sub-header) must capture its extent from the card body
    ui before entering the grid: see `theme::CardRowBand`, and paint
    the band *before* adding the row's widgets so content paints on
    top.

## Testing

15. Layout bugs of this class live in egui's placement math and are
    invisible to unit tests. The headless suite in `theme.rs`
    (`mod layout_tests`) renders the real helpers and a replica of the
    Buttons tab's pinned bar and three-strip block through
    `egui::Context::run_pass` at fixed viewports, and asserts the rules
    above on the painted shapes: widgets centred on the centre line,
    text inside the header bars, row bands reaching the card edges, and
    the scrollbar/fit invariants of the Buttons pane at every approved
    width. It runs in CI with the normal `cargo test --workspace`.
    Two harness caveats learned while calibrating it: galley origins in
    centring/justified layouts are unreliable headlessly (assert on
    allocation rects or widths instead), and if the strip math in
    `app.rs::ui_buttons()` changes, the suite's replica must move with
    it in the same PR so CI keeps catching overflow.
