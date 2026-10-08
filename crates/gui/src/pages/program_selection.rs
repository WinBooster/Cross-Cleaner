//! Program selection page: search, program checkboxes with per-program
//! category popups, and the Start Cleaning button.
//!
//! After each checkbox comes what the program belongs to: the category name as
//! plain text when it is in exactly one, and a menu button when it is in
//! several. The terminal frontend draws the same split as name versus count.

use eframe::egui;

use crate::app::MyApp;
use crate::categories::tristate_checkbox;
use crate::icons::{MENU_BYTES, load_asset_image};
use crate::sounds;
use crate::title_bar::TITLE_BAR_HEIGHT;

use super::{BUTTON_HEIGHT, split_list_and_button, ui_at_rect};

impl MyApp {
    pub(crate) fn render_program_selection(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        // Dynamic window sizing based on number of (filtered) programs.
        // Kept for when the fixed window size is re-enabled: the
        // `set_window_size` call below is still commented out.
        let num_programs = self.state.filtered_programs.len();
        let rows = num_programs.div_ceil(2); // 2 columns
        let row_height = 20.0;
        let base_height = 120.0; // Heading, search, buttons, separators
        let min_scroll_height = 20.0;
        let max_scroll_height = 400.0;

        let content_height = rows as f32 * row_height;
        let scroll_height = content_height.min(max_scroll_height).max(min_scroll_height);
        // Never taller than the screen: the program list scrolls and the
        // Start Cleaning button stays pinned to the bottom of the window.
        let _window_height =
            (base_height + scroll_height).min(crate::max_window_height(ctx) - TITLE_BAR_HEIGHT);

        //self.set_window_size(
        //    ctx,
        //    egui::Vec2::new(500.0, _window_height + TITLE_BAR_HEIGHT),
        //);

        ui.vertical_centered(|ui| {
            ui.heading("Select Programs to Clean");
        });
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Search:");
            let available_width = ui.available_width();
            let search_response = ui.add_sized(
                [available_width, 20.0],
                egui::TextEdit::singleline(&mut self.state.search_query_visible),
            );
            if search_response.changed() {
                // Clone first: `set_search` needs `&mut self.state`, so the
                // visible text cannot be borrowed at the same time.
                let query = self.state.search_query_visible.clone();
                self.state.set_search(&query);
            }
        });

        // The list scrolls inside the leftover space, the Start Cleaning
        // button below it is pinned to the bottom and never scrolls away.
        let (list_rect, button_rect) = split_list_and_button(ui);

        if self.menu_texture.is_none() {
            self.menu_texture = Some(ctx.load_texture(
                "menu",
                load_asset_image(MENU_BYTES),
                egui::TextureOptions::LINEAR,
            ));
        }
        let menu_tex = self.menu_texture.clone().unwrap();

        // Only lay out the rows that are actually visible; the
        // program list can be huge, and building every checkbox
        // (plus its category popup) each frame is very expensive.
        let total_rows = self.state.filtered_programs.len().div_ceil(2);
        ui_at_rect(ui, list_rect, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("programs_scroll")
                .auto_shrink([false, false])
                .max_height(list_rect.height())
                .show_rows(ui, row_height, total_rows, |ui, row_range| {
                    for row in row_range {
                        let num_columns = 2;
                        ui.columns(num_columns, |columns| {
                            for (col, column) in columns.iter_mut().enumerate().take(num_columns) {
                                let Some(&i) = self.state.filtered_programs.get(row * 2 + col)
                                else {
                                    break;
                                };
                                // The last column is laid out right-to-left so
                                // its checkboxes sit against the right window
                                // edge, same as the category grid on the main
                                // page. The menu button ends up in front of the
                                // checkbox there and is mirrored, so it still
                                // points at it.
                                let rightmost = col + 1 == num_columns;
                                column.horizontal(|ui| {
                                    if rightmost {
                                        // Right-to-left: the first widget added
                                        // ends up at the right edge.
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                self.program_checkbox(ui, i);
                                                self.program_category_control(
                                                    ui, i, &menu_tex, true,
                                                );
                                            },
                                        );
                                    } else {
                                        self.program_checkbox(ui, i);
                                        self.program_category_control(ui, i, &menu_tex, false);
                                    }
                                });
                            }
                        });
                    }
                });
        });

        ui_at_rect(ui, button_rect, |ui| {
            let available_width = ui.available_width();
            if ui
                .add_sized(
                    [available_width, BUTTON_HEIGHT],
                    egui::Button::new("Start Cleaning"),
                )
                .clicked()
            {
                sounds::click();
                self.state.start_cleaning();
            }
        });
    }

    /// Tristate checkbox of a single program in the selection list.
    fn program_checkbox(&mut self, ui: &mut egui::Ui, i: usize) {
        let is_checked = self.state.is_program_checked(i);
        let is_indet = self.state.is_program_indeterminate(i);
        let program = self.state.program_checkboxes[i].1.to_string();
        let (_resp, clicked) = tristate_checkbox(ui, is_checked, is_indet, &program);
        if clicked && self.state.toggle_program(i).is_on() {
            sounds::check();
        } else if clicked {
            sounds::uncheck();
        }
    }

    /// What a program row says about its categories, after the checkbox.
    ///
    /// Which control that is depends on how many categories the program has, and
    /// the terminal frontend applies the same rule to the marker it draws after
    /// the program name:
    ///
    /// * one category — the category's *name* as plain text, so the row says
    ///   where the program lives without anything having to be opened;
    /// * several — the menu button, because there is something to choose and the
    ///   count of what there is does not fit on a row.
    fn program_category_control(
        &mut self,
        ui: &mut egui::Ui,
        i: usize,
        menu_tex: &egui::TextureHandle,
        mirrored: bool,
    ) {
        match self.state.program_categories.get(i).map(Vec::len) {
            Some(0) | None => {}
            // Plain text, and deliberately not a button: there is nothing to
            // choose from, so a control that opened a one-item menu would be
            // worse than no control at all. The arrow the terminal frontend
            // draws is kept so both frontends read the same, and it points away
            // from the name in the mirrored column.
            Some(1) => {
                // Copied out rather than borrowed: the label is drawn right here,
                // but the `Some(_)` arm below needs `&mut self`, and holding the
                // borrow across the match would tie the two together.
                let Some(name) = self
                    .state
                    .program_categories
                    .get(i)
                    .and_then(|cats| cats.first())
                    .map(|cat| cat.to_string())
                else {
                    return;
                };
                ui.horizontal(|ui| {
                    arrow(ui, mirrored);
                    ui.label(egui::RichText::new(name).weak());
                });
            }
            Some(_) => self.program_menu_button(ui, i, menu_tex, mirrored),
        }
    }

    /// Menu button that opens the per-program category popup. Drawn only for
    /// programs that belong to more than one category. `mirrored` flips the
    /// icon horizontally, which is what the right-aligned column needs so the
    /// icon still visually points at its checkbox.
    fn program_menu_button(
        &mut self,
        ui: &mut egui::Ui,
        i: usize,
        menu_tex: &egui::TextureHandle,
        mirrored: bool,
    ) {
        // Owned copy: the popup closure needs `&mut self.state` to apply a
        // toggle, so it cannot hold a borrow of the category list.
        let Some(cats) = self.state.program_categories.get(i).cloned() else {
            return;
        };
        let menu_image = egui::Image::from_texture(egui::load::SizedTexture::new(
            menu_tex.id(),
            menu_tex.size_vec2(),
        ))
        .fit_to_exact_size(egui::vec2(16.0, 16.0))
        .uv(if mirrored {
            // Reversed u range mirrors the image horizontally.
            egui::Rect::from_min_max(egui::pos2(1.0, 0.0), egui::pos2(0.0, 1.0))
        } else {
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0))
        })
        .tint(ui.visuals().text_color())
        .sense(egui::Sense::click());
        let menu_resp = ui.add_sized(egui::vec2(16.0, 16.0), menu_image);
        if menu_resp.clicked() {
            sounds::pop();
        }

        // Popup: disable individual categories for this program only.
        let frame = egui::Frame::popup(ui.style());
        egui::Popup::menu(&menu_resp)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .frame(frame)
            .show(|ui| {
                ui.set_min_width(180.0);
                egui::ScrollArea::vertical()
                    .max_height(300.0)
                    .show(ui, |ui| {
                        for cat in cats {
                            let mut enabled = !self.state.program_disabled[i].contains(&cat);
                            if ui.checkbox(&mut enabled, &*cat).changed() {
                                // The checkbox inverts the flag, so an "enabled"
                                // turn-off means the category got excluded.
                                if self.state.toggle_program_category(i, &cat).is_on() {
                                    sounds::check();
                                } else {
                                    sounds::uncheck();
                                }
                            }
                        }
                    });
            });
    }
}

/// Draws a right-pointing arrow, or a left-pointing one when `mirrored`.
///
/// Painted rather than typeset: egui's built-in fonts carry no arrow glyph, so a
/// `→` in a label renders as a blank box or nothing at all. Three line segments
/// draw it in the UI's own text color at any size, which is also how the title
/// bar draws its close and minimize marks — the same reasoning, the same way
/// around.
fn arrow(ui: &mut egui::Ui, mirrored: bool) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ARROW_WIDTH, ARROW_HEIGHT), egui::Sense::hover());
    let stroke = egui::Stroke::new(1.2, ui.visuals().weak_text_color());
    for [from, to] in arrow_lines(rect, mirrored) {
        ui.painter().line_segment([from, to], stroke);
    }
}

/// Width of [`arrow`] in points.
const ARROW_WIDTH: f32 = 10.0;
/// Height of [`arrow`] in points, matching a line of text.
const ARROW_HEIGHT: f32 = 12.0;

/// One line segment of [`arrow`].
type Segment = [egui::Pos2; 2];

/// The three lines that draw [`arrow`] inside `rect`: the shaft, then the two
/// barbs meeting at the tip.
///
/// Kept apart from the painting so the shape can be asserted in a test. The barbs
/// have to run *backwards* from the tip, towards the tail — drawn forwards, as an
/// earlier version of this did, the result is a fork and not an arrow, and the
/// mistake is invisible in a screenshot of a row of labels.
fn arrow_lines(rect: egui::Rect, mirrored: bool) -> [Segment; 3] {
    let tip = if mirrored { rect.left() } else { rect.right() };
    let tail = if mirrored { rect.right() } else { rect.left() };
    let mid_y = rect.center().y;
    // How far the barbs reach from the tip, sideways. A third of the height keeps
    // the head open enough to read as an arrow at this size.
    let head = rect.height() * 0.3;
    // +1 towards the tail when the arrow points left, -1 when it points right.
    let towards_tail = if mirrored { 1.0 } else { -1.0 };
    let barb = egui::pos2(tip + towards_tail * head, mid_y);
    [
        // The shaft, tail to tip.
        [egui::pos2(tail, mid_y), egui::pos2(tip, mid_y)],
        [egui::pos2(barb.x, mid_y - head), egui::pos2(tip, mid_y)],
        [egui::pos2(barb.x, mid_y + head), egui::pos2(tip, mid_y)],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(ARROW_WIDTH, ARROW_HEIGHT))
    }

    /// The bug this pins: the barbs were drawn *forward* of the tip, which turns
    /// the arrow into a fork. Nothing in a screenshot of a list of program names
    /// would make that obvious, and the drawing code is not reviewable by eye.
    #[test]
    fn the_barbs_run_back_from_the_tip_towards_the_tail() {
        for mirrored in [false, true] {
            let rect = rect();
            let lines = arrow_lines(rect, mirrored);
            let [shaft, upper, lower] = lines;

            let (tip_x, tail_x) = if mirrored {
                (rect.left(), rect.right())
            } else {
                (rect.right(), rect.left())
            };

            // The shaft spans the whole width, tail to tip.
            assert_eq!(shaft[0].x, tail_x, "the shaft starts at the tail");
            assert_eq!(shaft[1].x, tip_x, "and ends at the tip");

            for barb in [upper, lower] {
                // Both barbs meet at the tip...
                assert_eq!(barb[1].x, tip_x, "a barb meets the tip");
                // ...and reach back towards the tail, never past it.
                let reached = barb[0].x;
                if mirrored {
                    assert!(reached > tip_x, "left arrow: barbs reach right");
                    assert!(reached <= tail_x, "left arrow: and no further");
                } else {
                    assert!(reached < tip_x, "right arrow: barbs reach left");
                    assert!(reached >= tail_x, "right arrow: and no further");
                }
            }

            // The two barbs open away from each other, so the head is a `>` and
            // not a `V`.
            assert!(upper[0].y < lower[0].y, "the head opens around the shaft",);
        }
    }

    /// Mirroring must be a real flip: same shape, pointing the other way.
    #[test]
    fn mirroring_only_flips_the_arrow() {
        let rect = rect();
        let right = arrow_lines(rect, false);
        let left = arrow_lines(rect, true);

        for ([r_from, r_to], [l_from, l_to]) in right.iter().zip(left.iter()) {
            // Reflected about the vertical centre line, the two must coincide.
            let centre = rect.center().x;
            assert!((r_from.x + l_from.x - 2.0 * centre).abs() < f32::EPSILON);
            assert!((r_to.x + l_to.x - 2.0 * centre).abs() < f32::EPSILON);
            assert_eq!(r_from.y, l_from.y);
            assert_eq!(r_to.y, l_to.y);
        }
    }
}
