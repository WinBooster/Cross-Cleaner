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

impl MyApp {
    pub(crate) fn render_main(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        // Calculate dynamic window height based on number of categories
        let num_categories = self.categories.len();
        let rows = num_categories.div_ceil(3); // Round up division by 3 (3 columns)
        let row_height = 20.0; // Approximate height per row
        let base_height = 45.0; // Space for heading, margins, and button
        let dynamic_height = base_height + (rows as f32 * row_height);
        let window_height = dynamic_height.clamp(20.0, 500.0); // Clamp between 200 and 500

        self.set_window_size(
            ctx,
            egui::Vec2::new(560.0, window_height + TITLE_BAR_HEIGHT),
        );

        if self.menu_texture.is_none() {
            self.menu_texture = Some(ctx.load_texture(
                "menu",
                load_asset_image(MENU_BYTES),
                egui::TextureOptions::LINEAR,
            ));
        }
        let menu_tex = self.menu_texture.clone().unwrap();

        ui.columns(3, |columns| {
            for (idx, cat) in self.categories.iter_mut().enumerate() {
                let column_index = idx % 3;
                let is_checked = cat.is_checked();
                let is_indet = cat.is_indeterminate();

                columns[column_index].horizontal(|ui| {
                    // Tristate checkbox with square for indeterminate
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
                    // menu image only if sub-categories exist (embedded menu.png)
                    if !cat.subs.is_empty() {
                        let menu_image = egui::Image::from_texture(egui::load::SizedTexture::new(
                            menu_tex.id(),
                            menu_tex.size_vec2(),
                        ))
                        .fit_to_exact_size(egui::vec2(16.0, 16.0))
                        .tint(ui.visuals().text_color())
                        .sense(egui::Sense::click());
                        let menu_resp = ui.add_sized(egui::vec2(16.0, 16.0), menu_image);
                        if menu_resp.clicked() {
                            sounds::pop();
                        }

                        // Popup with sub_category checkboxes - shifted to right-bottom corner of image so it doesn't cover the button
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
                                        // Show Uncategorized for objects without sub_category, only if category has >= 1 real sub
                                        if cat.has_empty {
                                            let key = (Arc::clone(&cat.name), Arc::from(""));
                                            let label = match self.sub_counts.get(&key).copied() {
                                                Some(n) if n > 0 => {
                                                    format!("Uncategorized ({})", n)
                                                }
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
                    let _ = resp;
                });
            }
        });

        let available_width = ui.available_width();

        if ui
            .add_sized([available_width, 25.0], egui::Button::new("Next"))
            .clicked()
        {
            sounds::click();
            if self.has_selection() {
                let selected_map = self.selected_map();
                let mut programs: Vec<(Arc<str>, Vec<Arc<str>>)> = Vec::new();
                let mut add = |program: Arc<str>, category: Arc<str>| {
                    if let Some(entry) = programs.iter_mut().find(|(p, _)| p.as_ref() == program.as_ref()) {
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
                self.current_page = Page::ProgramSelection;
            }
        }
    }
}
