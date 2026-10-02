//! Application state and the main UI (categories, program selection,
//! progress, results, changelog viewport).

use crate::notifications::{
    NotificationAction, NotificationManager, UPDATE_PROGRESS_ID, UpdateNotification,
    UpdateProgressNotification,
};
use database::cleaner_database::CleanerDatabase;
use database::get_version;
#[cfg(windows)]
use database::registry_database::RegistryDatabase;
use database::structures::{Cleared, CustomCleaner};

use crate::updater::{self, UpdateStage, UpdateState, UpdaterCommand};
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

    pub program_checkboxes: Vec<(Rc<RefCell<bool>>, Arc<str>)>,
    /// Selected categories that apply to each program (parallel to program_checkboxes).
    pub program_categories: Vec<Vec<Arc<str>>>,
    /// Categories the user disabled per program (parallel to program_checkboxes).
    pub program_disabled: Vec<HashSet<Arc<str>>>,
    pub search_query: String,
    pub search_query_visible: String,
    /// Indices into `program_checkboxes` matching `search_query`, in order.
    /// Recomputed only when the query or the program list changes, so the UI
    /// never has to scan/to-lowercase the whole list every frame.
    pub filtered_programs: Vec<usize>,
    pub excluded_programs: HashSet<Arc<str>>,
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
    /// Progress of the automatic update, filled by the platform updater worker.
    pub updater_state: UpdateState,
    /// Channel to the platform updater worker (`desktop`). `None` when no
    /// worker was registered — Android, or a build without self-replace —
    /// which disables the in-app update and falls back to the release page.
    pub updater_tx: Option<std::sync::mpsc::Sender<UpdaterCommand>>,
    /// Release offered by the last update check, kept so a failed download
    /// can be retried and the release page can be opened from the failure.
    pub update_release: Option<NewRelease>,
    /// Number of database paths per (category, sub_category).
    pub sub_counts: HashMap<(Arc<str>, Arc<str>), usize>,
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
    pub fn checked_boxes(&self) -> Vec<(Rc<RefCell<bool>>, Arc<str>)> {
        self.categories
            .iter()
            .map(|c| {
                let b = c.is_checked();
                (Rc::new(RefCell::new(b)), Arc::clone(&c.name))
            })
            .collect()
    }

    #[cfg(windows)]
    pub fn from_database(
        database: CleanerDatabase,
        reg_database: RegistryDatabase,
        custom_database: Arc<[CustomCleaner]>,
    ) -> Self {
        let mut cat_to_subs: HashMap<Arc<str>, HashSet<Arc<str>>> = HashMap::new();
        let mut cat_has_empty: HashMap<Arc<str>, bool> = HashMap::new();
        let mut category_counts: HashMap<Arc<str>, usize> = HashMap::new();
        let mut sub_counts: HashMap<(Arc<str>, Arc<str>), usize> = HashMap::new();
        // Ensure all categories appear even if no sub_category
        database
            .for_each_index(|data| {
                cat_to_subs.entry(Arc::clone(&data.category)).or_default();
                cat_has_empty
                    .entry(Arc::clone(&data.category))
                    .or_insert(false);
                *category_counts
                    .entry(Arc::clone(&data.category))
                    .or_insert(0) += 1;
                let sub = effective_sub("", &data.sub_category);
                *sub_counts
                    .entry((Arc::clone(&data.category), Arc::clone(&sub)))
                    .or_insert(0) += 1;
                if !sub.is_empty() {
                    cat_to_subs.get_mut(&data.category).unwrap().insert(sub);
                } else {
                    *cat_has_empty.get_mut(&data.category).unwrap() = true;
                }
            })
            .expect("Failed to read cleaner database");
        for data in custom_database.iter() {
            cat_to_subs.entry(Arc::clone(&data.category)).or_default();
            cat_has_empty
                .entry(Arc::clone(&data.category))
                .or_insert(false);
            *category_counts
                .entry(Arc::clone(&data.category))
                .or_insert(0) += 1;
            let sub = effective_sub("", &data.sub_category);
            *sub_counts
                .entry((Arc::clone(&data.category), Arc::clone(&sub)))
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
                cat_to_subs.entry(Arc::clone(&data.category)).or_default();
                cat_has_empty
                    .entry(Arc::clone(&data.category))
                    .or_insert(false);
                *category_counts
                    .entry(Arc::clone(&data.category))
                    .or_insert(0) += 1;
                let sub = effective_sub("", &data.sub_category);
                *sub_counts
                    .entry((Arc::clone(&data.category), Arc::clone(&sub)))
                    .or_insert(0) += 1;
                if !sub.is_empty() {
                    cat_to_subs.get_mut(&data.category).unwrap().insert(sub);
                } else {
                    *cat_has_empty.get_mut(&data.category).unwrap() = true;
                }
            })
            .expect("Failed to read registry database");
        let mut options: Vec<Arc<str>> = cat_to_subs.keys().cloned().collect();

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
            let mut subs: Vec<Arc<str>> = cat_to_subs
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
                _ => cat.name.to_string(),
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
            updater_state: updater::new_state(),
            updater_tx: None,
            update_release: None,
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
    pub fn from_database(database: CleanerDatabase, custom_database: Arc<[CustomCleaner]>) -> Self {
        let mut cat_to_subs: HashMap<Arc<str>, HashSet<Arc<str>>> = HashMap::new();
        let mut cat_has_empty: HashMap<Arc<str>, bool> = HashMap::new();
        let mut category_counts: HashMap<Arc<str>, usize> = HashMap::new();
        let mut sub_counts: HashMap<(Arc<str>, Arc<str>), usize> = HashMap::new();
        database
            .for_each_index(|data| {
                cat_to_subs.entry(Arc::clone(&data.category)).or_default();
                cat_has_empty
                    .entry(Arc::clone(&data.category))
                    .or_insert(false);
                *category_counts
                    .entry(Arc::clone(&data.category))
                    .or_insert(0) += 1;
                let sub = effective_sub("", &data.sub_category);
                *sub_counts
                    .entry((Arc::clone(&data.category), Arc::clone(&sub)))
                    .or_insert(0) += 1;
                if !sub.is_empty() {
                    cat_to_subs.get_mut(&data.category).unwrap().insert(sub);
                } else {
                    *cat_has_empty.get_mut(&data.category).unwrap() = true;
                }
            })
            .expect("Failed to read cleaner database");
        for data in custom_database.iter() {
            cat_to_subs.entry(Arc::clone(&data.category)).or_default();
            cat_has_empty
                .entry(Arc::clone(&data.category))
                .or_insert(false);
            *category_counts
                .entry(Arc::clone(&data.category))
                .or_insert(0) += 1;
            let sub = effective_sub("", &data.sub_category);
            *sub_counts
                .entry((Arc::clone(&data.category), Arc::clone(&sub)))
                .or_insert(0) += 1;
            if !sub.is_empty() {
                cat_to_subs.get_mut(&data.category).unwrap().insert(sub);
            } else {
                *cat_has_empty.get_mut(&data.category).unwrap() = true;
            }
        }

        let mut options: Vec<Arc<str>> = cat_to_subs.keys().cloned().collect();

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
            let mut subs: Vec<Arc<str>> = cat_to_subs
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
                _ => cat.name.to_string(),
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
            updater_state: updater::new_state(),
            updater_tx: None,
            update_release: None,
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
    /// On Android window is fullscreen, so this is a no-op.
    pub(crate) fn set_window_size(&mut self, ctx: &egui::Context, size: egui::Vec2) {
        #[cfg(target_os = "android")]
        {
            let _ = (ctx, size);
            return;
        }
        #[cfg(not(target_os = "android"))]
        if self.last_inner_size != Some(size) {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
            self.last_inner_size = Some(size);
        }
    }

    /// Android system back / title bar back handler.
    /// Returns true if navigation happened.
    pub fn go_back(&mut self) -> bool {
        match self.current_page {
            Page::Results => {
                self.cleared_data = None;
                self.results_window_resized = false;
                self.current_page = Page::Main;
                true
            }
            Page::ProgramSelection | Page::Settings | Page::Clearing => {
                self.current_page = Page::Main;
                true
            }
            Page::Main => false,
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

    pub(crate) fn selected_map(&self) -> HashMap<Arc<str>, HashSet<Arc<str>>> {
        let mut map = HashMap::new();
        for cat in &self.categories {
            if !cat.selected.is_empty() {
                map.insert(Arc::clone(&cat.name), cat.selected.clone());
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

    /// True when a platform updater worker is registered *and* the release
    /// ships a binary this build can install on its own. Otherwise the app
    /// can only point the user at the release page.
    fn can_install(&self, release: &NewRelease) -> bool {
        self.updater_tx.is_some() && release.has_asset()
    }

    /// Starts the automatic update: hands `release` to the updater worker and
    /// shows the progress notification. Falls back to the "new version
    /// available" banner when the update cannot be installed in-app.
    fn start_update(&mut self, release: NewRelease) {
        if !self.can_install(&release) {
            self.notifications.push(UpdateNotification::new(release));
            return;
        }

        let version = release.version.clone();
        let total = release.asset_size;
        // Stored as well so the notification can retry the install and link to
        // the release page if the update fails.
        self.update_release = Some(release.clone());
        self.notifications.close(egui::Id::new(UPDATE_PROGRESS_ID));
        self.notifications
            .push(UpdateProgressNotification::new(self.updater_state.clone()));
        // Show the download starting immediately, so the notification is never
        // blank while the worker picks the command up.
        self.publish_stage(UpdateStage::Downloading {
            version,
            done: 0,
            total,
        });
        self.send_to_updater(UpdaterCommand::Install(release));
    }

    /// Publishes `stage` to the updater state shared with the worker thread.
    fn publish_stage(&self, stage: UpdateStage) {
        updater::publish(&self.updater_state, stage);
    }

    /// Sends `command` to the updater worker, reporting the failure in the
    /// notification instead of silently dropping the update.
    fn send_to_updater(&self, command: UpdaterCommand) {
        let Some(tx) = &self.updater_tx else {
            return;
        };
        let version = self
            .update_release
            .as_ref()
            .map(|r| r.version.clone())
            .unwrap_or_default();
        if let Err(e) = tx.send(command) {
            self.publish_stage(UpdateStage::Failed {
                version,
                error: format!("The updater stopped responding: {e}"),
            });
        }
    }

    /// Asks the worker to start a fresh copy of the app and quit this one, so
    /// the freshly installed version is the one that keeps running. The
    /// replacement is already on disk at this point (see `self_replace`), so
    /// the new process picks it up.
    ///
    /// The notification deliberately stays up: the worker either exits the
    /// process or publishes [`UpdateStage::RestartFailed`] into it, which
    /// turns into a "Try again" prompt the user can act on.
    fn request_restart(&mut self) {
        // Never restart in the middle of a download or install: the worker
        // owns the executable then, and quitting would leave a partial update.
        let stage = self.update_stage();
        if stage.is_running() {
            return;
        }
        let Some(tx) = &self.updater_tx else {
            return;
        };
        if let Err(e) = tx.send(UpdaterCommand::Restart) {
            // The update is installed, so this is a restart problem, not a
            // failed download: `RetryUpdate` must not offer to install again.
            self.publish_stage(UpdateStage::RestartFailed {
                version: stage.version().unwrap_or("?").to_string(),
                error: format!(
                    "The updater stopped responding: {e} Close and reopen Cross Cleaner \
                     to use the new version."
                ),
            });
        }
    }

    /// Retries a failed update from the start of the download.
    fn retry_update(&mut self) {
        let Some(release) = self.update_release.clone() else {
            return;
        };
        self.publish_stage(UpdateStage::Downloading {
            version: release.version.clone(),
            done: 0,
            total: release.asset_size,
        });
        self.send_to_updater(UpdaterCommand::Install(release));
    }

    /// Opens the release page of the pending update in the system browser.
    fn open_release_page(&mut self) {
        if let Some(release) = &self.update_release {
            crate::title_bar::open_in_browser(&release.url);
        }
    }

    /// Current stage of the automatic update.
    fn update_stage(&self) -> UpdateStage {
        updater::current(&self.updater_state)
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
                        #[cfg(target_os = "android")]
                        {
                            let pad = 40.0;
                            let rect = egui::Rect::from_min_size(
                                ui.cursor().min,
                                egui::vec2(ui.available_width(), pad),
                            );
                            ui.painter().rect_filled(rect, 0.0, ui.visuals().panel_fill);
                            ui.add_space(pad);
                        }

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
                        #[cfg(not(target_os = "android"))]
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
        #[cfg(not(target_os = "android"))]
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
                        self.start_update(release);
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
                NotificationAction::RestartApp => self.request_restart(),
                NotificationAction::RetryUpdate => self.retry_update(),
                NotificationAction::OpenReleasePage => self.open_release_page(),
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
        let show_back = matches!(
            self.current_page,
            Page::Results | Page::ProgramSelection | Page::Settings
        );
        let show_settings = self.current_page == Page::Main;
        #[cfg(target_os = "android")]
        {
            let pad = 40.0;
            let rect =
                egui::Rect::from_min_size(ui.cursor().min, egui::vec2(ui.available_width(), pad));
            ui.painter().rect_filled(rect, 0.0, ui.visuals().panel_fill);
            ui.add_space(pad);
        }
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
            self.go_back();
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
