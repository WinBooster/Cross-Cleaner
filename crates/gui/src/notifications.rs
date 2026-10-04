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

/// Opacity of a notification that has been up for `elapsed` seconds.
///
/// Split out because the difference between the two branches is the whole point:
/// a sticky notification has no deadline, and giving it one empties it while
/// leaving its border behind — `set_opacity` only reaches the content, the frame
/// is painted around it.
fn notification_alpha(sticky: bool, elapsed: f32) -> f32 {
    let fade_in = (elapsed / NOTIFICATION_FADE_SECS).clamp(0.0, 1.0);
    if sticky {
        return fade_in;
    }
    let remaining = NOTIFICATION_LIFETIME.as_secs_f32() - elapsed;
    fade_in.min(remaining / NOTIFICATION_FADE_SECS).clamp(0.0, 1.0)
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

    /// True when no notification is currently shown.
    pub fn is_empty(&self) -> bool {
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
                    let alpha = notification_alpha(n.inner.sticky(), elapsed);
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

/// The one and only update notification.
///
/// It has two shapes:
///
/// * **In progress** — created with [`Self::new`] when a platform updater
///   worker can install the release. It shows download/install/restart
///   progress and reads everything from the shared [`UpdateState`].
/// * **Release page** — created with [`Self::new_release_page`] when this build
///   cannot replace its own executable (Android, or a release without a
///   matching binary). It degrades to a plain "new version available" line.
///
/// It never expires on its own (see [`Notification::sticky`]): it carries the
/// only way forward out of an unfinished update. `Later` closes it.
///
/// Every action row ends with a **Change log** button, so what the user is
/// being offered to install is always one click away. There is no separate
/// close button: `Later` already dismisses the notification, and the two would
/// have been the same action twice.
pub struct UpdateProgressNotification {
    state: UpdateState,
    /// Set in the reduced shape: there is no worker, so the state stays
    /// [`UpdateStage::Idle`] and the release itself drives the text.
    fallback_release: Option<NewRelease>,
}

impl UpdateProgressNotification {
    /// Progress shape, driven by the updater worker.
    pub fn new(state: UpdateState) -> Self {
        Self {
            state,
            fallback_release: None,
        }
    }

    /// Reduced shape for builds that cannot install the update themselves.
    pub fn new_release_page(release: NewRelease) -> Self {
        Self {
            state: updater::new_state(),
            fallback_release: Some(release),
        }
    }

    /// Appends the changelog button at the end of an action row, so it always
    /// reads last: act on the update, then see what changed, then dismiss.
    fn changelog_button(ui: &mut egui::Ui, action: &mut NotificationAction) {
        if ui.button("Change log").clicked() {
            crate::sounds::click();
            *action = NotificationAction::ShowChangelog;
        }
    }

    /// The reduced shape: no install possible, so point at the release page.
    fn ui_release_page(&self, ui: &mut egui::Ui, release: &NewRelease) -> NotificationAction {
        let mut action = NotificationAction::None;
        // Plain left-to-right rows keep the frame exactly content-sized; the
        // anchored Area aligns it to the right window edge.
        ui.vertical(|ui| {
            ui.strong(format!("New version available: v{}", release.version));
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.button("Release page").clicked() {
                    crate::sounds::click();
                    action = NotificationAction::OpenReleasePage;
                }
                Self::changelog_button(ui, &mut action);
                // This shape no longer expires on its own, so it needs a way out
                // — otherwise a build that cannot self-update would keep the box
                // on screen for the rest of the session with nothing to do about
                // it.
                if ui.button("Later").clicked() {
                    crate::sounds::click();
                    action = NotificationAction::Close;
                }
            });
            // This shape is informational: nothing is downloading behind it.
            ui.small("This build cannot update itself — download it manually.");
        });
        action
    }
}

impl Notification for UpdateProgressNotification {
    fn id(&self) -> egui::Id {
        egui::Id::new(UPDATE_PROGRESS_ID)
    }

    fn sticky(&self) -> bool {
        // Always. This notification *is* the update, and it holds the only route
        // forward: restart into the new version, retry a failed download, or open
        // the release page when this build cannot install anything itself.
        //
        // Expiring it on a timer strands the user on the old binary — and if it
        // expires while the update never finished, the Retry and Release page
        // buttons go with it, so a transient network error becomes a dead end
        // with no explanation on screen. `Later` closes it deliberately.
        true
    }

    fn ui(&mut self, ui: &mut egui::Ui) -> NotificationAction {
        if let Some(release) = &self.fallback_release {
            return self.ui_release_page(ui, release);
        }
        let stage = updater::current(&self.state);
        let mut action = NotificationAction::None;

        ui.vertical(|ui| {
            ui.strong(updater::stage_heading(&stage));

            match &stage {
                // The window between pushing the notification and publishing the
                // first real stage. It has to render something: returning early
                // here left the frame with no content at all, which draws as a
                // bare outline.
                UpdateStage::Idle => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Checking for updates…");
                    });
                }
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
                        Self::changelog_button(ui, &mut action);
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
                        Self::changelog_button(ui, &mut action);
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
                        Self::changelog_button(ui, &mut action);
                    });
                }
            }
        });
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release() -> NewRelease {
        NewRelease {
            version: "2.1.0".to_string(),
            url: "https://github.com/WinBooster/Cross-Cleaner/releases/tag/v2.1.0".to_string(),
            asset_url: Some("https://example.invalid/app.exe".to_string()),
            asset_size: Some(1_024),
        }
    }

    fn at(stage: UpdateStage) -> UpdateProgressNotification {
        let state = updater::new_state();
        // `publish` reports whether the slot changed, and a fresh slot is
        // already `Idle`, so this is false for `UpdateStage::Idle`.
        updater::publish(&state, stage);
        UpdateProgressNotification::new(state)
    }

    #[test]
    fn both_shapes_share_one_id_so_they_never_stack() {
        let progress = UpdateProgressNotification::new(updater::new_state());
        let fallback = UpdateProgressNotification::new_release_page(release());
        assert_eq!(progress.id(), fallback.id());
    }

    #[test]
    fn it_never_expires_on_its_own() {
        // Every stage, in both shapes. This notification holds the only way out
        // of an unfinished update — restart, retry, or the release page — so a
        // timer that removes it strands the user on the old binary with the
        // explanation gone too.
        let stages = [
            UpdateStage::Idle,
            UpdateStage::Downloading {
                version: "2.1.0".to_string(),
                done: 10,
                total: Some(100),
            },
            UpdateStage::Installing {
                version: "2.1.0".to_string(),
            },
            UpdateStage::Installed {
                version: "2.1.0".to_string(),
            },
            UpdateStage::RestartFailed {
                version: "2.1.0".to_string(),
                error: "elevation required".to_string(),
            },
            // The stage that used to auto-hide, taking `Retry` with it.
            UpdateStage::Failed {
                version: "2.1.0".to_string(),
                error: "connection reset".to_string(),
            },
        ];
        for stage in stages {
            assert!(
                at(stage).sticky(),
                "{stage:?} must not expire on its own",
            );
        }
        assert!(UpdateProgressNotification::new_release_page(release()).sticky());
    }

    /// The frame's fill and stroke are painted around `set_opacity`, so a sticky
    /// notification faded out by the auto-hide timer left an empty box: the border
    /// at full strength and no content at all. Alpha is therefore a function of
    /// `sticky` and elapsed time alone, with no deadline in it.
    #[test]
    fn a_sticky_notification_fades_in_and_then_holds_its_opacity() {
        assert_eq!(notification_alpha(true, 0.0), 0.0, "starts invisible");
        assert_eq!(notification_alpha(true, 0.15), 0.5, "fades in");
        assert_eq!(notification_alpha(true, 0.3), 1.0, "fully visible");
        // Long past the auto-hide lifetime: still fully visible.
        assert_eq!(notification_alpha(true, 600.0), 1.0, "holds");
    }

    /// The non-sticky path still fades out, and is gone by the time the manager
    /// drops it.
    #[test]
    fn a_timed_notification_still_fades_out() {
        assert_eq!(notification_alpha(false, 0.15), 0.5);
        assert_eq!(notification_alpha(false, 1.0), 1.0);
        assert_eq!(
            notification_alpha(false, NOTIFICATION_LIFETIME.as_secs_f32() - 0.1),
            0.0,
            "gone by the deadline",
        );
    }
}
