//! Settings page: sound volume sliders + auto-update toggle.

use eframe::egui;

use crate::app::MyApp;
use crate::title_bar::TITLE_BAR_HEIGHT;

impl MyApp {
    pub(crate) fn render_settings(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        #[cfg(windows)]
        let has_updater = {
            const CANDIDATES: &[&str] = &["updater.exe", "Windows-updater.exe"];
            std::env::current_exe()
                .ok()
                .and_then(|exe| exe.parent().map(|d| d.to_path_buf()))
                .is_some_and(|dir| CANDIDATES.iter().any(|n| dir.join(n).exists()))
        };
        #[cfg(not(windows))]
        let has_updater = false;

        let height = if has_updater { 110.0 } else { 70.0 } + TITLE_BAR_HEIGHT;
        self.set_window_size(ctx, egui::Vec2::new(500.0, height));

        let mut cfg = crate::config::get();
        let mut changed = false;

        ui.add_space(8.0);
        ui.columns(2, |columns| {
            columns[0].horizontal(|ui| {
                ui.label("Popup sound:");
                if ui
                    .add(egui::Slider::new(&mut cfg.volume.popup, 0.0..=1.0).show_value(true))
                    .changed()
                {
                    changed = true;
                }
            });
            columns[1].horizontal(|ui| {
                ui.label("Click sound:");
                if ui
                    .add(egui::Slider::new(&mut cfg.volume.click, 0.0..=1.0).show_value(true))
                    .changed()
                {
                    changed = true;
                }
            });
            columns[0].horizontal(|ui| {
                ui.label("Check sound:");
                if ui
                    .add(egui::Slider::new(&mut cfg.volume.checkbox, 0.0..=1.0).show_value(true))
                    .changed()
                {
                    changed = true;
                }
            });
            columns[1].horizontal(|ui| {
                ui.label("Done sound:");
                if ui
                    .add(egui::Slider::new(&mut cfg.volume.done, 0.0..=1.0).show_value(true))
                    .changed()
                {
                    changed = true;
                }
            });
        });

        if has_updater {
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);
            let mut auto = cfg.auto_update;
            if ui
                .checkbox(&mut auto, "Auto update (install silently when available)")
                .changed()
            {
                cfg.auto_update = auto;
                changed = true;
            }
        }

        if changed {
            crate::config::update(|c| *c = cfg);
        }
    }
}
