//! Application state and the main UI (categories, program selection,
//! progress, results, changelog viewport).

use database::get_version;
#[cfg(windows)]
use database::structures::CleanerDataRegistry;
use database::structures::{CleanerData, Cleared, CustomCleaner};
use database::utils::get_file_size_string;
use database::version::{Changelog, NewRelease, fetch_changelogs};
use eframe::egui;
use crate::notifications::{NotificationAction, NotificationManager, UpdateNotification};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::categories::{CategoryState, effective_sub, tristate_checkbox};
use crate::cleaning::work;
use crate::icons::{MENU_BYTES, load_asset_image, load_icon_color_image};
use crate::sounds;
use crate::taskbar;
use crate::title_bar::{TITLE_BAR_HEIGHT, paint_window_border, title_bar};

pub struct MyApp {
    pub categories: Vec<CategoryState>,
    /// legacy alias kept for tests compat: returns categories length etc.
    /// We keep checked_boxes as deprecated view for tests via method, but store as categories.
    /// For internal compat we also expose checked_boxes as computed (not stored). However test expects field.
    /// So we add a helper method and keep field via Deref? Instead keep both via getter.
    pub task_handle: Option<tokio::task::JoinHandle<(u64, u64, u64, Vec<Cleared>)>>,
    pub progress_message: String,
    pub progress_receiver: Option<mpsc::Receiver<String>>,
    pub cleared_data: Option<(u64, u64, u64, Vec<Cleared>)>,
    pub show_results: bool,
    pub current_task: usize,
    pub total_tasks: usize,
    pub cleaned_bytes: u64,
    pub progress_start: Option<std::time::Instant>,

    pub show_program_selection: bool,
    pub program_checkboxes: Vec<(Rc<RefCell<bool>>, String)>,
    pub search_query: String,
    pub search_query_visible: String,
    pub excluded_programs: HashSet<String>,
    pub results_window_resized: bool,

    pub result_sender: Option<mpsc::Sender<(u64, u64, u64, Vec<Cleared>)>>,
    pub result_receiver: Option<mpsc::Receiver<(u64, u64, u64, Vec<Cleared>)>>,

    pub database: Arc<[CleanerData]>,
    pub custom_database: Arc<[CustomCleaner]>,
    #[cfg(windows)]
    pub regisry_database: Arc<[CleanerDataRegistry]>,
    pub menu_texture: Option<egui::TextureHandle>,
    pub icon_texture: Option<egui::TextureHandle>,

    pub update_receiver: Option<std::sync::mpsc::Receiver<Result<Option<NewRelease>, String>>>,
    /// All currently visible notifications (update banner, etc.).
    pub notifications: NotificationManager,
    /// Shared slot filled by the background changelog fetch.
    pub changelog: Option<Arc<std::sync::Mutex<Option<Changelog>>>>,
    /// Background fetch of the changelog.
    pub changelog_handle: Option<std::thread::JoinHandle<()>>,
    /// Set to false when the user closes the changelog viewport window.
    pub changelog_open: Option<Arc<std::sync::Mutex<bool>>>,
    /// True while the changelog window is open.
    pub show_changelog: bool,
    /// Windows taskbar progress (no-op on other platforms).
    pub taskbar: Option<taskbar::TaskbarProgress>,
}

impl MyApp {
    // Helper for legacy test code: checked_boxes view
    #[allow(dead_code)]
    pub fn checked_boxes(&self) -> Vec<(Rc<RefCell<bool>>, String)> {
        self.categories
            .iter()
            .map(|c| {
                let b = c.is_checked();
                (Rc::new(RefCell::new(b)), c.name.clone())
            })
            .collect()
    }

    #[cfg(windows)]
    pub(crate) fn from_database(
        database: Arc<[CleanerData]>,
        reg_database: Arc<[CleanerDataRegistry]>,
        custom_database: Arc<[CustomCleaner]>,
    ) -> Self {
        let mut cat_to_subs: HashMap<String, HashSet<String>> = HashMap::new();
        let mut cat_has_empty: HashMap<String, bool> = HashMap::new();
        // Ensure all categories appear even if no sub_category
        for data in database.iter() {
            cat_to_subs.entry(data.category.clone()).or_default();
            cat_has_empty.entry(data.category.clone()).or_insert(false);
            let sub = effective_sub(&data.class, &data.sub_category);
            if !sub.is_empty() {
                cat_to_subs.get_mut(&data.category).unwrap().insert(sub);
            } else {
                *cat_has_empty.get_mut(&data.category).unwrap() = true;
            }
        }
        for data in custom_database.iter() {
            cat_to_subs.entry(data.category.clone()).or_default();
            cat_has_empty.entry(data.category.clone()).or_insert(false);
            let sub = effective_sub("", &data.sub_category);
            if !sub.is_empty() {
                cat_to_subs.get_mut(&data.category).unwrap().insert(sub);
            } else {
                *cat_has_empty.get_mut(&data.category).unwrap() = true;
            }
        }
        for data in reg_database.iter() {
            if data.category.is_empty() {
                continue;
            }
            cat_to_subs.entry(data.category.clone()).or_default();
            cat_has_empty.entry(data.category.clone()).or_insert(false);
            let sub = effective_sub(&data.class, &data.sub_category);
            if !sub.is_empty() {
                cat_to_subs.get_mut(&data.category).unwrap().insert(sub);
            } else {
                *cat_has_empty.get_mut(&data.category).unwrap() = true;
            }
        }
        let mut options: Vec<String> = cat_to_subs.keys().cloned().collect();

        let priority = |s: &str| match s {
            "Cache" => 0,
            "Logs" => 1,
            "Crashes" => 2,
            "Documentation" => 3,
            "Backups" => 4,
            "LastActivity" => 5,
            _ => 6,
        };

        options.sort_by(|a, b| {
            let a_prio = priority(a);
            let b_prio = priority(b);

            if a_prio == b_prio {
                a.cmp(b)
            } else {
                a_prio.cmp(&b_prio)
            }
        });

        let mut categories = vec![];
        for opt in options {
            let mut subs: Vec<String> = cat_to_subs
                .remove(&opt)
                .unwrap_or_default()
                .into_iter()
                .collect();
            subs.sort();
            let has_empty = cat_has_empty.remove(&opt).unwrap_or(false);
            categories.push(CategoryState {
                name: opt,
                subs,
                has_empty,
                selected: HashSet::new(),
            });
        }

        let (result_sender, result_receiver) = mpsc::channel(1);

        Self {
            database,
            custom_database,
            #[cfg(windows)]
            regisry_database: reg_database,
            categories,
            task_handle: None,
            progress_message: String::new(),
            progress_receiver: None,
            cleared_data: None,
            show_results: false,
            current_task: 0,
            total_tasks: 0,
            cleaned_bytes: 0,
            progress_start: None,

            show_program_selection: false,
            program_checkboxes: vec![],
            search_query: String::new(),
            search_query_visible: String::new(),
            excluded_programs: HashSet::new(),
            results_window_resized: false,

            result_sender: Some(result_sender),
            result_receiver: Some(result_receiver),
            menu_texture: None,
            icon_texture: None,

            update_receiver: None,
            notifications: NotificationManager::default(),
            changelog: None,
            changelog_handle: None,
            changelog_open: None,
            show_changelog: false,
            taskbar: None,
        }
    }

    #[cfg(not(windows))]
    pub(crate) fn from_database(
        database: Arc<[CleanerData]>,
        custom_database: Arc<[CustomCleaner]>,
    ) -> Self {
        let mut cat_to_subs: HashMap<String, HashSet<String>> = HashMap::new();
        let mut cat_has_empty: HashMap<String, bool> = HashMap::new();
        for data in database.iter() {
            cat_to_subs.entry(data.category.clone()).or_default();
            cat_has_empty.entry(data.category.clone()).or_insert(false);
            let sub = effective_sub(&data.class, &data.sub_category);
            if !sub.is_empty() {
                cat_to_subs.get_mut(&data.category).unwrap().insert(sub);
            } else {
                *cat_has_empty.get_mut(&data.category).unwrap() = true;
            }
        }
        for data in custom_database.iter() {
            cat_to_subs.entry(data.category.clone()).or_default();
            cat_has_empty.entry(data.category.clone()).or_insert(false);
            let sub = effective_sub("", &data.sub_category);
            if !sub.is_empty() {
                cat_to_subs.get_mut(&data.category).unwrap().insert(sub);
            } else {
                *cat_has_empty.get_mut(&data.category).unwrap() = true;
            }
        }

        let mut options: Vec<String> = cat_to_subs.keys().cloned().collect();

        let priority = |s: &str| match s {
            "Cache" => 0,
            "Logs" => 1,
            "Crashes" => 2,
            "Documentation" => 3,
            "Backups" => 4,
            "LastActivity" => 5,
            _ => 6,
        };

        options.sort_by(|a, b| {
            let a_prio = priority(a);
            let b_prio = priority(b);

            if a_prio == b_prio {
                a.cmp(b)
            } else {
                a_prio.cmp(&b_prio)
            }
        });

        let mut categories = vec![];
        for opt in options {
            let mut subs: Vec<String> = cat_to_subs
                .remove(&opt)
                .unwrap_or_default()
                .into_iter()
                .collect();
            subs.sort();
            let has_empty = cat_has_empty.remove(&opt).unwrap_or(false);
            categories.push(CategoryState {
                name: opt,
                subs,
                has_empty,
                selected: HashSet::new(),
            });
        }

        let (result_sender, result_receiver) = mpsc::channel(1);

        Self {
            database,
            custom_database,
            categories,
            task_handle: None,
            progress_message: String::new(),
            progress_receiver: None,
            cleared_data: None,
            show_results: false,
            current_task: 0,
            total_tasks: 0,
            cleaned_bytes: 0,
            progress_start: None,

            show_program_selection: false,
            program_checkboxes: vec![],
            search_query: String::new(),
            search_query_visible: String::new(),
            excluded_programs: HashSet::new(),
            results_window_resized: false,

            result_sender: Some(result_sender),
            result_receiver: Some(result_receiver),
            menu_texture: None,
            icon_texture: None,

            update_receiver: None,
            notifications: NotificationManager::default(),
            changelog: None,
            changelog_handle: None,
            changelog_open: None,
            show_changelog: false,
            taskbar: None,
        }
    }

    fn selected_map(&self) -> HashMap<String, HashSet<String>> {
        let mut map = HashMap::new();
        for cat in &self.categories {
            if !cat.selected.is_empty() {
                map.insert(cat.name.clone(), cat.selected.clone());
            }
        }
        map
    }

    fn has_selection(&self) -> bool {
        self.categories.iter().any(|c| !c.selected.is_empty())
    }

    /// Opens the changelog window (fetch starts in `show_changelog_window`).
    fn open_changelog(&mut self) {
        self.show_changelog = true;
    }

    /// Draws the changelog viewport ("What's New") as a separate native window.
    /// The changelog is shared via `Arc<Mutex<Option<Changelog>>>` so the
    /// background fetch fills it while the window is open.
    fn show_changelog_window(&mut self, ctx: &egui::Context) {
        if !self.show_changelog {
            return;
        }

        // Spawn the fetch once per open session.
        if self.changelog.is_none() && self.changelog_handle.is_none() {
            let current_version = get_version().to_string();
            let shared: Arc<std::sync::Mutex<Option<Changelog>>> =
                Arc::new(std::sync::Mutex::new(None));
            let shared_for_thread = shared.clone();
            self.changelog_handle = Some(std::thread::spawn(move || {
                let fetched = fetch_changelogs(&current_version).unwrap_or_default();
                *shared_for_thread.lock().expect("changelog mutex poisoned") = Some(fetched);
            }));
            self.changelog = Some(shared);
            self.changelog_open = Some(Arc::new(std::sync::Mutex::new(true)));
        }

        // Mark the slot as done once the fetch thread finishes, so the window
        // stops showing the spinner even if nothing was parsed.
        let shared = self.changelog.clone().expect("changelog slot exists");
        if let Some(handle) = self.changelog_handle.as_ref() {
            if handle.is_finished() {
                if let Ok(mut slot) = shared.lock() {
                    if slot.is_none() {
                        *slot = Some(Changelog::default());
                    }
                }
            }
        }

        let shared_open = self
            .changelog_open
            .clone()
            .expect("changelog open flag exists");
        let icon = self.icon_texture.clone();
        ctx.show_viewport_deferred(
            egui::ViewportId(egui::Id::new("changelog_viewport")),
            egui::ViewportBuilder::default()
                .with_title("Cross Cleaner - What's New")
                .with_inner_size([560.0, 640.0])
                .with_min_inner_size([460.0, 480.0])
                .with_decorations(false),
            move |ctx, _class| {
                // Detect the user pressing X on this viewport window.
                if ctx.input(|i| i.viewport().close_requested()) {
                    *shared_open.lock().expect("changelog open flag poisoned") = false;
                }
                let icon = icon.clone();
                // Same zero-margin panel as the main window so the title bar
                // is flush with the top edge and exactly TITLE_BAR_HEIGHT tall.
                egui::CentralPanel::default()
                    .frame(
                        egui::Frame::new()
                            .inner_margin(egui::Margin::same(0))
                            .fill(ctx.style().visuals.panel_fill),
                    )
                    .show(ctx, |ui| {
                        let ctx = ui.ctx().clone();
                        // Same custom title bar as the main window (drag, GitHub,
                        // minimize & close buttons). Close sends ViewportCommand::Close
                        // to this viewport, which is handled above.
                        title_bar(ui, &ctx, "Cross Cleaner - What's New", icon.as_ref(), false);
                        // Same 2px outline as the main window.
                        let focused = ctx.input(|i| i.viewport().focused.unwrap_or(false));
                        let border_color = if focused {
                            egui::Color32::from_rgb(0, 120, 215)
                        } else {
                            ui.visuals().text_color()
                        };
                        paint_window_border(&ctx, "changelog_window_border", border_color);
                        ui.add_space(8.0);
                        // Inner padding around the scroll content, matching
                        // the main window's 8px panel margin.
                        egui::Frame::new()
                            .inner_margin(egui::Margin::same(8))
                            .show(ui, |ui| {
                                egui::ScrollArea::vertical()
                                    .id_salt("changelog_scroll")
                                    // Fill the full window width so the scrollbar sits at
                                    // the window edge instead of hugging the text.
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| {
                                        let fetched =
                                            shared.lock().ok().and_then(|slot| slot.clone());
                                        match fetched {
                                            Some(changelog) => {
                                                if changelog.groups.is_empty()
                                                    && changelog.contributors.is_empty()
                                                {
                                                    ui.label("No changes found.");
                                                }
                                                for group in &changelog.groups {
                                                    ui.add_space(4.0);
                                                    ui.strong(&group.title);
                                                    for item in &group.items {
                                                        ui.horizontal_wrapped(|ui| {
                                                            ui.label("•");
                                                            ui.label(item);
                                                        });
                                                    }
                                                }
                                                if !changelog.contributors.is_empty() {
                                                    ui.add_space(8.0);
                                                    ui.strong("Contributors");
                                                    for contributor in &changelog.contributors {
                                                        ui.horizontal_wrapped(|ui| {
                                                            ui.label("•");
                                                            ui.label(contributor);
                                                        });
                                                    }
                                                }
                                            }
                                            None => {
                                                ui.horizontal(|ui| {
                                                    ui.spinner();
                                                    ui.label("Loading changelog...");
                                                });
                                            }
                                        }
                                    });
                            });
                    });
            },
        );

        // Reset state when the user closed the viewport window; the native
        // window disappears on the next frame because the viewport is no
        // longer requested.
        let still_open = self
            .changelog_open
            .as_ref()
            .and_then(|flag| flag.lock().ok().map(|v| *v))
            .unwrap_or(false);
        if !still_open {
            self.show_changelog = false;
            self.changelog = None;
            self.changelog_handle = None;
            self.changelog_open = None;
        }
    }
}

impl eframe::App for MyApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // Windows taskbar progress: create once on the first frame (needs HWND).
        if self.taskbar.is_none() {
            self.taskbar = Some(taskbar::TaskbarProgress::new(frame));
        }
        let focused = ctx.input(|i| i.viewport().focused.unwrap_or(false));
        let border_color = if focused {
            egui::Color32::from_rgb(0, 120, 215)
        } else {
            ui.visuals().text_color()
        };
        paint_window_border(&ctx, "main_window_border", border_color);
        if let Some(receiver) = &mut self.progress_receiver {
            if let Ok(message) = receiver.try_recv() {
                if message.starts_with("PROGRESS:") {
                    let parts: Vec<&str> = message.split(':').collect();
                    if parts.len() == 4 {
                        self.current_task = parts[1].parse().unwrap_or(0);
                        self.total_tasks = parts[2].parse().unwrap_or(0);
                        self.cleaned_bytes = parts[3].parse().unwrap_or(0);
                        if self.progress_start.is_none() {
                            self.progress_start = Some(std::time::Instant::now());
                        }
                        // Mirror the cleaning progress on the Windows taskbar.
                        if self.total_tasks > 0 {
                            if let Some(taskbar) = &self.taskbar {
                                taskbar.set_progress(
                                    self.current_task as u64,
                                    self.total_tasks as u64,
                                );
                            }
                        }
                    }
                } else {
                    self.progress_message = message;
                }
                ctx.request_repaint();
            }
        }

        if let Some(receiver) = &mut self.result_receiver {
            if let Ok(result) = receiver.try_recv() {
                self.cleared_data = Some(result);
                self.show_results = true;
                self.results_window_resized = false; // Reset flag for new results
                self.result_receiver = None; // Consume the result once
                sounds::done();
                ctx.request_repaint();
            }
        }

        if let Some(receiver) = &mut self.update_receiver {
            match receiver.try_recv() {
                Ok(check) => {
                    self.update_receiver = None;
                    if let Ok(Some(release)) = check {
                        self.notifications.push(UpdateNotification::new(release));
                        ctx.request_repaint();
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.update_receiver = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }

        if let Some(handle) = &mut self.task_handle {
            if handle.is_finished() {
                // Cleaning is done: clear the taskbar progress indicator.
                if let Some(taskbar) = &self.taskbar {
                    taskbar.remove();
                }
                let handle = self.task_handle.take().unwrap();
                if let Some(sender) = self.result_sender.take() {
                    tokio::spawn(async move {
                        match handle.await {
                            Ok(result) => {
                                let _ = sender.send(result).await;
                            }
                            Err(e) => eprintln!("Task failed: {:?}", e),
                        }
                    });
                }
            }
        }

        // INFO: Floating notifications (right side, above everything else)
        for (id, action) in self.notifications.update(&ctx) {
            match action {
                NotificationAction::Close => self.notifications.close(id),
                NotificationAction::ShowChangelog => self.open_changelog(),
                NotificationAction::None => {}
            }
        }
        self.show_changelog_window(&ctx);

        let title = format!("Cross Cleaner GUI v{}", get_version());
        if self.icon_texture.is_none() {
            self.icon_texture = Some(ctx.load_texture(
                "app_icon",
                load_icon_color_image(),
                egui::TextureOptions::LINEAR,
            ));
        }
        let show_back =
            (self.show_results && self.cleared_data.is_some()) || self.show_program_selection;
        let back_clicked = title_bar(ui, &ctx, &title, self.icon_texture.as_ref(), show_back);
        if back_clicked {
            if self.show_program_selection {
                self.show_program_selection = false;
            } else {
                self.show_results = false;
                self.cleared_data = None;
                self.results_window_resized = false;
            }
        }
        let inner_margin = 8;
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .inner_margin(egui::Margin::same(inner_margin))
                    .fill(ui.visuals().panel_fill),
            )
            .show(ui, |ui| {
                if self.task_handle.is_some() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::Vec2::new(
                        470.0,
                        100.0 + TITLE_BAR_HEIGHT,
                    )));
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
                                    .animate(true),
                            );
                            ui.add_space(4.0);

                            let eta = self.progress_start.and_then(|start| {
                                if self.current_task == 0 || self.current_task >= self.total_tasks {
                                    None
                                } else {
                                    let elapsed = start.elapsed().as_secs_f64();
                                    let per_task = elapsed / self.current_task as f64;
                                    let remaining =
                                        per_task * (self.total_tasks - self.current_task) as f64;
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
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Min),
                                    |ui| {
                                        if let Some(eta) = eta {
                                            ui.label(eta);
                                        }
                                        ui.add_space(4.0);
                                    },
                                );
                            });
                        } else {
                            ui.spinner();
                        }
                    });
                    return;
                }

                if self.show_results {
                    if let Some((bytes, files, dirs, cleared)) = &self.cleared_data {
                        ui.vertical_centered(|ui| {
                            ui.heading("Cleaning Results");
                            ui.heading(format!(
                                "Size: {}, Files: {}, Dirs: {}",
                                get_file_size_string(*bytes),
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
                            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(
                                egui::Vec2::new(total_width, total_height + TITLE_BAR_HEIGHT),
                            ));
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

                            // Scrollable table content
                            egui::ScrollArea::vertical()
                                .max_height(total_height)
                                .show(ui, |ui| {
                                    for cleared in cleared {
                                        ui.horizontal(|ui| {
                                            ui.style_mut().spacing.item_spacing =
                                                egui::vec2(0.0, 0.0);

                                            // Program column
                                            ui.add_sized(
                                                egui::vec2(column_widths[0], 20.0),
                                                egui::Label::new(&cleared.program).truncate(),
                                            );

                                            // Size column
                                            ui.add_sized(
                                                egui::vec2(column_widths[1], 20.0),
                                                egui::Label::new(get_file_size_string(
                                                    cleared.removed_bytes,
                                                ))
                                                .truncate(),
                                            );

                                            // Files column
                                            ui.add_sized(
                                                egui::vec2(column_widths[2], 20.0),
                                                egui::Label::new(cleared.removed_files.to_string())
                                                    .truncate(),
                                            );

                                            // Dirs column
                                            ui.add_sized(
                                                egui::vec2(column_widths[2], 20.0),
                                                egui::Label::new(
                                                    cleared.removed_directories.to_string(),
                                                )
                                                .truncate(),
                                            );

                                            // Categories column
                                            ui.add_sized(
                                                egui::vec2(column_widths[3], 20.0),
                                                egui::Label::new(
                                                    cleared.affected_categories.join(", "),
                                                )
                                                .wrap(),
                                            );
                                        });
                                        ui.separator();
                                    }
                                });
                        });
                        return;
                    }
                }

                if self.show_program_selection {
                    // Dynamic window sizing based on number of programs
                    let num_programs = self.program_checkboxes.len();
                    let rows = (num_programs + 1) / 2; // 2 columns
                    let row_height = 20.0;
                    let base_height = 120.0; // Heading, search, buttons, separators
                    let min_scroll_height = 20.0;
                    let max_scroll_height = 400.0;

                    let content_height = rows as f32 * row_height;
                    let scroll_height =
                        content_height.min(max_scroll_height).max(min_scroll_height);
                    let window_height = base_height + scroll_height;

                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::Vec2::new(
                        500.0,
                        window_height + TITLE_BAR_HEIGHT,
                    )));

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
                        }
                    });

                    ui.separator();

                    egui::ScrollArea::vertical()
                        .max_height(scroll_height)
                        .show(ui, |ui| {
                            ui.columns(2, |columns| {
                                let mut col_index = 0;
                                for (checkbox, program) in self.program_checkboxes.iter() {
                                    if self.search_query.is_empty()
                                        || program.to_lowercase().contains(&self.search_query)
                                    {
                                        let mut value = checkbox.borrow_mut();
                                        let resp =
                                            columns[col_index % 2].checkbox(&mut *value, program);
                                        if resp.changed() {
                                            if *value {
                                                sounds::check();
                                            } else {
                                                sounds::uncheck();
                                            }
                                        }
                                        col_index += 1;
                                    }
                                }
                            });
                        });

                    ui.separator();

                    let available_width = ui.available_width();
                    ui.horizontal(|ui| {
                        if ui
                            .add_sized([available_width, 25.0], egui::Button::new("Start Cleaning"))
                            .clicked()
                        {
                            sounds::click();
                            let selected_map = self.selected_map();

                            self.excluded_programs.clear();
                            for (checkbox, program) in &self.program_checkboxes {
                                if !*checkbox.borrow() {
                                    self.excluded_programs.insert(program.clone());
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

                            let database = Arc::clone(&self.database);
                            let custom_database = Arc::clone(&self.custom_database);
                            #[cfg(windows)]
                            let reg_database = Arc::clone(&self.regisry_database);
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
                                )
                                .await
                            });
                            self.task_handle = Some(handle);

                            self.show_program_selection = false;
                            // clear selection
                            for cat in &mut self.categories {
                                cat.selected.clear();
                            }
                        }
                    });
                } else {
                    // Calculate dynamic window height based on number of categories
                    let num_categories = self.categories.len();
                    let rows = (num_categories + 2) / 3; // Round up division by 3 (3 columns)
                    let row_height = 20.0; // Approximate height per row
                    let base_height = 45.0; // Space for heading, margins, and button
                    let dynamic_height = base_height + (rows as f32 * row_height);
                    let window_height = dynamic_height.max(20.0).min(500.0); // Clamp between 200 and 500

                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::Vec2::new(
                        470.0,
                        window_height + TITLE_BAR_HEIGHT,
                    )));

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
                                    tristate_checkbox(ui, is_checked, is_indet, &cat.name.clone());
                                if clicked {
                                    if is_checked || is_indet {
                                        cat.selected.clear();
                                        sounds::uncheck();
                                    } else {
                                        cat.selected = cat.subs.iter().cloned().collect();
                                        if cat.has_empty {
                                            cat.selected.insert(String::new());
                                        }
                                        if cat.subs.is_empty() && !cat.has_empty {
                                            cat.selected.insert(String::new());
                                        }
                                        sounds::check();
                                    }
                                }
                                // menu image only if sub-categories exist (embedded menu.png)
                                if !cat.subs.is_empty() {
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

                                    // Popup with sub_category checkboxes - shifted to right-bottom corner of image so it doesn't cover the button
                                    let frame = egui::Frame::popup(ui.style());
                                    egui::Popup::menu(&menu_resp)
                                        .close_behavior(
                                            egui::PopupCloseBehavior::CloseOnClickOutside,
                                        )
                                        .frame(frame)
                                        .show(|ui| {
                                            ui.set_min_width(200.0);
                                            egui::ScrollArea::vertical().max_height(300.0).show(
                                                ui,
                                                |ui| {
                                                    for sub in cat.subs.clone() {
                                                        let mut is_sel =
                                                            cat.selected.contains(&sub);
                                                        if ui.checkbox(&mut is_sel, &sub).changed()
                                                        {
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
                                                        let mut is_uncat =
                                                            cat.selected.contains("");
                                                        if ui
                                                            .checkbox(
                                                                &mut is_uncat,
                                                                "Uncategorized",
                                                            )
                                                            .changed()
                                                        {
                                                            if is_uncat {
                                                                cat.selected.insert(String::new());
                                                                sounds::check();
                                                            } else {
                                                                cat.selected.remove(&String::new());
                                                                sounds::uncheck();
                                                            }
                                                        }
                                                    }
                                                },
                                            );
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
                            let mut programs: Vec<String> = Vec::new();
                            for data in self.database.iter() {
                                let eff = effective_sub(&data.class, &data.sub_category);
                                if let Some(subs) = selected_map.get(&data.category) {
                                    if subs.contains(&eff) && !programs.contains(&data.program) {
                                        programs.push(data.program.clone());
                                    }
                                }
                            }
                            for data in self.custom_database.iter() {
                                let eff = effective_sub("", &data.sub_category);
                                if let Some(subs) = selected_map.get(&data.category) {
                                    if subs.contains(&eff) && !programs.contains(&data.program) {
                                        programs.push(data.program.clone());
                                    }
                                }
                            }
                            #[cfg(windows)]
                            {
                                for data in self.regisry_database.iter() {
                                    let eff = effective_sub(&data.class, &data.sub_category);
                                    if let Some(subs) = selected_map.get(&data.category) {
                                        if subs.contains(&eff) && !programs.contains(&data.program)
                                        {
                                            programs.push(data.program.clone());
                                        }
                                    }
                                }
                            }
                            programs.sort();

                            self.program_checkboxes.clear();
                            for program in programs {
                                self.program_checkboxes
                                    .push((Rc::new(RefCell::new(true)), program));
                            }

                            self.show_program_selection = true;
                        }
                    }
                }
            });
    }
}
