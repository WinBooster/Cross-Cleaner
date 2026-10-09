//! Cleaning results page: summary heading and virtualized results table.
//!
//! A row of the table is an aggregate — one program summed over every path it
//! touched — and an aggregate is the one number a user cannot check: the
//! question it provokes is *where did this go*. Clicking a program name opens
//! [`MyApp::render_results_details`] in the same window, which lists the paths
//! that program deleted and how much each of them freed, with a Back button
//! that returns to the report it came from.
//!
//! The paths themselves are stored segment-interned (see
//! [`database::structures::SharedPath`]), so a program that deleted ten thousand
//! files does not hold ten thousand copies of the directory they share.

use database::structures::{Cleared, ClearedPath};
use database::utils::get_file_size_string;
use eframe::egui;

use crate::app::MyApp;
use crate::title_bar::TITLE_BAR_HEIGHT;

/// Height of one deleted-path row.
///
/// Shared by the rows and the scroll area: the list is virtualized, and the
/// scroll area has to be told how tall a row is, so the two must be the same
/// number rather than two numbers that happen to match.
const DETAIL_ROW_HEIGHT: f32 = 21.0;

impl MyApp {
    /// Returns `false` when there is no result data yet (page should fall
    /// through to the main page), `true` after the table is drawn.
    pub(crate) fn render_results(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) -> bool {
        let Some((bytes, files, dirs, cleared)) = self.state.cleared_data.clone() else {
            return false;
        };

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

        // Row the user clicked, collected here and acted on after the scroll area:
        // the borrow of the report ends with the closure that reads it.
        let mut opened: Option<usize> = None;

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
                .on_hover_text("Program name — click to see what it removed");

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

                            // Program column. Clickable, because it is the one
                            // cell that can answer "where did this go?" — the
                            // rest are the numbers being questioned.
                            let program = ui
                                .add_sized(
                                    egui::vec2(column_widths[0], 20.0),
                                    egui::Label::new(&cleared.program)
                                        .truncate()
                                        .sense(egui::Sense::click()),
                                )
                                .on_hover_text("Show which paths were removed");
                            if program.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            if program.clicked() {
                                opened = Some(idx);
                            }

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

        if let Some(row) = opened {
            self.open_result_details(row);
        }

        true
    }

    /// The paths one row of the report deleted, and what each of them freed.
    ///
    /// Drawn in the same window as the report rather than in a popup or a second
    /// window: the run is the page, this is one level below it. There is no button
    /// of its own for the way back — the title bar's arrow is already on this window,
    /// and a second one saying the same thing is a control the user has to read twice
    /// to learn nothing new ([`MyApp::go_back`] is what both of them reach).
    ///
    /// Returns `false` when there is nothing to show, which drops the stale view
    /// and lets the caller fall back to the report.
    pub(crate) fn render_results_details(&mut self, ui: &mut egui::Ui) -> bool {
        let Some(row) = self.results_detail else {
            return false;
        };
        // An owned handle rather than a borrow: the run is behind an `Arc` (it
        // holds an entry per deleted path), and this view has to be able to
        // close itself while reading it.
        let Some(cleared) = self
            .state
            .cleared_data
            .as_ref()
            .map(|data| std::sync::Arc::clone(&data.3))
        else {
            self.close_result_details();
            return false;
        };
        let Some(entry) = cleared.get(row) else {
            // A second run replaced the report this view was pointing into.
            self.close_result_details();
            return false;
        };

        ui.heading(&entry.program);
        ui.separator();

        ui.label(format!(
            "Removed {} in {} · {} · {}",
            get_file_size_string(entry.removed_bytes),
            plural(entry.paths.len(), "path", "paths"),
            plural(entry.removed_files as usize, "file", "files"),
            plural(entry.removed_directories as usize, "folder", "folders"),
        ));
        ui.label(entry.affected_categories.join(", "));

        if entry.paths.is_empty() {
            // Never silently blank: a row that reported a total without saying
            // which path produced it has to say so.
            ui.separator();
            ui.label("This entry reported no deleted paths.");
            return true;
        }

        if entry.paths_omitted > 0 {
            // Said out loud, and above the list rather than inside it: a list that
            // stops early must not read as the whole of what was deleted.
            ui.separator();
            ui.label(format!(
                "{} more deleted {} are not listed.",
                entry.paths_omitted,
                plural(entry.paths_omitted, "path", "paths")
            ));
        }

        // Same fixed columns as the report, so the two read as one table seen
        // from two ends: the aggregate above, the entries behind it below.
        let widths = [340.0, 90.0, 80.0, 80.0];
        ui.horizontal(|ui| {
            ui.style_mut().spacing.item_spacing = egui::vec2(0.0, 0.0);
            for (label, width) in ["Path", "Size", "Files", "Dirs"].iter().zip(widths) {
                ui.add_sized(
                    egui::vec2(width, DETAIL_ROW_HEIGHT),
                    egui::Label::new(egui::RichText::new(*label).heading()),
                );
            }
        });
        ui.separator();

        // Virtualized like the report above it, and for the same reason: a program
        // can delete thousands of paths, and laying out all of them on every frame
        // is what makes a long list drag the whole window behind it. Only the rows
        // on screen are built, so the cost of the list is its height, not its length.
        egui::ScrollArea::vertical()
            .id_salt("result_details_scroll")
            .auto_shrink([false, false])
            .show_rows(ui, DETAIL_ROW_HEIGHT, entry.paths.len(), |ui, row_range| {
                for idx in row_range {
                    detail_row(ui, &entry.paths[idx], widths);
                }
            });

        true
    }

    /// Opens the deleted-path list of the report row at `row`.
    fn open_result_details(&mut self, row: usize) {
        if self.results_detail == Some(row) {
            // The same row twice is a toggle, so the way back is the way in.
            self.close_result_details();
            return;
        }
        self.results_detail = Some(row);
    }

    /// Returns from the deleted-path list to the report behind it.
    fn close_result_details(&mut self) {
        self.results_detail = None;
    }
}

/// One deleted path: what it was, and what it freed.
///
/// Fixed height, because the scroll area is told this exact number: a row that
/// measured itself would drift away from the count the virtualization uses.
fn detail_row(ui: &mut egui::Ui, detail: &ClearedPath, widths: [f32; 4]) {
    // Rebuilt from the interned segments, so the hover text can show the whole
    // path the truncated cell cannot.
    let path = detail.path.to_string();
    ui.horizontal(|ui| {
        ui.style_mut().spacing.item_spacing = egui::vec2(0.0, 0.0);
        ui.add_sized(
            egui::vec2(widths[0], DETAIL_ROW_HEIGHT),
            egui::Label::new(&path).truncate(),
        )
        .on_hover_text(path);
        ui.add_sized(
            egui::vec2(widths[1], 20.0),
            egui::Label::new(get_file_size_string(detail.removed_bytes)).truncate(),
        );
        ui.add_sized(
            egui::vec2(widths[2], 20.0),
            egui::Label::new(detail.removed_files.to_string()).truncate(),
        );
        ui.add_sized(
            egui::vec2(widths[3], 20.0),
            egui::Label::new(detail.removed_directories.to_string()).truncate(),
        );
    });
}

/// `one path` / `two paths`, from the count rather than by hand.
fn plural(count: usize, one: &'static str, many: &'static str) -> &'static str {
    if count == 1 { one } else { many }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The summary line has to read as English at both ends of the count, and a
    /// list that says "1 paths" is the kind of thing nobody notices until they
    /// do.
    #[test]
    fn the_count_is_pluralized() {
        assert_eq!(plural(0, "path", "paths"), "paths");
        assert_eq!(plural(1, "path", "paths"), "path");
        assert_eq!(plural(2, "path", "paths"), "paths");
        assert_eq!(plural(1, "file", "files"), "file");
    }
}
