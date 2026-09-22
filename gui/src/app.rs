//! Application state and the main UI (categories, program selection,
//! progress, results, changelog viewport).

use crate::notifications::{NotificationAction, NotificationManager, UpdateNotification};
use database::cleaner_database::CleanerDatabase;
use database::get_version;
#[cfg(windows)]
use database::registry_database::RegistryDatabase;
use database::structures::{Cleared, CustomCleaner};

use database::version::{Changelog, NewRelease, fetch_changelogs};
use eframe::egui;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::categories::{CategoryState, effective_sub};

use crate::icons::{SETTINGS_BYTES, load_asset_image, load_icon_color_image};
use crate::sounds;
use crate::taskbar;
use crate::title_bar::{paint_window_border, title_bar};

type CleanResult = (u64, u64, u64, Vec<Cleared>);

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Page {
    Main,
    ProgramSelection,
    Clearing,
    Results,
    Settings,
}

pub struct MyApp {
    pub categories: Vec<CategoryState>,
    /// legacy alias kept for tests compat: returns categories length etc.
    /// We keep checked_boxes as deprecated view for tests via method, but store as categories.
    /// For internal compat we also expose checked_boxes as computed (not stored). However test expects field.
    /// So we add a helper method and keep field via Deref? Instead keep both via getter.
    pub task_handle: Option<tokio::task::JoinHandle<CleanResult>>,
    pub progress_message: String,
    pub progress_receiver: Option<mpsc::Receiver<String>>,
    pub cleared_data: Option<(u64, u64, u64, Vec<Cleared>)>,
    pub current_page: Page,
    pub current_task: usize,
    pub total_tasks: usize,
    pub cleaned_bytes: u64,
    pub progress_start: Option<std::time::Instant>,

    pub program_checkboxes: Vec<(Rc<RefCell<bool>>, String)>,
    /// Selected categories that apply to each program (parallel to program_checkboxes).
    pub program_categories: Vec<Vec<String>>,
    /// Categories the user disabled per program (parallel to program_checkboxes).
    pub program_disabled: Vec<HashSet<String>>,
    pub search_query: String,
    pub search_query_visible: String,
    /// Indices into `program_checkboxes` matching `search_query`, in order.
    /// Recomputed only when the query or the program list changes, so the UI
    /// never has to scan/to-lowercase the whole list every frame.
    pub filtered_programs: Vec<usize>,
    pub excluded_programs: HashSet<String>,
    pub results_window_resized: bool,

    pub result_sender: Option<mpsc::Sender<CleanResult>>,
    pub result_receiver: Option<mpsc::Receiver<CleanResult>>,

    pub database: CleanerDatabase,
    pub custom_database: Arc<[CustomCleaner]>,
    #[cfg(windows)]
    pub regisry_database: RegistryDatabase,
    pub menu_texture: Option<egui::TextureHandle>,
    pub icon_texture: Option<egui::TextureHandle>,
    pub settings_texture: Option<egui::TextureHandle>,

    pub update_receiver: Option<std::sync::mpsc::Receiver<Result<Option<NewRelease>, String>>>,
    /// Number of database paths per (category, sub_category).
    pub sub_counts: HashMap<(String, String), usize>,
    /// Precomputed checkbox labels like `"Cache (12)"`, parallel to `categories`.
    /// Computed once so the UI does not rebuild them every frame.
    pub category_labels: Vec<String>,
    /// Precomputed window title.
    pub window_title: String,
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
    /// Last inner size sent to the viewport, used to avoid re-issuing
    /// `ViewportCommand::InnerSize` every frame (which forces a repaint).
    pub last_inner_size: Option<egui::Vec2>,
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
        database: CleanerDatabase,
        reg_database: RegistryDatabase,
        custom_database: Arc<[CustomCleaner]>,
    ) -> Self {
        let mut cat_to_subs: HashMap<String, HashSet<String>> = HashMap::new();
        let mut cat_has_empty: HashMap<String, bool> = HashMap::new();
        let mut category_counts: HashMap<String, usize> = HashMap::new();
        let mut sub_counts: HashMap<(String, String), usize> = HashMap::new();
        // Ensure all categories appear even if no sub_category
        database
            .for_each_index(|data| {
                cat_to_subs.entry(data.category.clone()).or_default();
                cat_has_empty.entry(data.category.clone()).or_insert(false);
                *category_counts.entry(data.category.clone()).or_insert(0) += 1;
                let sub = effective_sub("", &data.sub_category);
                *sub_counts
                    .entry((data.category.clone(), sub.clone()))
                    .or_insert(0) += 1;
                if !sub.is_empty() {
                    cat_to_subs.get_mut(&data.category).unwrap().insert(sub);
                } else {
                    *cat_has_empty.get_mut(&data.category).unwrap() = true;
                }
            })
            .expect("Failed to read cleaner database");
        for data in custom_database.iter() {
            cat_to_subs.entry(data.category.clone()).or_default();
            cat_has_empty.entry(data.category.clone()).or_insert(false);
            *category_counts.entry(data.category.clone()).or_insert(0) += 1;
            let sub = effective_sub("", &data.sub_category);
            *sub_counts
                .entry((data.category.clone(), sub.clone()))
                .or_insert(0) += 1;
            if !sub.is_empty() {
                cat_to_subs.get_mut(&data.category).unwrap().insert(sub);
            } else {
                *cat_has_empty.get_mut(&data.category).unwrap() = true;
            }
        }
        reg_database
            .for_each_index(|data| {
                if data.category.is_empty() {
                    return;
                }
                cat_to_subs.entry(data.category.clone()).or_default();
                cat_has_empty.entry(data.category.clone()).or_insert(false);
                *category_counts.entry(data.category.clone()).or_insert(0) += 1;
                let sub = effective_sub("", &data.sub_category);
                *sub_counts
                    .entry((data.category.clone(), sub.clone()))
                    .or_insert(0) += 1;
                if !sub.is_empty() {
                    cat_to_subs.get_mut(&data.category).unwrap().insert(sub);
                } else {
                    *cat_has_empty.get_mut(&data.category).unwrap() = true;
                }
            })
            .expect("Failed to read registry database");
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

        let category_labels: Vec<String> = categories
            .iter()
            .map(|cat| match category_counts.get(&cat.name).copied() {
                Some(n) if n > 0 => format!("{} ({})", cat.name, n),
                _ => cat.name.clone(),
            })
            .collect();
        let window_title = format!("Cross Cleaner GUI v{}", get_version());

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
            current_page: Page::Main,
            current_task: 0,
            total_tasks: 0,
            cleaned_bytes: 0,
            progress_start: None,

            program_checkboxes: vec![],
            program_categories: vec![],
            program_disabled: vec![],
            search_query: String::new(),
            search_query_visible: String::new(),
            filtered_programs: Vec::new(),
            excluded_programs: HashSet::new(),
            results_window_resized: false,

            result_sender: Some(result_sender),
            result_receiver: Some(result_receiver),
            menu_texture: None,
            icon_texture: None,
            settings_texture: None,

            update_receiver: None,
            sub_counts,
            category_labels,
            window_title,
            notifications: NotificationManager::default(),
            changelog: None,
            changelog_handle: None,
            changelog_open: None,
            show_changelog: false,
            taskbar: None,
            last_inner_size: None,
        }
    }

    #[cfg(not(windows))]
    pub(crate) fn from_database(
        database: CleanerDatabase,
        custom_database: Arc<[CustomCleaner]>,
    ) -> Self {
        let mut cat_to_subs: HashMap<String, HashSet<String>> = HashMap::new();
        let mut cat_has_empty: HashMap<String, bool> = HashMap::new();
        let mut category_counts: HashMap<String, usize> = HashMap::new();
        let mut sub_counts: HashMap<(String, String), usize> = HashMap::new();
        database
            .for_each_index(|data| {
                cat_to_subs.entry(data.category.clone()).or_default();
                cat_has_empty.entry(data.category.clone()).or_insert(false);
                *category_counts.entry(data.category.clone()).or_insert(0) += 1;
                let sub = effective_sub("", &data.sub_category);
                *sub_counts
                    .entry((data.category.clone(), sub.clone()))
                    .or_insert(0) += 1;
                if !sub.is_empty() {
                    cat_to_subs.get_mut(&data.category).unwrap().insert(sub);
                } else {
                    *cat_has_empty.get_mut(&data.category).unwrap() = true;
                }
            })
            .expect("Failed to read cleaner database");
        for data in custom_database.iter() {
            cat_to_subs.entry(data.category.clone()).or_default();
            cat_has_empty.entry(data.category.clone()).or_insert(false);
            *category_counts.entry(data.category.clone()).or_insert(0) += 1;
            let sub = effective_sub("", &data.sub_category);
            *sub_counts
                .entry((data.category.clone(), sub.clone()))
                .or_insert(0) += 1;
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

        let category_labels: Vec<String> = categories
            .iter()
            .map(|cat| match category_counts.get(&cat.name).copied() {
                Some(n) if n > 0 => format!("{} ({})", cat.name, n),
                _ => cat.name.clone(),
            })
            .collect();
        let window_title = format!("Cross Cleaner GUI v{}", get_version());

        let (result_sender, result_receiver) = mpsc::channel(1);

        Self {
            database,
            custom_database,
            categories,
            task_handle: None,
            progress_message: String::new(),
            progress_receiver: None,
            cleared_data: None,
            current_page: Page::Main,
            current_task: 0,
            total_tasks: 0,
            cleaned_bytes: 0,
            progress_start: None,

            program_checkboxes: vec![],
            program_categories: vec![],
            program_disabled: vec![],
            search_query: String::new(),
            search_query_visible: String::new(),
            filtered_programs: Vec::new(),
            excluded_programs: HashSet::new(),
            results_window_resized: false,

            result_sender: Some(result_sender),
            result_receiver: Some(result_receiver),
            menu_texture: None,
            icon_texture: None,
            settings_texture: None,

            update_receiver: None,
            sub_counts,
            category_labels,
            window_title,
            notifications: NotificationManager::default(),
            changelog: None,
            changelog_handle: None,
            changelog_open: None,
            show_changelog: false,
            taskbar: None,
            last_inner_size: None,
        }
    }

    /// Sends `ViewportCommand::InnerSize` only when the requested size
    /// actually changed. `send_viewport_cmd` triggers an immediate repaint, so
    /// calling it unconditionally every frame would keep the app rendering at
    /// full frame rate even while idle.
    pub(crate) fn set_window_size(&mut self, ctx: &egui::Context, size: egui::Vec2) {
        if self.last_inner_size != Some(size) {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
            self.last_inner_size = Some(size);
        }
    }

    /// Recomputes `filtered_programs` from `program_checkboxes` and
    /// `search_query`. Cheap and only called when one of them changes.
    pub(crate) fn rebuild_filtered_programs(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_programs = (0..self.program_checkboxes.len()).collect();
            return;
        }
        self.filtered_programs = self
            .program_checkboxes
            .iter()
            .enumerate()
            .filter(|(_, (_, program))| program.to_lowercase().contains(&self.search_query))
            .map(|(i, _)| i)
            .collect();
    }

    pub(crate) fn selected_map(&self) -> HashMap<String, HashSet<String>> {
        let mut map = HashMap::new();
        for cat in &self.categories {
            if !cat.selected.is_empty() {
                map.insert(cat.name.clone(), cat.selected.clone());
            }
        }
        map
    }

    pub(crate) fn has_selection(&self) -> bool {
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
        if let Some(handle) = self.changelog_handle.as_ref()
            && handle.is_finished()
            && let Ok(mut slot) = shared.lock()
            && slot.is_none()
        {
            *slot = Some(Changelog::default());
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
                        title_bar(
                            ui,
                            &ctx,
                            "Cross Cleaner - What's New",
                            icon.as_ref(),
                            false,
                            false,
                            None,
                        );
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
            // Drain everything that is ready, but repaint on a slower cadence
            // (see the `task_handle` branch): cleaning can emit many messages
            // per second, and an immediate repaint for each would keep the
            // software renderer busy.
            while let Ok(message) = receiver.try_recv() {
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
                        if self.total_tasks > 0
                            && let Some(taskbar) = &self.taskbar
                        {
                            taskbar.set_progress(self.current_task as u64, self.total_tasks as u64);
                        }
                    }
                } else {
                    self.progress_message = message;
                }
            }
        }

        if let Some(receiver) = &mut self.result_receiver
            && let Ok(result) = receiver.try_recv()
        {
            self.cleared_data = Some(result);
            self.current_page = Page::Results;
            self.results_window_resized = false;
            self.result_receiver = None;
            sounds::done();
            ctx.request_repaint();
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

        if let Some(handle) = &mut self.task_handle
            && handle.is_finished()
        {
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

        // INFO: Floating notifications (right side, above everything else)
        for (id, action) in self.notifications.update(&ctx) {
            match action {
                NotificationAction::Close => self.notifications.close(id),
                NotificationAction::ShowChangelog => self.open_changelog(),
                NotificationAction::None => {}
            }
        }
        self.show_changelog_window(&ctx);

        if self.icon_texture.is_none() {
            self.icon_texture = Some(ctx.load_texture(
                "app_icon",
                load_icon_color_image(),
                egui::TextureOptions::LINEAR,
            ));
        }
        if self.settings_texture.is_none() {
            self.settings_texture = Some(ctx.load_texture(
                "settings",
                load_asset_image(SETTINGS_BYTES),
                egui::TextureOptions::LINEAR,
            ));
        }
        let prev_page = self.current_page;
        let show_back = matches!(
            self.current_page,
            Page::Results | Page::ProgramSelection | Page::Settings
        );
        let show_settings = self.current_page == Page::Main;
        let (back_clicked, settings_clicked) = title_bar(
            ui,
            &ctx,
            &self.window_title,
            self.icon_texture.as_ref(),
            show_back,
            show_settings,
            self.settings_texture.as_ref(),
        );
        if back_clicked {
            if prev_page == Page::Results {
                self.cleared_data = None;
                self.results_window_resized = false;
            }
            self.current_page = Page::Main;
        }
        if settings_clicked {
            self.current_page = Page::Settings;
        }
        let inner_margin = 8;
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .inner_margin(egui::Margin::same(inner_margin))
                    .fill(ui.visuals().panel_fill),
            )
            .show(ui, |ui| {
                if self.current_page == Page::Clearing {
                    self.render_clearing(&ctx, ui);
                    return;
                }

                if self.current_page == Page::Results && self.render_results(&ctx, ui) {
                    return;
                }

                if self.current_page == Page::ProgramSelection {
                    self.render_program_selection(&ctx, ui);
                } else if self.current_page == Page::Settings {
                    self.render_settings(&ctx, ui);
                } else {
                    self.render_main(&ctx, ui);
                }
            });
    }
}
