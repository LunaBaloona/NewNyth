//! NewNyth egui application

use eframe::egui;

use roccat_nyth_hid::display;
use roccat_nyth_hid::protocol::*;

use crate::state::{hid_key_name, DeviceState, ASSIGNABLE_BUTTON_TYPES, HID_KEYS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Settings,
    Buttons,
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
    /// When capturing: (profile_index, slot_index) to assign the next captured app to.
    capturing_for: Option<(u8, usize)>,
    /// Last time the active-profile byte was re-read (indicator refresh poll).
    last_profile_poll: std::time::Instant,
}

impl NythConfigApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        crate::theme::apply(&cc.egui_ctx);

        let state = DeviceState::load_from_device();
        let selected = state.active_profile;
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
            capturing_for: None,
            last_profile_poll: std::time::Instant::now(),
        }
    }

    fn set_status(&mut self, msg: impl Into<String>, ctx: &egui::Context) {
        self.status_message = Some((msg.into(), ctx.input(|i| i.time)));
    }

    fn reload_device(&mut self, ctx: &egui::Context) {
        self.state = DeviceState::load_from_device();
        self.load_pending_from_profile();
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
                        let r = ui.add_sized([30.0, 26.0], btn);
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
                    if let Some((ref msg, ts)) = self.status_message {
                        if ui.input(|i| i.time) - ts < 5.0 {
                            ui.label(egui::RichText::new(msg).weak().italics());
                        }
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
            match self.section {
                Section::Settings => self.ui_settings(ui, &ctx),
                Section::Buttons => self.ui_buttons(ui, &ctx),
                Section::Lighting => self.ui_lighting(ui, &ctx),
                Section::AutoSwitch => self.ui_autoswitch(ui, &ctx),
            }
        });
    }
}

// ─── Settings panel (unchanged from v0.3.0) ────────────────────────

impl NythConfigApp {
    fn ui_settings(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let settings = match self.pending_settings.as_mut() {
            Some(s) => s as *mut ProfileSettings,
            None => {
                ui.label("No settings available.");
                return;
            }
        };
        let settings = unsafe { &mut *settings };

        ui.heading(format!("Profile {} — Settings", self.selected_profile + 1));
        ui.add_space(8.0);

        // Single stacked column, contents centred (Swarm keeps Settings
        // narrow and vertical rather than spreading over two columns).
        crate::theme::card(ui, "CPI levels", |ui| {
            ui.vertical_centered(|ui| {
                for i in 0..NYTH_CPI_LEVELS_NUM {
                    ui.horizontal(|ui| {
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
                        ui.label(label);
                        if ui
                            .add(
                                egui::Slider::new(
                                    &mut cpi,
                                    NYTH_CPI_MIN as f32..=NYTH_CPI_MAX as f32,
                                )
                                .step_by(NYTH_CPI_STEP as f64)
                                .suffix(" DPI")
                                .clamping(egui::SliderClamping::Always),
                            )
                            .changed()
                        {
                            settings.set_cpi_level(i, cpi as u16);
                            self.settings_dirty = true;
                        }
                    });
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
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
        crate::theme::card(ui, "Polling rate", |ui| {
            ui.vertical_centered(|ui| {
                ui.horizontal(|ui| {
                    for (v, l) in [
                        (1u8, "125 Hz"),
                        (2, "250 Hz"),
                        (3, "500 Hz"),
                        (4, "1000 Hz"),
                    ] {
                        if ui.selectable_label(settings.polling_rate == v, l).clicked()
                            && settings.polling_rate != v
                        {
                            settings.polling_rate = v;
                            self.settings_dirty = true;
                        }
                    }
                });
            });
        });

        ui.add_space(8.0);
        crate::theme::card(ui, "Sensitivity", |ui| {
            ui.vertical_centered(|ui| {
                let mut adv = settings.advanced_sensitivity != 0;
                if ui.checkbox(&mut adv, "Independent X / Y").changed() {
                    settings.advanced_sensitivity = if adv { 1 } else { 0 };
                    if !adv {
                        settings.sensitivity_y = settings.sensitivity_x;
                    }
                    self.settings_dirty = true;
                }
                let mut sx = settings.sensitivity_x as f32;
                if ui
                    .add(egui::Slider::new(&mut sx, 1.0..=11.0).text("X sensitivity"))
                    .changed()
                {
                    settings.sensitivity_x = sx as u8;
                    if !adv {
                        settings.sensitivity_y = sx as u8;
                    }
                    self.settings_dirty = true;
                }
                if adv {
                    let mut sy = settings.sensitivity_y as f32;
                    if ui
                        .add(egui::Slider::new(&mut sy, 1.0..=11.0).text("Y sensitivity"))
                        .changed()
                    {
                        settings.sensitivity_y = sy as u8;
                        self.settings_dirty = true;
                    }
                }
            });
        });

        ui.add_space(12.0);
        self.apply_revert_bar(ui, ctx, true);
    }

    // ─── Lighting panel (unchanged) ────────────────────────────

    fn ui_lighting(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let settings = match self.pending_settings.as_mut() {
            Some(s) => s as *mut ProfileSettings,
            None => {
                ui.label("No settings available.");
                return;
            }
        };
        let settings = unsafe { &mut *settings };

        ui.heading(format!("Profile {} — Lighting", self.selected_profile + 1));
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
                ui.vertical_centered(|ui| {
                    for (i, zone) in ["Logo", "Bottom"].iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(format!("{zone}:"));
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
                            ui.label(format!("#{:02x}{:02x}{:02x}", l.red, l.green, l.blue));
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
                    ui.horizontal(|ui| {
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
                        if ui
                            .add(egui::Slider::new(&mut spd, 1.0..=3.0).text("Speed"))
                            .changed()
                        {
                            settings.effect_speed = spd as u8;
                            self.settings_dirty = true;
                        }
                    }
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
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

    fn ui_buttons(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading(format!("Profile {} — Buttons", self.selected_profile + 1));
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

        egui::ScrollArea::vertical()
            .max_height(ui.available_height() - 120.0)
            .show(ui, |ui| {
                // Centre the quadrant block. When the mouse illustration lands
                // (v0.9.2) it will take the side space this frees up.
                let avail_w = ui.available_width();
                let block_w = avail_w.min(1000.0);
                let pad = ((avail_w - block_w) / 2.0).max(0.0);
                ui.horizontal(|ui| {
                    ui.add_space(pad);
                    ui.vertical(|ui| {
                        ui.set_width(block_w);
                        ui.columns(2, |cols| {
                            // ── Left column: Standard layer ─────
                            crate::theme::card(&mut cols[0], "Standard — Main buttons", |ui| {
                                self.button_grid(ui, buttons, 0..8, "btn_std_main", 0);
                            });
                            cols[0].add_space(6.0);
                            crate::theme::card(&mut cols[0], "Standard — Side buttons", |ui| {
                                self.button_grid(ui, buttons, 8..20, "btn_std_side", 4);
                            });

                            // ── Right column: EasyShift layer ───
                            crate::theme::card(&mut cols[1], "EasyShift — Main buttons", |ui| {
                                self.button_grid(ui, buttons, 20..28, "btn_es_main", 0);
                            });
                            cols[1].add_space(6.0);
                            crate::theme::card(&mut cols[1], "EasyShift — Side buttons", |ui| {
                                self.button_grid(ui, buttons, 28..40, "btn_es_side", 4);
                            });
                        });
                    });
                });
            });

        ui.add_space(4.0);
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
    ) {
        egui::Grid::new(grid_id)
            .num_columns(2)
            .spacing([8.0, 2.0])
            .striped(true)
            .show(ui, |ui| {
                let start = range.start;
                for i in range {
                    // Insert separator between groups of N.
                    let pos_in_range = i - start;
                    if group_every > 0
                        && pos_in_range > 0
                        && pos_in_range.is_multiple_of(group_every)
                    {
                        ui.separator();
                        ui.separator();
                        ui.end_row();
                    }
                    let is_selected = self.selected_button == Some(i);
                    let btn = &mut buttons.buttons[i];
                    let name = if i >= NYTH_PHYSICAL_BUTTON_NUM {
                        display::BUTTON_NAMES[i]
                            .strip_prefix("Shift+")
                            .unwrap_or(display::BUTTON_NAMES[i])
                    } else {
                        display::BUTTON_NAMES[i]
                    };

                    // Button name (clickable to select)
                    let name_label = if is_selected {
                        egui::RichText::new(name).strong()
                    } else {
                        egui::RichText::new(name)
                    };
                    if ui
                        .add(egui::Label::new(name_label).sense(egui::Sense::click()))
                        .clicked()
                    {
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
        let per_col = (NYTH_PROFILE_NUM as usize).div_ceil(num_cols);

        egui::ScrollArea::vertical()
            .id_salt("autoswitch_main_scroll")
            .show(ui, |ui| {
                ui.columns(num_cols, |cols| {
                    for (col_idx, col) in cols.iter_mut().enumerate() {
                        let start = col_idx * per_col;
                        let end = (start + per_col).min(NYTH_PROFILE_NUM as usize);
                        for p in start..end {
                            let profile_idx = p as u8;
                            let is_default =
                                self.state.autoswitch.default_profile == Some(profile_idx);
                            let cap_here =
                                self.capturing_for.map(|(pp, _)| pp) == Some(profile_idx);

                            // The header's "Default" toggle must not borrow
                            // `self` (the body closure already does), so it
                            // parks the desired change for after the card.
                            let mut new_default: Option<Option<u8>> = None;

                            crate::theme::card_with_actions(
                                col,
                                &format!("Mouse Profile {}", profile_idx + 1),
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

                            if let Some(nd) = new_default {
                                self.state.autoswitch.default_profile = nd;
                                self.autoswitch_dirty = true;
                            }
                            col.add_space(8.0);
                        }
                    }
                });
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
                                    "Saved — daemon unreachable, it will reload the file shortly",
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
