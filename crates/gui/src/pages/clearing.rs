//! Cleaning progress page: program name, progress bar, ETA.

use database::utils::get_file_size_string;
use eframe::egui;

use crate::app::MyApp;

impl MyApp {
    pub(crate) fn render_clearing(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        //self.set_window_size(ctx, egui::Vec2::new(560.0, 100.0 + TITLE_BAR_HEIGHT));
        // Panel gives 8px, text adds 12px from the screen edges
        let program = self.state.current_program().to_string();
        let fraction = self.state.progress_fraction();
        let eta = self.state.eta();
        let cleaned_bytes = self.state.cleaned_bytes;

        ui.vertical(|ui| {
            ui.add_space(4.0);
            // Top left: name of the program currently being cleaned
            if !program.is_empty() {
                ui.horizontal(|ui| {
                    ui.add_space(4.0);
                    ui.strong(&program);
                });
            }
            ui.add_space(4.0);

            match fraction {
                Some(progress) => {
                    // Progress bar: 8px from the screen edges
                    ui.add_sized(
                        [ui.available_width(), 20.0],
                        egui::ProgressBar::new(progress)
                            .show_percentage()
                            .animate(false),
                    );
                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        ui.add_space(4.0);
                        // Bottom left: amount cleaned so far
                        ui.label(get_file_size_string(cleaned_bytes));
                        // Bottom right: remaining time
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                            if let Some(eta) = eta {
                                ui.label(eta);
                            }
                            ui.add_space(4.0);
                        });
                    });
                }
                // Before the first progress tick there is nothing to fill, so a
                // spinner is the honest indicator.
                None => {
                    ui.spinner();
                }
            }
        });
        // Keep polling the cleaning task / progress channel at a
        // modest rate instead of repainting on every message.
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}
