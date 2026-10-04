//! Category selection widgets for the window frontend.
//!
//! The selection rules themselves live in [`appcore::categories`]; this module
//! only adds the egui widget that renders a category as a tristate checkbox.

pub use appcore::categories::{CategoryState, Toggle, effective_sub};

use eframe::egui;

/// Tristate checkbox with square (filled rect) for indeterminate state.
/// Returns response and whether state changed via click.
///
/// The toggle itself is *not* applied here: the caller routes it through
/// [`appcore::AppState::toggle_category`] so both frontends share one
/// implementation.
pub fn tristate_checkbox(
    ui: &mut egui::Ui,
    checked: bool,
    indeterminate: bool,
    text: &str,
) -> (egui::Response, bool) {
    // Use a mutable dummy bool for Checkbox widget (it will toggle on click)
    let mut dummy = checked;
    // We don't rely on Checkbox's indeterminate painting (hline); we will paint square ourselves.
    // So pass false to avoid double paint, and we handle visual manually.
    let mut response = ui.add(egui::Checkbox::new(&mut dummy, text));
    // If indeterminate, we need to paint square overlay manually
    if indeterminate && ui.is_rect_visible(response.rect) {
        // Calculate icon rect similar to Checkbox impl
        let icon_width = ui.spacing().icon_width;
        let rect = response.rect;
        // icon is at left side, centered vertically
        let icon_rect = egui::Rect::from_min_size(
            egui::pos2(rect.min.x, rect.center().y - icon_width / 2.0),
            egui::vec2(icon_width, icon_width),
        );
        // small inner square (shrink)
        let small_rect = icon_rect.shrink(4.0);
        let visuals = ui.style().interact(&response);
        // Use bg_fill for outer, but for indeterminate we fill inner square with fg color
        // Mimic checkbox bg
        ui.painter()
            .rect_filled(small_rect, 1.0, visuals.fg_stroke.color);
    }
    let clicked = response.clicked();
    // When clicked, dummy has been toggled (!checked) but for indeterminate we want custom toggle handling outside
    // Return whether clicked
    response.mark_changed();
    (response, clicked)
}