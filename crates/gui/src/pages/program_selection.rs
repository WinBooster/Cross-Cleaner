//! Program selection page: search, program checkboxes with per-program
//! category popups, and the Start Cleaning button.

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
                                                self.program_menu_button(ui, i, &menu_tex, true);
                                            },
                                        );
                                    } else {
                                        self.program_checkbox(ui, i);
                                        self.program_menu_button(ui, i, &menu_tex, false);
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
        // Owned copy: the popup closure needs `&mut self.state` to apply the
        // toggle, so it cannot hold a borrow of the category list.
        let Some(cats) = self.state.program_categories.get(i).cloned() else {
            return;
        };
        if cats.len() <= 1 {
            return;
        }
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
