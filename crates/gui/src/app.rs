//! egui application state: the shared [`AppState`] plus everything that only
//! exists inside a window (textures, notifications, the changelog viewport,
//! the update banner and the taskbar indicator).

pub use appcore::app::{AppState, Page};
use database::structures::CustomCleaner;
use database::version::{Changelog, NewRelease};

use eframe::egui;
use std::sync::Arc;

use crate::notifications::{
    NotificationAction, NotificationManager, UPDATE_PROGRESS_ID, UpdateProgressNotification,
};
use crate::taskbar;
use crate::title_bar::{paint_window_border, title_bar};
use crate::updater::{self, UpdateStage, UpdateState, UpdaterCommand};

/// The window frontend's application object.
pub struct MyApp {
    /// Selection, program list, cleaning job — everything both frontends share.
    pub state: AppState,

    // --- window chrome ---
    /// Last inner size sent to the viewport, used to avoid re-issuing
    /// `ViewportCommand::InnerSize` every frame (which forces a repaint).
    pub last_inner_size: Option<egui::Vec2>,
    /// Set once the results window has been resized for the current run.
    pub results_window_resized: bool,
    /// Row of the results report whose deleted paths are on screen, `None` while
    /// the report itself is.
    ///
    /// A row index rather than a copy of the paths: the paths live in the
    /// finished run, which is behind an `Arc`, and a second copy here could
    /// only ever disagree with it.
    pub results_detail: Option<usize>,
    /// Windows taskbar progress (no-op on other platforms).
    pub taskbar: Option<taskbar::TaskbarProgress>,

    // --- textures ---
    pub menu_texture: Option<egui::TextureHandle>,
    pub icon_texture: Option<egui::TextureHandle>,
    pub settings_texture: Option<egui::TextureHandle>,

    // --- updates and notifications ---
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
    /// All currently visible notifications (update banner, etc.).
    pub notifications: NotificationManager,

    // --- changelog ---
    /// Shared slot filled by the background changelog fetch.
    pub changelog: Option<Arc<std::sync::Mutex<Option<Changelog>>>>,
    /// Background fetch of the changelog.
    pub changelog_handle: Option<std::thread::JoinHandle<()>>,
    /// Set to false when the user closes the changelog viewport window.
    pub changelog_open: Option<Arc<std::sync::Mutex<bool>>>,
    /// True while the changelog window is open.
    pub show_changelog: bool,
}

impl MyApp {
    #[cfg(windows)]
    pub fn from_database(
        database: database::cleaner_database::CleanerDatabase,
        reg_database: database::registry_database::RegistryDatabase,
        custom_database: Arc<[CustomCleaner]>,
    ) -> Self {
        Self::new(AppState::from_database(
            database,
            reg_database,
            custom_database,
        ))
    }

    #[cfg(not(windows))]
    pub fn from_database(
        database: database::cleaner_database::CleanerDatabase,
        custom_database: Arc<[CustomCleaner]>,
    ) -> Self {
        Self::new(AppState::from_database(database, custom_database))
    }

    /// Wraps the shared state in the window-only plumbing.
    fn new(state: AppState) -> Self {
        Self {
            state,
            last_inner_size: None,
            results_window_resized: false,
            results_detail: None,
            taskbar: None,
            menu_texture: None,
            icon_texture: None,
            settings_texture: None,
            update_receiver: None,
            updater_state: updater::new_state(),
            updater_tx: None,
            update_release: None,
            notifications: NotificationManager::default(),
            changelog: None,
            changelog_handle: None,
            changelog_open: None,
            show_changelog: false,
        }
    }

    /// Sends `ViewportCommand::InnerSize` only when the requested size
    /// actually changed. `send_viewport_cmd` triggers an immediate repaint, so
    /// calling it unconditionally every frame would keep the app rendering at
    /// full frame rate even while idle.
    /// On Android the window is fullscreen, so this is a no-op.
    pub(crate) fn set_window_size(&mut self, ctx: &egui::Context, size: egui::Vec2) {
        #[cfg(target_os = "android")]
        {
            let _ = (ctx, size);
        }
        #[cfg(not(target_os = "android"))]
        if self.last_inner_size != Some(size) {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
            self.last_inner_size = Some(size);
        }
    }

    /// Opens the changelog window (fetch starts in `show_changelog_window`).
    fn open_changelog(&mut self) {
        self.show_changelog = true;
    }

    /// Answers a back request from the title bar, the platform gesture or the
    /// hardware key. Returns `true` when the app handled it in-app.
    ///
    /// The deleted-path list is one level below the report, so the first back
    /// press returns to the report rather than dropping the whole results page
    /// — the same order the terminal frontend's overlays use.
    pub fn go_back(&mut self) -> bool {
        if self.results_detail.take().is_some() {
            return true;
        }
        self.state.go_back()
    }

    /// True when a platform updater worker is registered *and* the release
    /// ships a binary this build can install on its own. Otherwise the app
    /// can only point the user at the release page.
    fn can_install(&self, release: &NewRelease) -> bool {
        self.updater_tx.is_some() && release.has_asset()
    }

    /// Starts the automatic update: hands `release` to the updater worker and
    /// shows the progress notification.
    ///
    /// When this build cannot replace its own executable (Android, or a release
    /// without a matching binary) the same notification is shown in its
    /// reduced form, offering the changelog and the release page instead.
    fn start_update(&mut self, release: NewRelease) {
        if !self.can_install(&release) {
            self.notifications
                .push(UpdateProgressNotification::new_release_page(release));
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
        let stage = updater::current(&self.updater_state);
        if stage.is_running() {
            return;
        }
        let Some(tx) = &self.updater_tx else {
            return;
        };
        if let Err(e) = tx.send(UpdaterCommand::Restart) {
            // The update is installed, so this is a restart problem, not a
            // failed download: "Retry" must not offer to install again.
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

    /// Draws the changelog viewport ("What's New") as a separate native window.
    /// The changelog is shared via `Arc<Mutex<Option<Changelog>>>` so the
    /// background fetch fills it while the window is open.
    fn show_changelog_window(&mut self, ctx: &egui::Context) {
        if !self.show_changelog {
            return;
        }

        // Spawn the fetch once per open session.
        if self.changelog.is_none() && self.changelog_handle.is_none() {
            let current_version = database::get_version().to_string();
            let shared: Arc<std::sync::Mutex<Option<Changelog>>> =
                Arc::new(std::sync::Mutex::new(None));
            let shared_for_thread = shared.clone();
            self.changelog_handle = Some(std::thread::spawn(move || {
                let fetched =
                    database::version::fetch_changelogs(&current_version).unwrap_or_default();
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

        // Drain everything that is ready, but repaint on a slower cadence:
        // cleaning can emit many messages per second, and an immediate repaint
        // for each would keep the software renderer busy.
        if self.state.drain_progress() {
            // Mirror the cleaning progress on the Windows taskbar.
            if self.state.total_tasks > 0
                && let Some(taskbar) = &self.taskbar
            {
                taskbar.set_progress(
                    self.state.current_task as u64,
                    self.state.total_tasks as u64,
                );
            }
        }

        if self.state.poll_result() {
            // Cleaning is done: clear the taskbar progress indicator.
            if let Some(taskbar) = &self.taskbar {
                taskbar.remove();
            }
            self.results_window_resized = false;
            // A new run replaces the report, so a path list pointing into the
            // old one has nothing left to describe.
            self.results_detail = None;
            crate::sounds::done();
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
                crate::icons::load_icon_color_image(),
                egui::TextureOptions::LINEAR,
            ));
        }
        if self.settings_texture.is_none() {
            self.settings_texture = Some(ctx.load_texture(
                "settings",
                crate::icons::load_asset_image(crate::icons::SETTINGS_BYTES),
                egui::TextureOptions::LINEAR,
            ));
        }
        let show_back = self.state.current_page.has_back();
        let show_settings = self.state.current_page == Page::Main;
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
            &self.state.window_title,
            self.icon_texture.as_ref(),
            show_back,
            show_settings,
            self.settings_texture.as_ref(),
        );
        if back_clicked {
            self.go_back();
        }
        if settings_clicked {
            self.state.current_page = Page::Settings;
        }
        let inner_margin = 8;
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .inner_margin(egui::Margin::same(inner_margin))
                    .fill(ui.visuals().panel_fill),
            )
            .show(ui, |ui| {
                match self.state.current_page {
                    Page::Clearing => self.render_clearing(&ctx, ui),
                    Page::Results => {
                        // The deleted-path list is a level below the report,
                        // so it is drawn first and the report stays behind it.
                        if !self.render_results_details(ui)
                            // No result data yet: fall through to the main page.
                            && !self.render_results(&ctx, ui)
                        {
                            self.render_main(&ctx, ui);
                        }
                    }
                    Page::ProgramSelection => self.render_program_selection(&ctx, ui),
                    Page::Settings => self.render_settings(&ctx, ui),
                    Page::Main => self.render_main(&ctx, ui),
                }
            });
    }
}
