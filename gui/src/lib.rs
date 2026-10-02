#![allow(dead_code)]

pub mod app;
pub mod categories;
pub mod cleaning;
pub mod config;
pub mod icons;
pub mod notifications;
pub mod pages;
pub mod sounds;
pub mod taskbar;
pub mod title_bar;
pub mod updater;

pub use app::{MyApp, Page};
pub use title_bar::TITLE_BAR_HEIGHT;
pub use updater::UpdaterCommand;

/// Number of category checkboxes per row.
pub const CATEGORY_COLUMNS: usize = 2;

/// Returns number of columns for category grid.
/// Always 2 categories per row, so the grid stays readable (and the
/// subcategory popups do not overlap the pinned Next button) on both
/// desktop windows and small phone/tablet screens.
pub fn category_columns(ctx: &eframe::egui::Context) -> usize {
    let _ = ctx;
    CATEGORY_COLUMNS
}

/// Helper for window height calculation based on dynamic columns.
pub fn category_rows(num_categories: usize, columns: usize) -> usize {
    if columns == 0 {
        return num_categories;
    }
    num_categories.div_ceil(columns)
}

/// Tallest window height (title bar included) that still fits on the
/// current screen, with a small margin for the window manager frame.
/// Used to cap the dynamic window height so the bottom action button
/// (Next / Start Cleaning) is never pushed off a small screen.
pub fn max_window_height(ctx: &eframe::egui::Context) -> f32 {
    let screen_height = ctx.content_rect().height();
    if screen_height <= 0.0 {
        // Screen size not reported (e.g. headless test context).
        return 500.0;
    }
    (screen_height - 40.0).max(200.0)
}
