//! Cleaning results page: summary heading and virtualized results table.

use database::structures::Cleared;
use database::utils::get_file_size_string;
use eframe::egui;

use crate::app::MyApp;
use crate::title_bar::TITLE_BAR_HEIGHT;

impl MyApp {
    /// Returns `false` when there is no result data yet (page should fall
    /// through to the main page), `true` after the table is drawn.
    pub(crate) fn render_results(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) -> bool {
        if self.cleared_data.is_none() {
            return false;
        }
        // Take ownership so `self` is free for `results_window_resized` etc.
        let (bytes, files, dirs, cleared) = self.cleared_data.clone().expect("checked above");

        ui.vertical_centered(|ui| {
            ui.heading("Cleaning Results");
            ui.heading(format!(
                "Size: {}, Files: {}, Dirs: {}",
                get_file_size_string(bytes),
                files,
                dirs
            ));
        });
        ui.separator();

        // Fixed column widths
        let column_widths = [150.0, 80.0, 80.0, 170.0];
        let total_width = column_widths.iter().sum::<f32>() + 120.0;
        let total_height = 500.0;

        // Resize window only once when results are first shown
        if !self.results_window_resized {
            let size = egui::Vec2::new(total_width, total_height + TITLE_BAR_HEIGHT);
            if self.last_inner_size != Some(size) {
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
                self.last_inner_size = Some(size);
            }
            self.results_window_resized = true;
        }

        // Outer container for the table
        ui.vertical(|ui| {
            // Table headers
            ui.horizontal(|ui| {
                ui.style_mut().spacing.item_spacing = egui::vec2(0.0, 0.0);

                // Program column
                ui.add_sized(
                    egui::vec2(column_widths[0], 20.0),
                    egui::Label::new(egui::RichText::new("Program").heading()),
                )
                .on_hover_text("Program name");

                // Size column
                ui.add_sized(
                    egui::vec2(column_widths[1], 20.0),
                    egui::Label::new(egui::RichText::new("Size").heading()),
                )
                .on_hover_text("Deleted data size");

                // Files column
                ui.add_sized(
                    egui::vec2(column_widths[2], 20.0),
                    egui::Label::new(egui::RichText::new("Files").heading()),
                )
                .on_hover_text("Number of files");

                // Dirs column
                ui.add_sized(
                    egui::vec2(column_widths[2], 20.0),
                    egui::Label::new(egui::RichText::new("Dirs").heading()),
                )
                .on_hover_text("Number of folders");

                // Categories column
                ui.add_sized(
                    egui::vec2(column_widths[3], 20.0),
                    egui::Label::new(egui::RichText::new("Categories").heading()),
                )
                .on_hover_text("Data categories");
            });
            ui.separator();

            // Scrollable, virtualized table content: only the
            // visible rows are laid out each frame.
            egui::ScrollArea::vertical()
                .max_height(total_height)
                .show_rows(ui, 21.0, cleared.len(), |ui, row_range| {
                    let content_right = ui.max_rect().right();
                    for idx in row_range {
                        let cleared: &Cleared = &cleared[idx];
                        let row = ui.horizontal(|ui| {
                            ui.style_mut().spacing.item_spacing = egui::vec2(0.0, 0.0);

                            // Program column
                            ui.add_sized(
                                egui::vec2(column_widths[0], 20.0),
                                egui::Label::new(&cleared.program).truncate(),
                            );

                            // Size column
                            ui.add_sized(
                                egui::vec2(column_widths[1], 20.0),
                                egui::Label::new(get_file_size_string(cleared.removed_bytes))
                                    .truncate(),
                            );

                            // Files column
                            ui.add_sized(
                                egui::vec2(column_widths[2], 20.0),
                                egui::Label::new(cleared.removed_files.to_string()).truncate(),
                            );

                            // Dirs column
                            ui.add_sized(
                                egui::vec2(column_widths[2], 20.0),
                                egui::Label::new(cleared.removed_directories.to_string())
                                    .truncate(),
                            );

                            // Categories column
                            ui.add_sized(
                                egui::vec2(column_widths[3], 20.0),
                                egui::Label::new(cleared.affected_categories.join(", ")).wrap(),
                            );
                        });
                        // Row separator, painted instead of a
                        // `ui.separator()` so it does not add
                        // height and break row virtualization.
                        let rect = row.response.rect;
                        ui.painter().hline(
                            rect.min.x..=content_right.max(rect.max.x),
                            rect.bottom(),
                            ui.visuals().widgets.noninteractive.bg_stroke,
                        );
                    }
                });
        });

        true
    }
}
