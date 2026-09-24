//! Program selection page: search, program checkboxes with per-program
//! category popups, and the Start Cleaning button.

use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::mpsc;

use eframe::egui;

use crate::app::{MyApp, Page};
use crate::categories::tristate_checkbox;
use crate::cleaning::work;
use crate::icons::{MENU_BYTES, load_asset_image};
use crate::sounds;
use crate::title_bar::TITLE_BAR_HEIGHT;

impl MyApp {
    pub(crate) fn render_program_selection(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        // Dynamic window sizing based on number of (filtered) programs
        let num_programs = self.filtered_programs.len();
        let rows = num_programs.div_ceil(2); // 2 columns
        let row_height = 20.0;
        let base_height = 120.0; // Heading, search, buttons, separators
        let min_scroll_height = 20.0;
        let max_scroll_height = 400.0;

        let content_height = rows as f32 * row_height;
        let scroll_height = content_height.min(max_scroll_height).max(min_scroll_height);
        let window_height = base_height + scroll_height;

        self.set_window_size(
            ctx,
            egui::Vec2::new(500.0, window_height + TITLE_BAR_HEIGHT),
        );

        ui.vertical_centered(|ui| {
            ui.heading("Select Programs to Clean");
        });
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Search:");
            let available_width = ui.available_width();
            let search_response = ui.add_sized(
                [available_width, 20.0],
                egui::TextEdit::singleline(&mut self.search_query_visible),
            );
            if search_response.changed() {
                self.search_query = self.search_query_visible.to_lowercase();
                self.rebuild_filtered_programs();
            }
        });

        ui.separator();

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
        let total_rows = self.filtered_programs.len().div_ceil(2);
        egui::ScrollArea::vertical()
            .max_height(scroll_height)
            .show_rows(ui, row_height, total_rows, |ui, row_range| {
                for row in row_range {
                    ui.columns(2, |columns| {
                        for (col, column) in columns.iter_mut().enumerate().take(2) {
                            let Some(&i) = self.filtered_programs.get(row * 2 + col) else {
                                break;
                            };
                            let (checkbox, program) = &self.program_checkboxes[i];
                            let master = *checkbox.borrow();
                            let is_checked = master && self.program_disabled[i].is_empty();
                            let is_indet = master && !self.program_disabled[i].is_empty();
                            column.horizontal(|ui| {
                                let (_resp, clicked) =
                                    tristate_checkbox(ui, is_checked, is_indet, &*program);
                                if clicked {
                                    if is_checked || is_indet {
                                        *checkbox.borrow_mut() = false;
                                        self.program_disabled[i].clear();
                                        sounds::uncheck();
                                    } else {
                                        *checkbox.borrow_mut() = true;
                                        self.program_disabled[i].clear();
                                        sounds::check();
                                    }
                                }
                                // Per-program popup: disable individual
                                // categories for this program only.
                                if let Some(cats) = self.program_categories.get(i)
                                    && cats.len() > 1
                                {
                                    let menu_image =
                                        egui::Image::from_texture(egui::load::SizedTexture::new(
                                            menu_tex.id(),
                                            menu_tex.size_vec2(),
                                        ))
                                        .fit_to_exact_size(egui::vec2(16.0, 16.0))
                                        .tint(ui.visuals().text_color())
                                        .sense(egui::Sense::click());
                                    let menu_resp =
                                        ui.add_sized(egui::vec2(16.0, 16.0), menu_image);
                                    if menu_resp.clicked() {
                                        sounds::pop();
                                    }

                                    let frame = egui::Frame::popup(ui.style());
                                    egui::Popup::menu(&menu_resp)
                                        .close_behavior(
                                            egui::PopupCloseBehavior::CloseOnClickOutside,
                                        )
                                        .frame(frame)
                                        .show(|ui| {
                                            ui.set_min_width(180.0);
                                            egui::ScrollArea::vertical().max_height(300.0).show(
                                                ui,
                                                |ui| {
                                                    for cat in cats.clone() {
                                                        let mut enabled = !self.program_disabled[i]
                                                            .contains(&cat);
                                                        if ui.checkbox(&mut enabled, &*cat).changed()
                                                        {
                                                            if enabled {
                                                                self.program_disabled[i]
                                                                    .remove(&cat);
                                                                sounds::uncheck();
                                                            } else {
                                                                self.program_disabled[i]
                                                                    .insert(cat.clone());
                                                                sounds::uncheck();
                                                            }
                                                        }
                                                    }
                                                },
                                            );
                                        });
                                }
                            });
                        }
                    });
                }
            });

        ui.separator();

        let available_width = ui.available_width();
        ui.horizontal(|ui| {
            if ui
                .add_sized([available_width, 25.0], egui::Button::new("Start Cleaning"))
                .clicked()
            {
                sounds::click();
                self.start_cleaning();
            }
        });
    }

    fn start_cleaning(&mut self) {
        let selected_map = self.selected_map();

        self.excluded_programs.clear();
        for (checkbox, program) in &self.program_checkboxes {
            if !*checkbox.borrow() {
                self.excluded_programs.insert(program.clone());
            }
        }

        // Per-program category exclusions from the popups
        let mut excluded_program_categories: HashSet<(Arc<str>, Arc<str>)> = HashSet::new();
        for (i, (checkbox, program)) in self.program_checkboxes.iter().enumerate() {
            if *checkbox.borrow()
                && let Some(disabled) = self.program_disabled.get(i)
            {
                for cat in disabled {
                    excluded_program_categories.insert((Arc::clone(&program), Arc::clone(&cat)));
                }
            }
        }

        let (progress_sender, progress_receiver) = mpsc::channel(32);
        self.progress_receiver = Some(progress_receiver);
        let (result_sender, result_receiver) = mpsc::channel(1);
        self.result_sender = Some(result_sender);
        self.result_receiver = Some(result_receiver);
        self.current_task = 0;
        self.total_tasks = 0;
        self.cleaned_bytes = 0;
        self.progress_start = None;
        self.results_window_resized = false;

        let database = self.database.clone();
        let custom_database = Arc::clone(&self.custom_database);
        #[cfg(windows)]
        let reg_database = self.regisry_database.clone();
        let excluded_programs = self.excluded_programs.clone();
        let handle = tokio::spawn(async move {
            work(
                selected_map,
                progress_sender,
                &database,
                &custom_database,
                #[cfg(windows)]
                &reg_database,
                excluded_programs,
                excluded_program_categories,
            )
            .await
        });
        self.task_handle = Some(handle);

        self.current_page = Page::Clearing;
        // clear selection
        for cat in &mut self.categories {
            cat.selected.clear();
        }
    }
}
