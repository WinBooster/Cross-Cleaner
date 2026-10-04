//! Main (home) page: category tristate checkboxes with subcategory
//! popups and the Next button that builds the program list.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;

use eframe::egui;

use crate::app::{MyApp, Page};
use crate::categories::{effective_sub, tristate_checkbox};
use crate::icons::{MENU_BYTES, load_asset_image};
use crate::sounds;
use crate::title_bar::TITLE_BAR_HEIGHT;

use super::{BUTTON_HEIGHT, split_list_and_button, ui_at_rect};

impl MyApp {
    pub(crate) fn render_main(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        // Responsive columns: 2 categories per row
        let columns_count = crate::category_columns(ctx);
        // Calculate dynamic window height based on number of categories
        let num_categories = self.categories.len();
        let rows = crate::category_rows(num_categories, columns_count);
        let row_height = 20.0; // Approximate height per row
        let base_height = 45.0; // Space for heading, margins, and button
        let dynamic_height = base_height + (rows as f32 * row_height);
        // Never taller than the screen: the category list scrolls, the
        // Next button is pinned to the bottom of the window.
        let max_height = crate::max_window_height(ctx);
        let window_height = dynamic_height.clamp(20.0, max_height - TITLE_BAR_HEIGHT);

        // On Android window is fullscreen, don't enforce fixed size
        #[cfg(not(target_os = "android"))]
        //self.set_window_size(
        //    ctx,
        //    egui::Vec2::new(560.0, window_height + TITLE_BAR_HEIGHT),
        //);
        #[cfg(target_os = "android")]
        {
            let _ = (window_height, TITLE_BAR_HEIGHT);
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
                        for idx in 0..self.categories.len() {
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
                if self.build_program_list() {
                    self.current_page = Page::ProgramSelection;
                }
            }
        });
    }

    /// Turns the current category selection into the program list shown by the
    /// program-selection page. Returns `false` when nothing is selected.
    pub(crate) fn build_program_list(&mut self) -> bool {
        if !self.has_selection() {
            return false;
        }
        let selected_map = self.selected_map();
        let mut programs: Vec<(Arc<str>, Vec<Arc<str>>)> = Vec::new();
        let mut add = |program: Arc<str>, category: Arc<str>| {
            if let Some(entry) = programs
                .iter_mut()
                .find(|(p, _)| p.as_ref() == program.as_ref())
            {
                if !entry.1.iter().any(|c| c.as_ref() == category.as_ref()) {
                    entry.1.push(category);
                }
            } else {
                programs.push((program, vec![category]));
            }
        };
        let _ = self.database.for_each_index(|data| {
            let eff = effective_sub("", &data.sub_category);
            if let Some(subs) = selected_map.get(data.category.as_ref())
                && subs.contains(&eff)
            {
                add(Arc::clone(&data.program), Arc::clone(&data.category));
            }
        });
        for data in self.custom_database.iter() {
            let eff = effective_sub("", &data.sub_category);
            if let Some(subs) = selected_map.get(data.category.as_ref())
                && subs.contains(&eff)
            {
                add(Arc::clone(&data.program), Arc::clone(&data.category));
            }
        }
        #[cfg(windows)]
        {
            let _ = self.regisry_database.for_each_index(|data| {
                let eff = effective_sub("", &data.sub_category);
                if let Some(subs) = selected_map.get(data.category.as_ref())
                    && subs.contains(&eff)
                {
                    add(Arc::clone(&data.program), Arc::clone(&data.category));
                }
            });
        }
        programs.sort_by(|a, b| a.0.cmp(&b.0));
        for (_, cats) in programs.iter_mut() {
            cats.sort();
        }

        self.program_checkboxes.clear();
        self.program_categories.clear();
        self.program_disabled.clear();
        for (program, cats) in programs {
            self.program_checkboxes
                .push((Rc::new(RefCell::new(true)), program));
            self.program_categories.push(cats);
            self.program_disabled.push(HashSet::new());
        }

        self.rebuild_filtered_programs();
        true
    }

    /// Starts a cleaning run over *every* category without going through the page
    /// flow, which is what the loaded build needs when its hotkey should clean in
    /// one step.
    ///
    /// Returns `false` when a run is already in progress or the database has no
    /// programs to clean, so the caller can tell a no-op from a started run.
    pub fn quick_clean_all(&mut self) -> bool {
        if self.current_page == Page::Clearing {
            return false;
        }
        // Same result as ticking every category box on the main page.
        for cat in &mut self.categories {
            cat.selected = cat.subs.iter().cloned().collect();
            if cat.has_empty || cat.subs.is_empty() {
                cat.selected.insert(Arc::from(""));
            }
        }
        if !self.build_program_list() {
            for cat in &mut self.categories {
                cat.selected.clear();
            }
            return false;
        }
        self.start_cleaning();
        true
    }

    /// Tristate checkbox of a single category; selects or clears all of its
    /// subcategories at once.
    fn category_checkbox(&mut self, ui: &mut egui::Ui, idx: usize) {
        let cat = &mut self.categories[idx];
        let is_checked = cat.is_checked();
        let is_indet = cat.is_indeterminate();
        let (resp, clicked) =
            tristate_checkbox(ui, is_checked, is_indet, &self.category_labels[idx]);
        if clicked {
            if is_checked || is_indet {
                cat.selected.clear();
                sounds::uncheck();
            } else {
                cat.selected = cat.subs.iter().cloned().collect();
                if cat.has_empty {
                    cat.selected.insert(Arc::from(""));
                }
                if cat.subs.is_empty() && !cat.has_empty {
                    cat.selected.insert(Arc::from(""));
                }
                sounds::check();
            }
        }
        let _ = resp;
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
        let cat = &mut self.categories[idx];
        if cat.subs.is_empty() {
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
                        for sub in cat.subs.clone() {
                            let key = (cat.name.clone(), sub.clone());
                            let label = match self.sub_counts.get(&key).copied() {
                                Some(n) if n > 0 => format!("{} ({})", sub, n),
                                _ => sub.to_string(),
                            };
                            let mut is_sel = cat.selected.contains(&sub);
                            if ui.checkbox(&mut is_sel, &label).changed() {
                                if is_sel {
                                    cat.selected.insert(sub.clone());
                                    sounds::check();
                                } else {
                                    cat.selected.remove(&sub);
                                    sounds::uncheck();
                                }
                            }
                        }
                        // Show Uncategorized for objects without sub_category, only if
                        // category has >= 1 real sub
                        if cat.has_empty {
                            let key = (Arc::clone(&cat.name), Arc::from(""));
                            let label = match self.sub_counts.get(&key).copied() {
                                Some(n) if n > 0 => format!("Uncategorized ({})", n),
                                _ => String::from("Uncategorized"),
                            };
                            let mut is_uncat = cat.selected.contains("");
                            if ui.checkbox(&mut is_uncat, &label).changed() {
                                if is_uncat {
                                    cat.selected.insert(Arc::from(""));

                                    sounds::check();
                                } else {
                                    cat.selected.remove("");
                                    sounds::uncheck();
                                }
                            }
                        }
                    });
            });
    }
}
