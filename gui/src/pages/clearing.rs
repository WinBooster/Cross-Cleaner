//! Cleaning progress page: program name, progress bar, ETA.

use database::utils::get_file_size_string;
use eframe::egui;

use crate::app::MyApp;
use crate::title_bar::TITLE_BAR_HEIGHT;

impl MyApp {
    pub(crate) fn render_clearing(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        self.set_window_size(ctx, egui::Vec2::new(560.0, 100.0 + TITLE_BAR_HEIGHT));
        // Panel gives 8px, text adds 12px from the screen edges
        ui.vertical(|ui| {
            ui.add_space(4.0);
            // Top left: name of the program currently being cleaned
            let program = self
                .progress_message
                .strip_prefix("Cleaning: ")
                .unwrap_or(&self.progress_message)
                .to_string();
            if !program.is_empty() {
                ui.horizontal(|ui| {
                    ui.add_space(4.0);
                    ui.strong(&program);
                });
            }
            ui.add_space(4.0);

            if self.total_tasks > 0 {
                let progress = self.current_task as f32 / self.total_tasks as f32;
                // Progress bar: 8px from the screen edges
                ui.add_sized(
                    [ui.available_width(), 20.0],
                    egui::ProgressBar::new(progress)
                        .show_percentage()
                        .animate(false),
                );
                ui.add_space(4.0);

                let eta = self.progress_start.and_then(|start| {
                    if self.current_task == 0 || self.current_task >= self.total_tasks {
                        None
                    } else {
                        let elapsed = start.elapsed().as_secs_f64();
                        let per_task = elapsed / self.current_task as f64;
                        let remaining = per_task * (self.total_tasks - self.current_task) as f64;
                        let mins = (remaining / 60.0).floor() as u64;
                        let secs = (remaining % 60.0).round() as u64;
                        if mins > 0 {
                            Some(format!("~{}m {:02}s", mins, secs))
                        } else {
                            Some(format!("~{}s", secs))
                        }
                    }
                });

                ui.horizontal(|ui| {
                    ui.add_space(4.0);
                    // Bottom left: amount cleaned so far
                    ui.label(get_file_size_string(self.cleaned_bytes));
                    // Bottom right: remaining time
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        if let Some(eta) = eta {
                            ui.label(eta);
                        }
                        ui.add_space(4.0);
                    });
                });
            } else {
                ui.spinner();
            }
        });
        // Keep polling the cleaning task / progress channel at a
        // modest rate instead of repainting on every message.
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}
