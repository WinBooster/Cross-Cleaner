// ============================================================================
// Notification system
// ============================================================================
// Any widget can become a floating notification: implement `Notification`
// and push it into `MyApp::notifications`. Notifications are stacked on the
// right side above everything else, fade in smoothly and auto-hide after
// `NOTIFICATION_LIFETIME`.

use eframe::egui;

use database::version::NewRelease;

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
}

/// A single floating notification widget.
pub trait Notification {
    /// Stable identity, used for de-duplication and dismissal.
    fn id(&self) -> egui::Id;
    /// Draws the notification content. May return an action for the app.
    fn ui(&mut self, ui: &mut egui::Ui) -> NotificationAction;
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
            .retain(|n| n.shown_at.elapsed() < NOTIFICATION_LIFETIME);
        if self.is_empty() {
            return Vec::new();
        }

        let mut actions = Vec::new();
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
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
        actions
    }
}

/// "New version is available" notification with Download / Changelog buttons.
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
                    action = NotificationAction::Close;
                }
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                // Left-to-right so the buttons read Download, Changelog; the
                // row hugs the left edge of the (content-sized) notification.
                if ui.button("Download").clicked() {
                    ui.ctx()
                        .open_url(egui::OpenUrl::new_tab(self.release.url.clone()));
                }
                if ui.button("Changelog").clicked() {
                    action = NotificationAction::ShowChangelog;
                }
            });
        });
        action
    }
}
