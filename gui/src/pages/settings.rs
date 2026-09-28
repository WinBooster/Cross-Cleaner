//! Settings page: sound volume sliders.

use eframe::egui;

use crate::app::MyApp;
use crate::title_bar::TITLE_BAR_HEIGHT;

impl MyApp {
    pub(crate) fn render_settings(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        //self.set_window_size(ctx, egui::Vec2::new(500.0, 110.0 + TITLE_BAR_HEIGHT));

        let mut cfg = crate::config::get();
        let mut changed = false;

        ui.add_space(8.0);
        // Single column: one slider per row, so every label keeps enough
        // room for a full-width slider.
        ui.horizontal(|ui| {
            ui.label("Popup sound:");
            if ui
                .add(egui::Slider::new(&mut cfg.sound_volume, 0.0..=1.0).show_value(true))
                .changed()
            {
                changed = true;
            }
        });
        ui.horizontal(|ui| {
            ui.label("Click sound:");
            if ui
                .add(egui::Slider::new(&mut cfg.click_volume, 0.0..=1.0).show_value(true))
                .changed()
            {
                changed = true;
            }
        });
        ui.horizontal(|ui| {
            ui.label("Check sound:");
            if ui
                .add(egui::Slider::new(&mut cfg.check_volume, 0.0..=1.0).show_value(true))
                .changed()
            {
                changed = true;
            }
        });
        ui.horizontal(|ui| {
            ui.label("Done sound:");
            if ui
                .add(egui::Slider::new(&mut cfg.done_volume, 0.0..=1.0).show_value(true))
                .changed()
            {
                changed = true;
            }
        });

        if changed {
            crate::config::update(|c| *c = cfg);
        }
    }
}
