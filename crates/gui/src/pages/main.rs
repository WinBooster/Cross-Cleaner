//! Main (home) page: category tristate checkboxes with subcategory popups and
//! the Next button that builds the program list.
//!
//! All selection logic is delegated to [`appcore::AppState`]; this module only
//! draws the widgets and plays the matching sounds.

use eframe::egui;
use std::sync::Arc;

use crate::app::{MyApp, Page};
use crate::categories::tristate_checkbox;
use crate::icons::{MENU_BYTES, load_asset_image};
use crate::sounds;
use crate::title_bar::TITLE_BAR_HEIGHT;

use super::{BUTTON_HEIGHT, split_list_and_button, ui_at_rect};

impl MyApp {
    pub(crate) fn render_main(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        // Responsive columns: 2 categories per row
        let columns_count = crate::category_columns(ctx);
        // Calculate dynamic window height based on number of categories.
        // Kept for when the fixed window size is re-enabled: the `set_window_size`
        // call below is still commented out, so the value is currently unused.
        let num_categories = self.state.categories.len();
        let rows = crate::category_rows(num_categories, columns_count);
        let row_height = 20.0; // Approximate height per row
        let base_height = 45.0; // Space for heading, margins, and button
        let dynamic_height = base_height + (rows as f32 * row_height);
        // Never taller than the screen: the category list scrolls, the
        // Next button is pinned to the bottom of the window.
        let max_height = crate::max_window_height(ctx);
        let _window_height = dynamic_height.clamp(20.0, max_height - TITLE_BAR_HEIGHT);

        // On Android window is fullscreen, don't enforce fixed size.
        #[cfg(not(target_os = "android"))]
        //self.set_window_size(
        //    ctx,
        //    egui::Vec2::new(560.0, _window_height + TITLE_BAR_HEIGHT),
        //);
        #[cfg(target_os = "android")]
        {
            let _ = (_window_height, TITLE_BAR_HEIGHT);
        }

        if self.menu_texture.is_none() {
            self.menu_texture = Some(ctx.load_texture(
                "menu",
                load_asset_image(MENU_BYTES),
                egui::TextureOptions::LINEAR,
            ));
        }
        let menu_tex = self.menu_texture.clone().unwrap();

        // Split the page into a scrolling list and a button that always
        // stays visible at the bottom, even in a very short window.
        let (list_rect, button_rect) = split_list_and_button(ui);

        ui_at_rect(ui, list_rect, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("categories_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.columns(columns_count, |columns| {
                        for idx in 0..self.state.categories.len() {
                            let column_index = idx % columns_count;
                            // The last column is laid out right-to-left so its
                            // checkboxes sit against the right window edge
                            // instead of floating in the middle. The menu button
                            // then ends up in front of the checkbox and is
                            // mirrored, so it still points at the checkbox.
                            let rightmost = column_index + 1 == columns_count;
                            columns[column_index].horizontal(|ui| {
                                if rightmost {
                                    // Right-to-left: the first widget added ends
                                    // up at the right edge.
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            self.category_checkbox(ui, idx);
                                            self.category_menu_button(ui, idx, &menu_tex, true);
                                        },
                                    );
                                } else {
                                    self.category_checkbox(ui, idx);
                                    self.category_menu_button(ui, idx, &menu_tex, false);
                                }
                            });
                        }
                    });
                });
        });

        ui_at_rect(ui, button_rect, |ui| {
            let available_width = ui.available_width();
            if ui
                .add_sized([available_width, BUTTON_HEIGHT], egui::Button::new("Next"))
                .clicked()
            {
                sounds::click();
                if self.state.build_program_list() {
                    self.state.current_page = Page::ProgramSelection;
                }
            }
        });
    }

    /// Tristate checkbox of a single category. The actual toggle is done by
    /// `AppState` so the terminal frontend behaves identically.
    fn category_checkbox(&mut self, ui: &mut egui::Ui, idx: usize) {
        let is_checked = self.state.categories[idx].is_checked();
        let is_indet = self.state.categories[idx].is_indeterminate();
        let label = self.state.category_label(idx).to_string();
        let (_resp, clicked) = tristate_checkbox(ui, is_checked, is_indet, &label);
        if clicked && self.state.toggle_category(idx).is_on() {
            sounds::check();
        } else if clicked {
            sounds::uncheck();
        }
    }

    /// Menu button that opens the per-category subcategory popup. Drawn only
    /// for categories that actually have subcategories (embedded menu.png).
    /// `mirrored` flips the icon horizontally, which is what the right-aligned
    /// column needs so the icon still visually points at its checkbox.
    fn category_menu_button(
        &mut self,
        ui: &mut egui::Ui,
        idx: usize,
        menu_tex: &egui::TextureHandle,
        mirrored: bool,
    ) {
        let category = &self.state.categories[idx];
        if category.subs.is_empty() {
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

        // Popup with sub_category checkboxes
        let frame = egui::Frame::popup(ui.style());
        egui::Popup::menu(&menu_resp)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .frame(frame)
            .show(|ui| {
                ui.set_min_width(200.0);
                egui::ScrollArea::vertical()
                    .max_height(300.0)
                    .show(ui, |ui| {
                        // Snapshot the entries: the closure needs `&mut self`
                        // to apply the toggle, so it cannot hold a borrow of
                        // the category it is iterating.
                        let category_name = self.state.categories[idx].name.clone();
                        let subs = self.state.categories[idx].subs.clone();
                        let has_empty = self.state.categories[idx].has_empty;

                        for sub in subs {
                            let label = self.state.sub_label(&category_name, &sub);
                            let mut is_sel = self.state.categories[idx].selected.contains(&sub);
                            if ui.checkbox(&mut is_sel, &label).changed() {
                                let toggle = self.state.toggle_category_sub(idx, &sub);
                                if toggle.is_on() {
                                    sounds::check();
                                } else {
                                    sounds::uncheck();
                                }
                            }
                        }
                        // Show Uncategorized for objects without sub_category, only if
                        // category has >= 1 real sub
                        if has_empty {
                            let empty: Arc<str> = Arc::from("");
                            let label = self.state.sub_label(&category_name, &empty);
                            let mut is_uncat = self.state.categories[idx].selected.contains("");
                            if ui.checkbox(&mut is_uncat, &label).changed() {
                                let toggle = self.state.toggle_category_sub(idx, &empty);
                                if toggle.is_on() {
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
