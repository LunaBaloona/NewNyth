//! NewNyth egui application

use eframe::egui;

use roccat_nyth_hid::display;
use roccat_nyth_hid::protocol::*;

use crate::state::{ASSIGNABLE_BUTTON_TYPES, DeviceState, HID_KEYS, hid_key_name};

/// Flat-vector Nyth illustrations with the button numbering baked in, used
/// on the Buttons tab. Rendered in-repo (not Roccat artwork; see ROADMAP).
const NYTH_TOP_PNG: &[u8] = include_bytes!("../../../assets/images/nyth_top.png");
const NYTH_SIDE_PNG: &[u8] = include_bytes!("../../../assets/images/nyth_side.png");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Settings,
    Buttons,
    Advanced,
    Lighting,
    AutoSwitch,
}

pub struct NythConfigApp {
    state: DeviceState,
    selected_profile: u8,
    section: Section,
    auto_apply_profile: bool,
    status_message: Option<(String, f64)>,
    pending_settings: Option<ProfileSettings>,
    settings_dirty: bool,
    pending_buttons: Option<ProfileButtons>,
    buttons_dirty: bool,
    selected_button: Option<usize>,
    opener_edit_buf: String,
    // Auto-switch state
    autoswitch_dirty: bool,
    /// PC-side profile display-name edit buffers, indexed by profile.
    /// The Nyth has no name field, so these map straight onto
    /// `state.profile_names` and are saved as soon as they change.
    profile_name_buf: [String; NYTH_PROFILE_NUM as usize],
    /// When capturing: (profile_index, slot_index) to assign the next captured app to.
    capturing_for: Option<(u8, usize)>,
    /// Last time the active-profile byte was re-read (indicator refresh poll).
    last_profile_poll: std::time::Instant,
    /// Mouse illustrations for the Buttons tab, decoded lazily on first visit.
    tex_nyth_top: Option<egui::TextureHandle>,
    tex_nyth_side: Option<egui::TextureHandle>,
}

impl NythConfigApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        crate::theme::apply(&cc.egui_ctx);

        let state = DeviceState::load_from_device();
        let selected = state.active_profile;
        let profile_name_buf: [String; NYTH_PROFILE_NUM as usize] = std::array::from_fn(|i| {
            state
                .profile_names
                .get(i as u8)
                .unwrap_or_default()
                .to_string()
        });
        let profile = state
            .profiles
            .get(selected as usize)
            .and_then(|p| p.as_ref());

        Self {
            pending_settings: profile.map(|p| p.settings),
            pending_buttons: profile.map(|p| p.buttons),
            state,
            selected_profile: selected,
            section: Section::Settings,
            auto_apply_profile: true,
            status_message: None,
            settings_dirty: false,
            buttons_dirty: false,
            selected_button: None,
            opener_edit_buf: String::new(),
            autoswitch_dirty: false,
            profile_name_buf,
            capturing_for: None,
            last_profile_poll: std::time::Instant::now(),
            tex_nyth_top: None,
            tex_nyth_side: None,
        }
    }

    /// Decode one of the embedded mouse illustrations into a texture (once).
    fn mouse_texture(
        cache: &mut Option<egui::TextureHandle>,
        ctx: &egui::Context,
        name: &str,
        png: &[u8],
    ) -> Option<egui::TextureHandle> {
        if cache.is_none() {
            let rgba = image::load_from_memory(png).ok()?.into_rgba8();
            let color_img = egui::ColorImage::from_rgba_unmultiplied(
                [rgba.width() as usize, rgba.height() as usize],
                rgba.as_raw(),
            );
            *cache = Some(ctx.load_texture(name, color_img, egui::TextureOptions::LINEAR));
        }
        cache.clone()
    }

    /// Paint a mouse illustration aspect-fitted into `max_h` of the column,
    /// centred, never above `max_scale` of its natural size.
    fn mouse_illustration(
        ui: &mut egui::Ui,
        tex: Option<&egui::TextureHandle>,
        max_h: f32,
        max_scale: f32,
    ) {
        let Some(tex) = tex else { return };
        let avail_w = ui.available_width();
        let size = tex.size_vec2();
        let scale = (avail_w / size.x)
            .min(max_h / size.y)
            .min(1.0)
            .min(max_scale.max(0.0));
        let img_size = size * scale;
        let frame =
            egui::Rect::from_min_size(ui.cursor().min, egui::vec2(avail_w, max_h.max(img_size.y)));
        ui.advance_cursor_after_rect(frame);
        let rect = egui::Rect::from_center_size(frame.center(), img_size);
        let pad = 1.0;
        ui.painter()
            .rect_filled(rect.expand(pad), 4.0, crate::theme::ILLU_BG);
        ui.painter().image(
            tex.id(),
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
        ui.painter().rect_stroke(
            rect.expand(pad),
            4.0,
            egui::Stroke::new(1.0, crate::theme::EDGE),
            egui::StrokeKind::Outside,
        );
    }

    fn set_status(&mut self, msg: impl Into<String>, ctx: &egui::Context) {
        self.status_message = Some((msg.into(), ctx.input(|i| i.time)));
    }

    fn reload_device(&mut self, ctx: &egui::Context) {
        self.state = DeviceState::load_from_device();
        self.load_pending_from_profile();
        self.profile_name_buf = std::array::from_fn(|i| {
            self.state
                .profile_names
                .get(i as u8)
                .unwrap_or_default()
                .to_string()
        });
        if self.state.connected {
            self.set_status("Device data reloaded", ctx);
        }
    }

    fn load_pending_from_profile(&mut self) {
        let profile = self
            .state
            .profiles
            .get(self.selected_profile as usize)
            .and_then(|p| p.as_ref());
        self.pending_settings = profile.map(|p| p.settings);
        self.pending_buttons = profile.map(|p| p.buttons);
        self.settings_dirty = false;
        self.buttons_dirty = false;
        self.selected_button = None;
        self.opener_edit_buf.clear();
    }

    fn apply_settings(&mut self, ctx: &egui::Context) {
        if let Some(ref mut settings) = self.pending_settings {
            match self
                .state
                .apply_profile_settings(self.selected_profile, settings)
            {
                Ok(()) => {
                    self.settings_dirty = false;
                    if let Some(ref mut p) = self.state.profiles[self.selected_profile as usize] {
                        p.settings = *settings;
                    }
                    self.set_status(
                        format!("Profile {} settings saved", self.selected_profile + 1),
                        ctx,
                    );
                }
                Err(e) => self.set_status(format!("Error: {}", e), ctx),
            }
        }
    }

    fn apply_buttons(&mut self, ctx: &egui::Context) {
        if let Some(ref mut buttons) = self.pending_buttons {
            match self
                .state
                .apply_profile_buttons(self.selected_profile, buttons)
            {
                Ok(()) => {
                    self.buttons_dirty = false;
                    if let Some(ref mut p) = self.state.profiles[self.selected_profile as usize] {
                        p.buttons = *buttons;
                    }
                    // Also save the openers config for any Open Application buttons.
                    if let Err(e) = self.state.save_openers() {
                        log::warn!("Could not save openers config: {}", e);
                    }
                    self.set_status(
                        format!("Profile {} buttons saved", self.selected_profile + 1),
                        ctx,
                    );
                }
                Err(e) => self.set_status(format!("Error: {}", e), ctx),
            }
        }
    }

    fn activate_profile_on_device(&mut self, index: u8, ctx: &egui::Context) {
        match self.state.apply_active_profile(index) {
            Ok(()) => {
                self.state.active_profile = index;
                self.set_status(format!("Switched to profile {}", index + 1), ctx);
            }
            Err(e) => self.set_status(format!("Error: {}", e), ctx),
        }
    }

    /// Re-read the active-profile byte so the sidebar indicator follows
    /// changes made elsewhere — the daemon's auto-switch flips the profile
    /// on the device, and without this the GUI would keep highlighting
    /// whichever profile was active when it started. Throttled to 1 Hz.
    ///
    /// When the device moved on by itself, the viewed profile follows it
    /// (like the mini window), so the coloured highlight tracks the device.
    /// Unsaved edits in the current view block the follow until they are
    /// saved or discarded.
    fn poll_active_profile(&mut self) {
        if self.last_profile_poll.elapsed() < std::time::Duration::from_secs(1) {
            return;
        }
        self.last_profile_poll = std::time::Instant::now();
        match self.state.read_active_profile() {
            Ok(profile) => {
                if profile == self.state.active_profile {
                    return;
                }
                self.state.active_profile = profile;
                if !self.settings_dirty && !self.buttons_dirty && self.selected_profile != profile {
                    self.selected_profile = profile;
                    // The sidebar's own change detection can no longer
                    // see the jump (prev is already updated), so load
                    // the newly viewed profile's data here.
                    self.load_pending_from_profile();
                }
            }
            Err(e) => log::debug!("Active-profile poll failed: {e}"),
        }
    }
}

// ─── Main update loop ──────────────────────────────────────────────

impl eframe::App for NythConfigApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Panels below need `ui` mutably, so grab a cheap context handle up
        // front (Context is an Arc internally; cloning is cheap).
        let ctx = ui.ctx().clone();

        // Follow external profile changes (daemon auto-switch): refresh the
        // sidebar indicator at 1 Hz. The read is throttled internally, the
        // repaint request just keeps the app waking up at that rate.
        if self.state.connected {
            self.poll_active_profile();
            ctx.request_repaint_after(std::time::Duration::from_secs(1));
        }

        // ── Header: branding, device info, refresh ─────
        egui::Panel::top("header").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading(egui::RichText::new("NEWNYTH").strong());
                ui.separator();
                if self.state.connected {
                    ui.label(&self.state.device_name);
                    ui.label(format!("Firmware {}", self.state.firmware));
                    ui.label(egui::RichText::new(&self.state.hidraw_path).small().weak());
                } else {
                    ui.colored_label(egui::Color32::from_rgb(200, 80, 80), "No device found");
                    if let Some(ref msg) = self.state.error_message {
                        ui.label(egui::RichText::new(msg).small().weak());
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                            .small()
                            .weak(),
                    );
                    ui.separator();
                    if ui.button("↻ Refresh").clicked() {
                        self.reload_device(&ctx);
                    }
                });
            });
        });

        // ── Tab strip ──────────────────────────────────
        egui::Panel::top("tabs").show(ui, |ui| {
            ui.horizontal(|ui| {
                for (section, label) in [
                    (Section::Settings, "SETTINGS"),
                    (Section::Buttons, "BUTTON ASSIGNMENT"),
                    (Section::Advanced, "ADVANCED"),
                    (Section::Lighting, "LIGHTING"),
                    (Section::AutoSwitch, "AUTO-SWITCH"),
                ] {
                    if ui
                        .selectable_label(
                            self.section == section,
                            egui::RichText::new(label).strong(),
                        )
                        .clicked()
                    {
                        self.section = section;
                    }
                }
            });
        });

        // ── Bottom profile bar (the Swarm "game profiles" strip) ──
        egui::Panel::bottom("profile_bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                if self.state.connected {
                    ui.label(egui::RichText::new("Profile").strong());

                    let prev = self.selected_profile;
                    for i in 0..NYTH_PROFILE_NUM {
                        let btn = if i == self.selected_profile {
                            egui::Button::new(egui::RichText::new(format!("{}", i + 1)).strong())
                                .fill(ui.visuals().selection.bg_fill)
                        } else {
                            egui::Button::new(format!("{}", i + 1))
                        };
                        let tip = match self.state.profile_names.get(i) {
                            Some(name) => format!("Profile {}: {}", i + 1, name),
                            None => format!("Profile {}", i + 1),
                        };
                        let r = ui.add_sized([30.0, 26.0], btn).on_hover_text(tip);
                        if r.clicked() {
                            self.selected_profile = i;
                        }
                        if i == self.state.active_profile {
                            ui.painter().line_segment(
                                [r.rect.left_bottom(), r.rect.right_bottom()],
                                ui.visuals().widgets.active.fg_stroke,
                            );
                        }
                    }

                    if prev != self.selected_profile {
                        self.load_pending_from_profile();
                    }

                    ui.separator();
                    ui.checkbox(&mut self.auto_apply_profile, "Auto-apply");

                    if self.auto_apply_profile
                        && prev != self.selected_profile
                        && self.selected_profile != self.state.active_profile
                    {
                        let idx = self.selected_profile;
                        self.activate_profile_on_device(idx, &ctx);
                    }

                    if !self.auto_apply_profile
                        && self.selected_profile != self.state.active_profile
                        && ui.button("Activate on device").clicked()
                    {
                        let idx = self.selected_profile;
                        self.activate_profile_on_device(idx, &ctx);
                    }
                } else {
                    ui.colored_label(
                        egui::Color32::from_rgb(200, 80, 80),
                        "Plug in the Roccat Nyth to configure profiles.",
                    );
                }

                // Transient status messages, right-aligned.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some((ref msg, ts)) = self.status_message
                        && ui.input(|i| i.time) - ts < 5.0
                    {
                        ui.label(egui::RichText::new(msg).weak().italics());
                    }
                });
            });
        });

        // ── Poll for captured app (when in capture mode) ───
        if self.capturing_for.is_some() {
            if let Some(app_class) = DeviceState::poll_captured_app() {
                let (profile_idx, slot_idx) = self.capturing_for.take().unwrap();
                let apps = self.state.autoswitch.apps_for_profile_mut(profile_idx);

                // Check for duplicate.
                let already_exists = apps
                    .iter()
                    .any(|a| a.to_lowercase() == app_class.to_lowercase());

                if already_exists {
                    self.set_status(format!("'{}' is already assigned", app_class), &ctx);
                } else if slot_idx < apps.len() {
                    // Replace existing slot.
                    apps[slot_idx] = app_class.clone();
                    self.autoswitch_dirty = true;
                    self.set_status(
                        format!(
                            "Captured: {} → MP {} slot {}",
                            app_class,
                            profile_idx + 1,
                            slot_idx + 1
                        ),
                        &ctx,
                    );
                } else if apps.len() < roccat_nyth_profile::config::MAX_APPS_PER_PROFILE {
                    // Append to next empty slot.
                    apps.push(app_class.clone());
                    self.autoswitch_dirty = true;
                    self.set_status(
                        format!("Captured: {} → MP {}", app_class, profile_idx + 1),
                        &ctx,
                    );
                } else {
                    self.set_status("All slots full", &ctx);
                }
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }

        egui::CentralPanel::default().show(ui, |ui| {
            if !self.state.connected {
                ui.centered_and_justified(|ui| {
                    ui.heading("No Roccat Nyth detected");
                });
                return;
            }
            if self.section == Section::Buttons {
                // The button detail editor and Apply/Revert bar stay pinned
                // at the bottom of the pane (outside the scroll), so they
                // remain visible while the three button strips scroll.
                egui::Panel::bottom("buttons_actions").show(ui, |ui| {
                    self.ui_buttons_actions(ui, &ctx);
                });
                egui::ScrollArea::vertical()
                    .id_salt("section_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.ui_section(ui, &ctx));
            } else if matches!(
                self.section,
                Section::Settings | Section::Advanced | Section::Lighting
            ) {
                // Long card stacks must stay reachable when the window is
                // short. Auto-switch keeps its own profile scroll (the save
                // bar has to live outside it) and must not be wrapped twice.
                egui::ScrollArea::vertical()
                    .id_salt("section_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.ui_section(ui, &ctx));
            } else {
                self.ui_section(ui, &ctx);
            }
        });
    }
}

// ─── Settings panel (unchanged from v0.3.0) ────────────────────────

impl NythConfigApp {
    /// Paint the body of the active tab. Lives outside the `eframe::App`
    /// impl so both the plain and the scroll-wrapped CentralPanel paths
    /// can call it.
    fn ui_section(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        match self.section {
            Section::Settings => self.ui_settings(ui, ctx),
            Section::Buttons => self.ui_buttons(ui, ctx),
            Section::Advanced => self.ui_advanced(ui, ctx),
            Section::Lighting => self.ui_lighting(ui, ctx),
            Section::AutoSwitch => self.ui_autoswitch(ui, ctx),
        }
    }

    fn ui_settings(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let settings = match self.pending_settings.as_mut() {
            Some(s) => s as *mut ProfileSettings,
            None => {
                ui.label("No settings available.");
                return;
            }
        };
        let settings = unsafe { &mut *settings };

        ui.heading(format!("Profile {}: Settings", self.selected_profile + 1));
        ui.add_space(8.0);

        // Single stacked column, contents centred (Swarm keeps Settings
        // narrow and vertical rather than spreading over two columns).
        crate::theme::card(ui, "DPI switcher", |ui| {
            ui.vertical_centered(|ui| {
                for i in 0..NYTH_CPI_LEVELS_NUM {
                    crate::theme::centered_row_sized(ui, 540.0, 34.0, |ui| {
                        let mut en = settings.cpi_levels_enabled & (1 << i) != 0;
                        if ui.checkbox(&mut en, "").changed() {
                            if en {
                                settings.cpi_levels_enabled |= 1 << i;
                            } else {
                                settings.cpi_levels_enabled &= !(1 << i);
                            }
                            self.settings_dirty = true;
                        }
                        let mut cpi = settings.cpi_level(i) as f32;
                        let label = if settings.cpi_active as usize == i {
                            format!("Level {} ★", i + 1)
                        } else {
                            format!("Level {}", i + 1)
                        };
                        // Whole row (name, bar, value box) dims while the
                        // level is unchecked — only the checkbox stays live.
                        ui.add_enabled_ui(en, |ui| {
                            let text = if en {
                                egui::RichText::new(label)
                            } else {
                                egui::RichText::new(label).weak()
                            };
                            ui.add_sized(
                                [88.0, 20.0],
                                egui::Label::new(text).halign(egui::Align::Max),
                            );
                            // Swarm's DPI switcher: chunky segmented rail
                            // with preset ticks and a value box.
                            let slider = crate::widgets::SegmentedSlider::new(
                                &mut cpi,
                                NYTH_CPI_MIN as f32..=NYTH_CPI_MAX as f32,
                            )
                            .step(NYTH_CPI_STEP as f32)
                            .width(300.0)
                            .value_box(0)
                            .tick(200.0, "200")
                            .tick(2000.0, "2000")
                            .tick(4000.0, "4000")
                            .tick(6000.0, "6000")
                            .tick(8200.0, "8200")
                            .tick(12000.0, "12000");
                            if ui.add(slider).changed() {
                                settings.set_cpi_level(i, cpi as u16);
                                self.settings_dirty = true;
                            }
                        });
                    });
                }
                ui.add_space(4.0);
                crate::theme::centered_row(ui, 320.0, |ui| {
                    ui.label("Active level:");
                    for i in 0..NYTH_CPI_LEVELS_NUM {
                        if ui
                            .selectable_label(
                                settings.cpi_active as usize == i,
                                format!("{}", i + 1),
                            )
                            .clicked()
                            && settings.cpi_active as usize != i
                        {
                            settings.cpi_active = i as u8;
                            self.settings_dirty = true;
                        }
                    }
                });
            });
        });

        ui.add_space(8.0);
        crate::theme::card(ui, "Sensitivity", |ui| {
            ui.vertical_centered(|ui| {
                // Swarm shows sensitivity as −5…+5 around a neutral 0;
                // the device stores 1…11 (6 = neutral), a straight −6 shift.
                // This is the synced control: Y follows X unless advanced
                // sensitivity (Advanced tab) has split the axes.
                let mut sx = settings.sensitivity_x as f32 - 6.0;
                crate::theme::centered_row_sized(ui, 500.0, 34.0, |ui| {
                    let mut slider =
                        crate::widgets::SegmentedSlider::new(&mut sx, -5.0..=5.0).width(300.0);
                    for t in -5..=5 {
                        slider = slider.tick(t as f32, t.to_string());
                    }
                    if ui.add(slider.value_box(0)).changed() {
                        let v = (sx + 6.0).round() as u8;
                        settings.sensitivity_x = v;
                        if settings.advanced_sensitivity == 0 {
                            settings.sensitivity_y = v;
                        }
                        self.settings_dirty = true;
                    }
                });
                if settings.advanced_sensitivity != 0 {
                    ui.label(
                        egui::RichText::new(
                            "Separate axes are enabled - set X and Y on the Advanced tab.",
                        )
                        .small()
                        .weak(),
                    );
                }
            });
        });

        ui.add_space(12.0);
        self.apply_revert_bar(ui, ctx, true);
    }

    // ─── Advanced panel (Swarm's third tab) ──────────────────────────

    fn ui_advanced(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let settings = match self.pending_settings.as_mut() {
            Some(s) => s as *mut ProfileSettings,
            None => {
                ui.label("No settings available.");
                return;
            }
        };
        let settings = unsafe { &mut *settings };

        ui.heading(format!("Profile {}: Advanced", self.selected_profile + 1));
        ui.add_space(8.0);

        // ── Per-profile: split-axis sensitivity ──────────────────────
        crate::theme::card(ui, "Advanced sensitivity", |ui| {
            ui.vertical_centered(|ui| {
                let adv = settings.advanced_sensitivity != 0;
                let axes = [
                    ('X', settings.sensitivity_x, true),
                    ('Y', settings.sensitivity_y, adv),
                ];
                for (axis, value, enabled) in axes {
                    // −5…+5 display, device stores 1…11 (see Settings tab).
                    let mut v = value as f32 - 6.0;
                    crate::theme::centered_row_sized(ui, 540.0, 34.0, |ui| {
                        ui.add_sized(
                            [30.0, 20.0],
                            egui::Label::new(format!("{axis}:")).halign(egui::Align::Max),
                        );
                        ui.add_enabled_ui(enabled, |ui| {
                            let mut slider =
                                crate::widgets::SegmentedSlider::new(&mut v, -5.0..=5.0)
                                    .width(300.0);
                            for t in -5..=5 {
                                slider = slider.tick(t as f32, t.to_string());
                            }
                            if ui.add(slider.value_box(0)).changed() {
                                let raw = (v + 6.0).round() as u8;
                                if axis == 'X' {
                                    settings.sensitivity_x = raw;
                                } else {
                                    settings.sensitivity_y = raw;
                                }
                                self.settings_dirty = true;
                            }
                        });
                    });
                }
                ui.add_space(4.0);
                let mut adv = settings.advanced_sensitivity != 0;
                if ui
                    .checkbox(&mut adv, "Enable advanced sensitivity")
                    .changed()
                {
                    settings.advanced_sensitivity = if adv { 1 } else { 0 };
                    // Never jump: the Y axis starts where X is, whichever
                    // way the switch is thrown.
                    settings.sensitivity_y = settings.sensitivity_x;
                    self.settings_dirty = true;
                }
            });
        });

        ui.add_space(8.0);
        crate::theme::card(ui, "Polling rate", |ui| {
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("How often the mouse reports to the computer.")
                        .small()
                        .weak(),
                );
                let mut rate = settings.polling_rate;
                ui.vertical_centered(|ui| {
                    for (v, l) in [
                        (1u8, "125 Hz"),
                        (2, "250 Hz"),
                        (3, "500 Hz"),
                        (4, "1000 Hz"),
                    ] {
                        if ui.radio_value(&mut rate, v, l).changed() {
                            settings.polling_rate = rate;
                            self.settings_dirty = true;
                        }
                    }
                });
            });
        });

        // ── Device-level: sensor control units (report 0x0f) ─────────
        ui.add_space(8.0);
        crate::theme::card(ui, "Tracking Control Unit", |ui| {
            ui.vertical_centered(|ui| {
                match self.state.control_unit {
                    Some(cu) => {
                        let on = cu.tcu != 0;
                        ui.label(
                            egui::RichText::new(format!(
                                "Lift-off detection is currently {}.",
                                if on { "on" } else { "off" }
                            ))
                            .small()
                            .weak(),
                        );
                        crate::theme::centered_row(ui, 220.0, |ui| {
                            // The device only ever switches the TCU on via
                            // the surface-calibration handshake, which
                            // NewNyth does not drive yet — so "On" is
                            // display-only and switching off is the write.
                            ui.add_enabled_ui(false, |ui| {
                                let _ = ui.radio(on, "On");
                            });
                            if ui.radio(!on, "Off").clicked() && on {
                                match self.state.apply_tracking_control_off() {
                                    Ok(()) => {
                                        self.state.refresh_control_unit();
                                        self.set_status("Tracking Control Unit off", ctx);
                                    }
                                    Err(e) => self.set_status(format!("Error: {e}"), ctx),
                                }
                            }
                        });
                        ui.label(
                            egui::RichText::new(
                                "Switching it back on needs surface calibration, not offered by NewNyth yet.",
                            )
                            .small()
                            .weak(),
                        );
                    }
                    None => {
                        ui.label("Could not read the sensor state.");
                        ui.label(
                            egui::RichText::new("Use ↻ Refresh once the mouse is reachable.")
                                .small()
                                .weak(),
                        );
                    }
                }
            });
        });

        ui.add_space(8.0);
        crate::theme::card(ui, "Distance Control Unit", |ui| {
            ui.vertical_centered(|ui| {
                match self.state.control_unit {
                    Some(cu) => {
                        let current = DistanceControl::from_u8(cu.dcu);
                        ui.label(
                            egui::RichText::new(
                                "Lift-off distance: how high the mouse may float before tracking stops.",
                            )
                            .small()
                            .weak(),
                        );
                        crate::theme::centered_row(ui, 420.0, |ui| {
                            for d in DistanceControl::ALL {
                                if ui.radio(d == current, Self::dcu_label(d)).clicked()
                                    && d != current
                                {
                                    match self.state.apply_distance_control(d) {
                                        Ok(()) => {
                                            self.state.refresh_control_unit();
                                            self.set_status(
                                                format!("Lift-off distance: {}", Self::dcu_label(d)),
                                                ctx,
                                            );
                                        }
                                        Err(e) => self.set_status(format!("Error: {e}"), ctx),
                                    }
                                }
                            }
                        });
                    }
                    None => {
                        ui.label("Could not read the sensor state.");
                    }
                }
            });
        });

        ui.add_space(6.0);
        ui.label(
            egui::RichText::new(
                "Tracking and lift-off distance are mouse-wide settings: they apply immediately and are shared by all profiles.",
            )
            .small()
            .weak(),
        );

        ui.add_space(8.0);
        self.apply_revert_bar(ui, ctx, true);
    }

    /// Swarm's labels for the lift-off distance settings.
    fn dcu_label(d: DistanceControl) -> &'static str {
        match d {
            DistanceControl::Off => "Off (default)",
            DistanceControl::Normal => "Normal",
            DistanceControl::Low => "Low",
            DistanceControl::ExtraLow => "Extra Low",
        }
    }

    // ─── Lighting panel ──────────────────────────────────────

    fn ui_lighting(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let settings = match self.pending_settings.as_mut() {
            Some(s) => s as *mut ProfileSettings,
            None => {
                ui.label("No settings available.");
                return;
            }
        };
        let settings = unsafe { &mut *settings };

        ui.heading(format!("Profile {}: Lighting", self.selected_profile + 1));
        ui.add_space(8.0);

        let mut enabled = (settings.lights_enabled & 0x03) == 3;
        if ui.checkbox(&mut enabled, "Lighting enabled").changed() {
            settings.lights_enabled = if enabled {
                (settings.lights_enabled & !0x03) | 0x03
            } else {
                settings.lights_enabled & !0x03
            };
            self.settings_dirty = true;
        }

        if !enabled {
            ui.label(egui::RichText::new("Lighting is turned off.").weak());
        } else {
            ui.add_space(8.0);
            crate::theme::card(ui, "Colours", |ui| {
                // Roomier text and fatter swatches take up the space the
                // (removed) preview card used to fill — the colour well
                // sizes itself from `interact_size`. `style_mut` is
                // clone-on-write: it applies to this card's `Ui` and its
                // children, not to the sibling cards.
                let style = ui.style_mut();
                style
                    .text_styles
                    .insert(egui::TextStyle::Body, egui::FontId::proportional(16.0));
                style
                    .text_styles
                    .insert(egui::TextStyle::Button, egui::FontId::proportional(16.0));
                style.spacing.interact_size = egui::vec2(26.0, 26.0);
                ui.vertical_centered(|ui| {
                    for (i, zone) in ["Logo", "Bottom"].iter().enumerate() {
                        crate::theme::centered_row(ui, 460.0, |ui| {
                            ui.add_sized(
                                [72.0, 26.0],
                                egui::Label::new(format!("{zone}:")).halign(egui::Align::Max),
                            );
                            let l = &mut settings.lights[i];
                            let mut c = [
                                l.red as f32 / 255.0,
                                l.green as f32 / 255.0,
                                l.blue as f32 / 255.0,
                            ];
                            if ui.color_edit_button_rgb(&mut c).changed() {
                                l.red = (c[0] * 255.0) as u8;
                                l.green = (c[1] * 255.0) as u8;
                                l.blue = (c[2] * 255.0) as u8;
                                self.settings_dirty = true;
                            }
                            ui.add_sized(
                                [98.0, 26.0],
                                egui::Label::new(format!(
                                    "#{:02x}{:02x}{:02x}",
                                    l.red, l.green, l.blue
                                ))
                                .halign(egui::Align::Min),
                            );
                            let bit = 4 + i;
                            let mut custom = settings.lights_enabled & (1 << bit) != 0;
                            if ui.checkbox(&mut custom, "Custom colour").changed() {
                                if custom {
                                    settings.lights_enabled |= 1 << bit;
                                } else {
                                    settings.lights_enabled &= !(1 << bit);
                                }
                                self.settings_dirty = true;
                            }
                        });
                    }
                });
            });
            ui.add_space(8.0);
            crate::theme::card(ui, "Effects", |ui| {
                ui.vertical_centered(|ui| {
                    crate::theme::centered_row(ui, 420.0, |ui| {
                        ui.label("Light effect:");
                        for (v, l) in [
                            (1u8, "Fully lit"),
                            (2, "Blinking"),
                            (3, "Breathing"),
                            (4, "Heartbeat"),
                        ] {
                            if ui.selectable_label(settings.light_effect == v, l).clicked()
                                && settings.light_effect != v
                            {
                                settings.light_effect = v;
                                self.settings_dirty = true;
                            }
                        }
                    });
                    if settings.light_effect > 1 {
                        let mut spd = settings.effect_speed as f32;
                        crate::theme::centered_row(ui, 400.0, |ui| {
                            if ui
                                .add(egui::Slider::new(&mut spd, 1.0..=3.0).text("Speed"))
                                .changed()
                            {
                                settings.effect_speed = spd as u8;
                                self.settings_dirty = true;
                            }
                        });
                    }
                    ui.add_space(4.0);
                    crate::theme::centered_row(ui, 430.0, |ui| {
                        ui.label("Colour effect:");
                        for (v, l) in [(0u8, "Off"), (1, "Simultaneously"), (2, "Up"), (3, "Down")]
                        {
                            if ui.selectable_label(settings.color_effect == v, l).clicked()
                                && settings.color_effect != v
                            {
                                settings.color_effect = v;
                                self.settings_dirty = true;
                            }
                        }
                    });
                });
            });
        }

        ui.add_space(12.0);
        self.apply_revert_bar(ui, ctx, true);
    }

    // ─── Buttons panel (v0.7.1: 4-quadrant layout) ──────────

    fn ui_buttons(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context) {
        ui.heading(format!("Profile {}: Buttons", self.selected_profile + 1));
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Select a button to configure. Shortcut and Open application have additional options below.").weak().italics());
        ui.add_space(8.0);

        let buttons = match self.pending_buttons.as_mut() {
            Some(b) => b as *mut ProfileButtons,
            None => {
                ui.label("No button data available.");
                return;
            }
        };
        let buttons = unsafe { &mut *buttons };

        // Button index ranges:
        // Standard main: 0..8    (Left, Right, Middle, Wheel up/down, Top, Fin left/right)
        // Standard side: 8..20   (Side 1–12)
        // EasyShift main: 20..28
        // EasyShift side: 28..40

        // The three-column block below flows directly into the outer
        // `section_scroll` (same as the other tabs) so the scrollbar
        // styling stays uniform; it must not open a second scroll area.
        {
            // Three columns sized to their content: Standard combos,
            // EasyShift combos (each hugging the dropdowns), and the
            // numbered mouse illustrations taking the remaining width.
            let avail_w = ui.available_width();
            let gap = ui.spacing().item_spacing.x.max(10.0);
            // `ui.horizontal` inserts its own item spacing between the
            // columns on top of the explicit gaps; if the block is even a
            // few points wider than the viewport, egui shrinks the outer
            // scrollbar to a sliver, so account for every point here.
            let junction = gap + ui.spacing().item_spacing.x;
            let combo_w = 292.0f32;
            let img_w = (avail_w - 2.0 * combo_w - 2.0 * junction - 2.0).clamp(220.0, 560.0);
            let block_w = 2.0 * combo_w + img_w + 2.0 * junction;
            let pad = ((avail_w - block_w) / 2.0).max(0.0);

            let top_tex =
                Self::mouse_texture(&mut self.tex_nyth_top, ui.ctx(), "nyth_top", NYTH_TOP_PNG);
            let side_tex = Self::mouse_texture(
                &mut self.tex_nyth_side,
                ui.ctx(),
                "nyth_side",
                NYTH_SIDE_PNG,
            );

            let mut main_card_h = 0.0f32;
            let mut side_card_h = 0.0f32;

            ui.horizontal(|ui| {
                ui.add_space(pad);

                // ── Column 1: Standard layer ────────────────
                ui.vertical(|ui| {
                    ui.set_width(combo_w);
                    main_card_h = crate::theme::card(ui, "Standard | Main buttons", |ui| {
                        self.button_grid(ui, buttons, 0..8, "btn_std_main", 0, true);
                    })
                    .response
                    .rect
                    .height();
                    ui.add_space(6.0);
                    side_card_h = crate::theme::card(ui, "Standard | Side buttons", |ui| {
                        self.button_grid(ui, buttons, 8..20, "btn_std_side", 4, false);
                    })
                    .response
                    .rect
                    .height();
                });

                ui.add_space(gap);

                // ── Column 2: EasyShift layer ───────────────
                ui.vertical(|ui| {
                    ui.set_width(combo_w);
                    crate::theme::card(ui, "EasyShift | Main buttons", |ui| {
                        self.button_grid(ui, buttons, 20..28, "btn_es_main", 0, true);
                    });
                    ui.add_space(6.0);
                    crate::theme::card(ui, "EasyShift | Side buttons", |ui| {
                        self.button_grid(ui, buttons, 28..40, "btn_es_side", 4, false);
                    });
                });

                ui.add_space(gap);

                // ── Column 3: numbered mouse illustrations ──
                // Top mouse: 25% more height than the card beside it
                // (fixed — its numbers are the tab's legend). Side
                // keypad: capped at 0.55× natural size (~470px wide on
                // the user's 160% display), the size that reads best
                // beside the combo cards without dominating them.
                ui.vertical(|ui| {
                    ui.set_width(img_w);
                    Self::mouse_illustration(ui, top_tex.as_ref(), main_card_h * 1.25, 1.0);
                    ui.add_space(6.0);
                    Self::mouse_illustration(ui, side_tex.as_ref(), side_card_h, 0.55);
                });
            });
        }

        // The detail editor + Apply/Revert bar are pinned at the bottom of
        // the pane by the CentralPanel path (see `ui_buttons_actions`),
        // outside this scroll body.
    }

    /// Detail editor + Apply/Revert bar for the Buttons tab, shown in a
    /// pinned bottom panel so they stay visible while the strips scroll.
    fn ui_buttons_actions(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let buttons = match self.pending_buttons.as_mut() {
            Some(b) => b as *mut ProfileButtons,
            None => return,
        };
        let buttons = unsafe { &mut *buttons };

        ui.separator();

        // ── Detail editor for selected button ───────
        self.ui_button_detail(ui, buttons);

        ui.add_space(8.0);
        self.apply_revert_bar(ui, ctx, false);
    }

    fn button_grid(
        &mut self,
        ui: &mut egui::Ui,
        buttons: &mut ProfileButtons,
        range: std::ops::Range<usize>,
        grid_id: &str,
        group_every: usize,
        numbered: bool,
    ) {
        let range_start = range.start;
        // Long grids (12-key side keypads) are split into groups of
        // `group_every` rows separated by an accent-blue rule, mirroring
        // the 4×3 keypad blocks on the physical mouse.
        let mut groups = Vec::new();
        if group_every == 0 {
            groups.push(range);
        } else {
            let mut start = range.start;
            while start < range.end {
                let end = (start + group_every).min(range.end);
                groups.push(start..end);
                start += group_every;
            }
        }

        let row_h = ui.spacing().interact_size.y;
        for (gi, group) in groups.into_iter().enumerate() {
            if gi > 0 {
                ui.add_space(5.0);
                crate::theme::accent_rule(ui);
                ui.add_space(5.0);
            }
            egui::Grid::new(format!("{grid_id}_{gi}"))
                .num_columns(2)
                .spacing([8.0, 2.0])
                .striped(true)
                .show(ui, |ui| {
                    for i in group {
                        let is_selected = self.selected_button == Some(i);
                        let btn = &mut buttons.buttons[i];
                        let name = if i >= NYTH_PHYSICAL_BUTTON_NUM {
                            display::BUTTON_NAMES[i]
                                .strip_prefix("Shift+")
                                .unwrap_or(display::BUTTON_NAMES[i])
                        } else {
                            display::BUTTON_NAMES[i]
                        };

                        // Main-button rows carry the illustration number as
                        // a "N: " prefix so the cards and the numbered mouse
                        // art cross-reference (side grids self-number).
                        let name_text = if numbered {
                            format!("{}: {}", i - range_start + 1, name)
                        } else {
                            name.to_string()
                        };

                        // Button name (clickable to select), pinned width so
                        // the dropdown column lines up across the groups,
                        // left-aligned inside that column.
                        let name_label = if is_selected {
                            egui::RichText::new(name_text).strong()
                        } else {
                            egui::RichText::new(name_text)
                        };
                        let name_response = ui
                            .allocate_ui_with_layout(
                                egui::vec2(96.0, row_h),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.add(
                                        egui::Label::new(name_label)
                                            .selectable(false)
                                            .sense(egui::Sense::click()),
                                    )
                                },
                            )
                            .inner;
                        if name_response.clicked() {
                            self.selected_button = Some(i);
                            // Load opener text if this is an Open application button.
                            self.opener_edit_buf = self
                                .state
                                .openers
                                .get(self.selected_profile, i as u8)
                                .unwrap_or("")
                                .to_string();
                        }

                        // Type dropdown
                        let current_name = display::button_type_name(btn.button_type);
                        let display_text = match btn.button_type {
                            0x05 => {
                                // Show shortcut summary inline
                                let mods = display::modifier_string(btn.modifier);
                                let key = hid_key_name(btn.key);
                                if btn.key != 0 {
                                    format!("⌨ {mods}+{key}")
                                } else {
                                    "⌨ Shortcut (click to set)".into()
                                }
                            }
                            0x0f => {
                                let cmd = self
                                    .state
                                    .openers
                                    .get(self.selected_profile, i as u8)
                                    .unwrap_or("");
                                if cmd.is_empty() {
                                    "▶ Open app (click to set)".into()
                                } else {
                                    format!("▶ {}", truncate(cmd, 20))
                                }
                            }
                            _ => current_name.to_string(),
                        };

                        let mut changed = false;
                        egui::ComboBox::from_id_salt(format!("btn_{i}"))
                            .selected_text(&display_text)
                            .width(170.0)
                            .show_ui(ui, |ui| {
                                for &(type_val, type_name) in ASSIGNABLE_BUTTON_TYPES {
                                    if ui
                                        .selectable_label(btn.button_type == type_val, type_name)
                                        .clicked()
                                        && btn.button_type != type_val
                                    {
                                        btn.button_type = type_val;
                                        if type_val != 0x05 {
                                            btn.modifier = 0;
                                            btn.key = 0;
                                        }
                                        changed = true;
                                        // Auto-select this button for detail editing.
                                        self.selected_button = Some(i);
                                        self.opener_edit_buf = String::new();
                                    }
                                }
                            });

                        if changed {
                            self.buttons_dirty = true;
                        }
                        ui.end_row();
                    }
                });
        }
    }

    /// Detail editor panel shown below the button grids.
    fn ui_button_detail(&mut self, ui: &mut egui::Ui, buttons: &mut ProfileButtons) {
        let btn_idx = match self.selected_button {
            Some(i) => i,
            None => {
                ui.label(egui::RichText::new("Click a button name above to configure it.").weak());
                return;
            }
        };

        let btn = &mut buttons.buttons[btn_idx];
        let name = display::BUTTON_NAMES[btn_idx];

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("Editing: {name}")).strong());
            ui.label(format!(
                "(type: {})",
                display::button_type_name(btn.button_type)
            ));
        });

        match btn.button_type {
            // ── Shortcut editor ──────────────────────
            0x05 => {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("Modifiers:");

                    let mut ctrl = btn.modifier & (1 << 2) != 0;
                    let mut shift = btn.modifier & (1 << 1) != 0;
                    let mut alt = btn.modifier & (1 << 3) != 0;
                    let mut win = btn.modifier & (1 << 4) != 0;

                    let mut mod_changed = false;
                    if ui.checkbox(&mut ctrl, "Ctrl").changed() {
                        mod_changed = true;
                    }
                    if ui.checkbox(&mut shift, "Shift").changed() {
                        mod_changed = true;
                    }
                    if ui.checkbox(&mut alt, "Alt").changed() {
                        mod_changed = true;
                    }
                    if ui.checkbox(&mut win, "Win").changed() {
                        mod_changed = true;
                    }

                    if mod_changed {
                        btn.modifier = 0;
                        if ctrl {
                            btn.modifier |= 1 << 2;
                        }
                        if shift {
                            btn.modifier |= 1 << 1;
                        }
                        if alt {
                            btn.modifier |= 1 << 3;
                        }
                        if win {
                            btn.modifier |= 1 << 4;
                        }
                        self.buttons_dirty = true;
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Key:");
                    let current_key_name = hid_key_name(btn.key);
                    egui::ComboBox::from_id_salt("shortcut_key")
                        .selected_text(current_key_name)
                        .width(180.0)
                        .show_ui(ui, |ui| {
                            for &(hid_val, hid_name) in HID_KEYS {
                                if ui.selectable_label(btn.key == hid_val, hid_name).clicked()
                                    && btn.key != hid_val
                                {
                                    btn.key = hid_val;
                                    self.buttons_dirty = true;
                                }
                            }
                        });

                    if btn.key != 0 {
                        let summary = format!(
                            "Result: {}+{}",
                            display::modifier_string(btn.modifier),
                            hid_key_name(btn.key)
                        );
                        ui.label(egui::RichText::new(summary).weak());
                    }
                });
            }

            // ── Open application editor ─────────────
            0x0f => {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("Command:");
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.opener_edit_buf)
                            .hint_text("e.g. firefox, /usr/bin/dolphin, flatpak run ...")
                            .desired_width(400.0),
                    );
                    if response.changed() {
                        self.state.openers.set(
                            self.selected_profile,
                            btn_idx as u8,
                            &self.opener_edit_buf,
                        );
                        self.buttons_dirty = true;
                    }
                });
                ui.label(
                    egui::RichText::new(
                        "Enter any command or application path. The daemon (nythd) will launch it when this button is pressed."
                    ).small().weak()
                );
            }

            // ── Other types: no additional config ───
            _ => {
                ui.label(
                    egui::RichText::new("No additional configuration for this button type.").weak(),
                );
            }
        }
    }

    // ─── Shared apply / revert bar ─────────────────────────────

    fn apply_revert_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, is_settings: bool) {
        let dirty = if is_settings {
            self.settings_dirty
        } else {
            self.buttons_dirty
        };
        ui.horizontal(|ui| {
            let label = if is_settings {
                "Apply settings"
            } else {
                "Apply button mappings"
            };
            if ui.add_enabled(dirty, egui::Button::new(label)).clicked() {
                if is_settings {
                    self.apply_settings(ctx);
                } else {
                    self.apply_buttons(ctx);
                }
            }
            if ui.add_enabled(dirty, egui::Button::new("Revert")).clicked() {
                self.load_pending_from_profile();
                self.set_status("Changes reverted", ctx);
            }
            if dirty {
                ui.label(egui::RichText::new("● Unsaved changes").weak().italics());
            }
        });
    }

    // ─── Auto-switch panel (vertical stacked layout) ──────────

    fn ui_autoswitch(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("Profile Auto-switch");
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(
                "Automatically switch mouse profiles when you focus different application windows.",
            )
            .weak()
            .italics(),
        );
        ui.add_space(8.0);

        if ui
            .checkbox(
                &mut self.state.autoswitch.enabled,
                "Enable profile auto-switching",
            )
            .changed()
        {
            self.autoswitch_dirty = true;
        }

        ui.add_space(8.0);

        // The save bar must be added *before* the profile scroll area below:
        // a vertical ScrollArea expands to fill all remaining window height,
        // so anything added after it falls outside the window and can never
        // be seen or clicked (the bug that made captured apps unsaveable).
        self.ui_autoswitch_save_bar(ui, ctx);

        if !self.state.autoswitch.enabled {
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Auto-switching is disabled.").weak());
            return;
        }

        ui.add_space(8.0);

        // Capture mode indicator
        if let Some((cap_profile, _)) = self.capturing_for {
            ui.horizontal(|ui| {
                ui.colored_label(
                    egui::Color32::from_rgb(255, 180, 50),
                    format!(
                        "⏳ Click any window to assign it to Mouse Profile {}…",
                        cap_profile + 1
                    ),
                );
                if ui.button("Cancel").clicked() {
                    self.capturing_for = None;
                    DeviceState::cancel_capture();
                    self.set_status("Capture cancelled", ctx);
                }
            });
            ui.add_space(4.0);
        }

        // ── Profile cards, wrapping into width-responsive columns ──
        // One card per profile; the column count is recomputed from the
        // available width every frame, so resizing the window reflows the
        // layout (Swarm crams the profiles into a table; we keep cards).
        let max_apps = roccat_nyth_profile::config::MAX_APPS_PER_PROFILE;
        let min_card_w = 340.0;
        let gap = ui.spacing().item_spacing.x;
        let avail_w = ui.available_width();
        let num_cols = (((avail_w + gap) / (min_card_w + gap)).floor() as usize)
            .clamp(1, NYTH_PROFILE_NUM as usize);

        egui::ScrollArea::vertical()
            .id_salt("autoswitch_main_scroll")
            .show(ui, |ui| {
                // Row-major flow: 1 2 3 across the top, 4 5 (… 6) below.
                // Each card stretches to fill an equal-width cell of its row.
                let rows = (NYTH_PROFILE_NUM as usize).div_ceil(num_cols);
                for row in 0..rows {
                    ui.horizontal(|ui| {
                        // Explicit `add_space` gaps keep the cells exactly
                        // `gap` apart without double-counting item spacing.
                        ui.spacing_mut().item_spacing.x = 0.0;
                        let cell_w =
                            (ui.available_width() - (num_cols - 1) as f32 * gap) / num_cols as f32;
                        for col in 0..num_cols {
                            let p = row * num_cols + col;
                            if p >= NYTH_PROFILE_NUM as usize {
                                break;
                            }
                            if col > 0 {
                                ui.add_space(gap);
                            }
                            let profile_idx = p as u8;
                            let is_default =
                                self.state.autoswitch.default_profile == Some(profile_idx);
                            let cap_here =
                                self.capturing_for.map(|(pp, _)| pp) == Some(profile_idx);

                            // The header's "Default" toggle must not borrow
                            // `self` (the body closure already does), so it
                            // parks the desired change for after the card.
                            let mut new_default: Option<Option<u8>> = None;

                            ui.vertical(|ui| {
                                ui.set_width(cell_w);
                                // Fixed "Mouse Profile N" prefix, optional
                                // PC-side name behind a colon.
                                let name_suffix = self
                                    .state
                                    .profile_names
                                    .get(profile_idx)
                                    .map(|n| format!(": {}", n))
                                    .unwrap_or_default();
                                let header =
                                    format!("Mouse Profile {}{}", profile_idx + 1, name_suffix);
                                crate::theme::card_with_actions(
                                    ui,
                                &header,
                                &mut |ui| {
                                    let mut def = is_default;
                                    if ui
                                        .checkbox(&mut def, "")
                                        .on_hover_text(
                                            "Profile to activate when no listed app is focused",
                                        )
                                        .changed()
                                    {
                                        new_default =
                                            Some(if def { Some(profile_idx) } else { None });
                                    }
                                    ui.label("Default");
                                },
                                |ui| {
                                    // Display-name editor: saved to the PC
                                    // config immediately (the device has no
                                    // name field, so nothing has to be
                                    // written to the mouse).
                                    let name_w = ui.available_width().max(120.0);
                                    let name_resp = ui
                                        .add_sized(
                                            [name_w, 20.0],
                                            egui::TextEdit::singleline(
                                                &mut self.profile_name_buf[profile_idx as usize],
                                            )
                                            .hint_text("no name"),
                                        )
                                        .on_hover_text(
                                            "Display name, stored on this computer only. It \
                                             shows here and as a tooltip on the profile-slot \
                                             buttons.",
                                        );
                                    if name_resp.changed() {
                                        let i = profile_idx as usize;
                                        self.profile_name_buf[i] = self.profile_name_buf[i]
                                            .chars()
                                            .take(roccat_nyth_profile::config::MAX_PROFILE_NAME_LEN)
                                            .collect();
                                        self.state.profile_names.set(
                                            profile_idx,
                                            &self.profile_name_buf[i],
                                        );
                                        if let Err(e) = self.state.save_profile_names() {
                                            self.set_status(
                                                format!("Could not save profile name: {e}"),
                                                ctx,
                                            );
                                        }
                                    }
                                    ui.separator();

                                    let apps = self
                                        .state
                                        .autoswitch
                                        .apps_for_profile(profile_idx)
                                        .to_vec();
                                    let mut to_remove: Option<usize> = None;

                                    for (slot, app) in apps.iter().enumerate() {
                                        ui.horizontal(|ui| {
                                            let name_w =
                                                (ui.available_width() - 140.0).max(60.0);
                                            ui.add_sized(
                                                [name_w, 20.0],
                                                egui::Label::new(
                                                    egui::RichText::new(app.as_str()).monospace(),
                                                )
                                                .truncate(),
                                            );
                                            if self.capturing_for == Some((profile_idx, slot)) {
                                                ui.label(
                                                    egui::RichText::new("Waiting…")
                                                        .weak()
                                                        .italics(),
                                                );
                                            } else if ui
                                                .button("Capture")
                                                .on_hover_text(
                                                    "Assign a newly focused window to this slot",
                                                )
                                                .clicked()
                                            {
                                                self.capturing_for = Some((profile_idx, slot));
                                                DeviceState::start_capture();
                                                self.set_status(
                                                    format!(
                                                        "Click a window to replace slot {} in MP {}…",
                                                        slot + 1,
                                                        profile_idx + 1
                                                    ),
                                                    ctx,
                                                );
                                            }
                                            if ui.button("✕").on_hover_text("Remove").clicked() {
                                                to_remove = Some(slot);
                                            }
                                        });
                                    }

                                    if let Some(slot) = to_remove {
                                        self.state
                                            .autoswitch
                                            .apps_for_profile_mut(profile_idx)
                                            .remove(slot);
                                        self.autoswitch_dirty = true;
                                    }

                                    // One compact add row instead of a wall
                                    // of empty capture slots.
                                    if apps.len() < max_apps {
                                        if cap_here {
                                            ui.label(
                                                egui::RichText::new(
                                                    "Waiting for you to click a window…",
                                                )
                                                .weak()
                                                .italics(),
                                            );
                                        } else if ui.button("＋ Add application").clicked() {
                                            let slot = self
                                                .state
                                                .autoswitch
                                                .apps_for_profile(profile_idx)
                                                .len();
                                            self.capturing_for = Some((profile_idx, slot));
                                            DeviceState::start_capture();
                                            self.set_status(
                                                format!(
                                                    "Click a window to add it to Mouse Profile {}…",
                                                    profile_idx + 1
                                                ),
                                                ctx,
                                            );
                                        }
                                    }
                                },
                                );
                            });

                            if let Some(nd) = new_default {
                                self.state.autoswitch.default_profile = nd;
                                self.autoswitch_dirty = true;
                            }
                        }
                    });
                    ui.add_space(8.0);
                }
            });
    }

    fn ui_autoswitch_save_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    self.autoswitch_dirty,
                    egui::Button::new("Save auto-switch settings"),
                )
                .clicked()
            {
                match self.state.save_autoswitch() {
                    Ok(()) => {
                        self.autoswitch_dirty = false;
                        // Tell the daemon to reload config. If the nudge
                        // fails the daemon still picks the file up via its
                        // mtime watch, but say so instead of staying quiet.
                        match DeviceState::notify_daemon_config_changed() {
                            Ok(()) => self.set_status("Auto-switch settings saved", ctx),
                            Err(e) => {
                                log::warn!("Saved, but could not notify daemon: {}", e);
                                self.set_status(
                                    "Saved - daemon unreachable, it will reload the file shortly",
                                    ctx,
                                );
                            }
                        }
                    }
                    Err(e) => self.set_status(format!("Error saving: {}", e), ctx),
                }
            }
            if ui
                .add_enabled(self.autoswitch_dirty, egui::Button::new("Revert"))
                .clicked()
            {
                self.state.autoswitch = roccat_nyth_profile::config::AutoSwitchConfig::load();
                self.autoswitch_dirty = false;
                self.set_status("Auto-switch settings reverted", ctx);
            }
            if self.autoswitch_dirty {
                ui.label(egui::RichText::new("● Unsaved changes").weak().italics());
            }
        });
    }
}

/// Truncate a string for display, adding "…" if too long.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}…", &s[..max_len])
    }
}
