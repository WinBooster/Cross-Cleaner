// ============================================================================
// Notification system
// ============================================================================
// Any widget can become a floating notification: implement `Notification`
// and push it into `MyApp::notifications`. Notifications are stacked on the
// right side above everything else, fade in smoothly and auto-hide after
// `NOTIFICATION_LIFETIME` — unless they opt out with [`Notification::sticky`].

use eframe::egui;

use database::version::NewRelease;

use crate::updater::{self, UpdateStage, UpdateState};

/// How long a notification stays visible before it starts fading out.
const NOTIFICATION_LIFETIME: std::time::Duration = std::time::Duration::from_secs(15);
/// Duration of the fade-in and fade-out animations (seconds).
const NOTIFICATION_FADE_SECS: f32 = 0.3;

/// Action a notification asks the app to perform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationAction {
    None,
    /// Close (dismiss) this notification.
    Close,
    /// Open the changelog window.
    ShowChangelog,
    /// Relaunch the app so the installed update is picked up.
    RestartApp,
    /// Download and install the update again after a failure.
    RetryUpdate,
    /// Open the release page of the new version in the browser.
    OpenReleasePage,
}

/// A single floating notification widget.
pub trait Notification {
    /// Stable identity, used for de-duplication and dismissal.
    fn id(&self) -> egui::Id;
    /// Draws the notification content. May return an action for the app.
    fn ui(&mut self, ui: &mut egui::Ui) -> NotificationAction;
    /// Notifications that must not disappear on their own (e.g. the update
    /// dialog waiting for a restart) return `true`; they only fade in and
    /// stay until the app closes them.
    fn sticky(&self) -> bool {
        false
    }
}

/// A notification together with the moment it appeared.
struct TimedNotification {
    shown_at: std::time::Instant,
    inner: Box<dyn Notification>,
}

/// Holds and renders all active notifications.
#[derive(Default)]
pub struct NotificationManager {
    active: Vec<TimedNotification>,
}

impl NotificationManager {
    /// Shows a new notification unless one with the same id is already visible.
    pub fn push(&mut self, notification: impl Notification + 'static) {
        let id = notification.id();
        if self.active.iter().any(|n| n.inner.id() == id) {
            return;
        }
        self.active.push(TimedNotification {
            shown_at: std::time::Instant::now(),
            inner: Box::new(notification),
        });
    }

    /// Dismisses the notification with the given id.
    pub fn close(&mut self, id: egui::Id) {
        self.active.retain(|n| n.inner.id() != id);
    }

    fn is_empty(&self) -> bool {
        self.active.is_empty()
    }

    /// Removes expired notifications and draws the rest with fade animations.
    /// Returns the actions requested by the notifications.
    pub fn update(&mut self, ctx: &egui::Context) -> Vec<(egui::Id, NotificationAction)> {
        self.active
            .retain(|n| n.inner.sticky() || n.shown_at.elapsed() < NOTIFICATION_LIFETIME);
        if self.is_empty() {
            return Vec::new();
        }

        let mut actions = Vec::new();
        // While a notification is fading in or out we need smooth frames;
        // otherwise a slow cadence is enough to advance the auto-hide
        // countdown without burning CPU. Sticky notifications own their own
        // repaints (the download bar animates), so they never count as
        // "animating" here — otherwise they would hold the app at full frame
        // rate for as long as they are on screen.
        let animating = self.active.iter().any(|n| {
            if n.inner.sticky() {
                return false;
            }
            let elapsed = n.shown_at.elapsed().as_secs_f32();
            let remaining = NOTIFICATION_LIFETIME.as_secs_f32() - elapsed;
            elapsed < NOTIFICATION_FADE_SECS || remaining < NOTIFICATION_FADE_SECS
        });
        egui::Area::new(egui::Id::new("notification_stack"))
            .order(egui::Order::Foreground)
            .anchor(
                egui::Align2::RIGHT_TOP,
                [-10.0, crate::TITLE_BAR_HEIGHT + 8.0],
            )
            .show(ctx, |ui| {
                for (index, n) in self.active.iter_mut().enumerate() {
                    if index > 0 {
                        ui.add_space(6.0);
                    }
                    let elapsed = n.shown_at.elapsed().as_secs_f32();
                    let remaining = NOTIFICATION_LIFETIME.as_secs_f32() - elapsed;
                    let alpha = (elapsed / NOTIFICATION_FADE_SECS)
                        .min(remaining / NOTIFICATION_FADE_SECS)
                        .clamp(0.0, 1.0);
                    let inner = egui::Frame::new()
                        .corner_radius(4.0)
                        .inner_margin(egui::Margin::symmetric(10, 8))
                        .fill(ui.visuals().window_fill)
                        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                        .show(ui, |ui| {
                            ui.set_opacity(alpha);
                            // Notifications hug the right edge; cap the width
                            // so long content cannot stretch the area across
                            // the whole window. Otherwise the frame hugs the
                            // content exactly.
                            ui.set_max_width(280.0);
                            let action = n.inner.ui(ui);
                            (n.inner.id(), action)
                        })
                        .inner;
                    if inner.1 != NotificationAction::None {
                        actions.push(inner);
                    }
                }
            });
        // Keep animating fades and the auto-hide countdown.
        let delay = if animating {
            std::time::Duration::from_millis(16)
        } else {
            std::time::Duration::from_millis(100)
        };
        ctx.request_repaint_after(delay);
        actions
    }
}

/// Id of the auto-update progress notification, used to dismiss it.
pub const UPDATE_PROGRESS_ID: &str = "update_progress_notification";

/// Progress of the automatic update: downloading, installing, and — once the
/// new version is on disk — the prompt to restart the app.
///
/// Unlike the other notifications this one is sticky: the update must not be
/// missed, and it has to survive a download that takes minutes. It reads
/// everything it shows from the shared [`UpdateState`], so the updater worker
/// stays the single source of truth and the UI never shows a stale stage.
pub struct UpdateProgressNotification {
    state: UpdateState,
}

impl UpdateProgressNotification {
    pub fn new(state: UpdateState) -> Self {
        Self { state }
    }
}

impl Notification for UpdateProgressNotification {
    fn id(&self) -> egui::Id {
        egui::Id::new(UPDATE_PROGRESS_ID)
    }

    fn sticky(&self) -> bool {
        true
    }

    fn ui(&mut self, ui: &mut egui::Ui) -> NotificationAction {
        let stage = updater::current(&self.state);
        // Nothing to report before a release was found.
        if matches!(stage, UpdateStage::Idle) {
            return NotificationAction::Close;
        }
        let mut action = NotificationAction::None;

        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.strong(heading(&stage));
                // The X is only offered once the worker is done with the
                // update: closing mid-download would leave the user with an
                // old binary and no way back to the new one.
                if !stage.is_running() && ui.small_button("x").clicked() {
                    crate::sounds::click();
                    action = NotificationAction::Close;
                }
            });

            match &stage {
                UpdateStage::Idle => {}
                UpdateStage::Downloading {
                    version,
                    done,
                    total,
                } => {
                    ui.label(format!(
                        "Downloading v{version} — {}",
                        updater::format_progress(*done, *total)
                    ));
                    let time = ui.input(|i| i.time);
                    ui.add(
                        egui::ProgressBar::new(updater::progress_fraction(*done, *total, time))
                            .desired_height(8.0),
                    );
                    // The bar animates while the length is unknown, so keep
                    // frames coming; 100ms is enough for both cases.
                    ui.ctx()
                        .request_repaint_after(std::time::Duration::from_millis(100));
                }
                UpdateStage::Installing { version } => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(format!("Installing v{version}…"));
                    });
                    ui.label("The new version starts after a restart.");
                    ui.ctx()
                        .request_repaint_after(std::time::Duration::from_millis(100));
                }
                UpdateStage::Installed { version } => {
                    ui.label(format!(
                        "v{version} is installed. Restart Cross Cleaner to use it?"
                    ));
                    ui.horizontal(|ui| {
                        if ui.button("Restart now").clicked() {
                            crate::sounds::click();
                            action = NotificationAction::RestartApp;
                        }
                        if ui.button("Later").clicked() {
                            crate::sounds::click();
                            action = NotificationAction::Close;
                        }
                    });
                }
                UpdateStage::RestartFailed { version, error } => {
                    // The new version is already on disk, so retrying the
                    // download would be pointless — only the restart failed.
                    ui.label(format!("v{version} is installed, but restarting failed."));
                    ui.add(egui::Label::new(error.as_str()).wrap());
                    ui.horizontal(|ui| {
                        if ui.button("Try again").clicked() {
                            crate::sounds::click();
                            action = NotificationAction::RestartApp;
                        }
                        if ui.button("Later").clicked() {
                            crate::sounds::click();
                            action = NotificationAction::Close;
                        }
                    });
                }
                UpdateStage::Failed { version, error } => {
                    ui.label(format!("Update to v{version} failed:"));
                    // Installer errors are sentences, not identifiers, so wrap.
                    ui.add(egui::Label::new(error.as_str()).wrap());
                    ui.horizontal(|ui| {
                        if ui.button("Retry").clicked() {
                            crate::sounds::click();
                            action = NotificationAction::RetryUpdate;
                        }
                        if ui.button("Release page").clicked() {
                            crate::sounds::click();
                            action = NotificationAction::OpenReleasePage;
                        }
                    });
                }
            }
        });
        action
    }
}

/// First line of the update notification for `stage`.
fn heading(stage: &UpdateStage) -> String {
    let version = stage.version().unwrap_or("?");
    match stage {
        UpdateStage::Idle => String::new(),
        UpdateStage::Downloading { .. } => format!("Updating to v{version}"),
        UpdateStage::Installing { .. } => format!("Installing v{version}"),
        UpdateStage::Installed { .. } | UpdateStage::RestartFailed { .. } => {
            format!("v{version} installed")
        }
        UpdateStage::Failed { .. } => format!("Update to v{version} failed"),
    }
}

/// "New version is available" notification with Download / Changelog buttons.
/// Used as a fallback when no updater worker is registered (e.g. Android, or a
/// release without a binary for this platform).
pub struct UpdateNotification {
    release: NewRelease,
}

impl UpdateNotification {
    pub fn new(release: NewRelease) -> Self {
        Self { release }
    }
}

impl Notification for UpdateNotification {
    fn id(&self) -> egui::Id {
        egui::Id::new("update_notification")
    }

    fn ui(&mut self, ui: &mut egui::Ui) -> NotificationAction {
        let mut action = NotificationAction::None;
        ui.vertical(|ui| {
            // Plain left-to-right rows keep the frame exactly content-sized;
            // the anchored Area aligns it to the right window edge.
            ui.horizontal(|ui| {
                ui.strong(format!("New version available: v{}", self.release.version));
                if ui.small_button("x").clicked() {
                    crate::sounds::click();
                    action = NotificationAction::Close;
                }
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                // Left-to-right so the buttons read Download, Changelog; the
                // row hugs the left edge of the (content-sized) notification.
                if ui.button("Download").clicked() {
                    crate::sounds::click();
                    // eframe's native backend ignores egui's OpenUrl command,
                    // so open the release page through the system browser.
                    crate::title_bar::open_in_browser(&self.release.url);
                }
                if ui.button("Changelog").clicked() {
                    crate::sounds::click();
                    action = NotificationAction::ShowChangelog;
                }
            });
        });
        action
    }
}
