//! Page renderers, one module per `Page` variant.

mod clearing;
mod main;
mod program_selection;
mod results;
mod settings;

use eframe::egui;

/// Height of the main action button (Next / Start Cleaning).
pub(crate) const BUTTON_HEIGHT: f32 = 25.0;

/// Empty space kept below the action button. On Android the window goes
/// fullscreen under the system navigation bar, so the button is lifted by
/// this inset instead of being hidden behind / flush against the bar.
#[cfg(target_os = "android")]
pub(crate) const BOTTOM_INSET: f32 = 40.0;
#[cfg(not(target_os = "android"))]
pub(crate) const BOTTOM_INSET: f32 = 0.0;

/// Splits the remaining page area into a list rect that gets the scrollable
/// content and a button rect pinned to the bottom of the window (minus
/// [`BOTTOM_INSET`]), so the action button stays visible on small screens
/// while the list scrolls above.
pub(crate) fn split_list_and_button(ui: &egui::Ui) -> (egui::Rect, egui::Rect) {
    let available = ui.available_rect_before_wrap();
    // Reserve the bottom inset so the button is not flush with the screen edge.
    let available = egui::Rect::from_min_max(
        available.min,
        egui::pos2(available.max.x, (available.max.y - BOTTOM_INSET).max(available.min.y)),
    );
    let spacing = ui.spacing().item_spacing.y;
    let button_height = BUTTON_HEIGHT.min(available.height());
    let list_height = (available.height() - button_height - spacing)
        .max(20.0)
        .min(available.height().max(20.0));
    let list_rect =
        egui::Rect::from_min_size(available.min, egui::vec2(available.width(), list_height));
    let button_rect = egui::Rect::from_min_max(
        egui::pos2(list_rect.min.x, list_rect.max.y + spacing),
        egui::pos2(list_rect.max.x, list_rect.max.y + spacing + button_height),
    );
    (list_rect, button_rect)
}

/// Lays out `add_contents` inside an exact `rect` instead of letting it
/// follow the cursor.
pub(crate) fn ui_at_rect<R>(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), add_contents)
        .inner
}
