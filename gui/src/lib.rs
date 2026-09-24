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

pub use app::{MyApp, Page};
pub use title_bar::TITLE_BAR_HEIGHT;

/// Returns number of columns for category grid.
/// Desktop: 3
/// Android landscape (width > height): 2
/// Android portrait: 1
pub fn category_columns(ctx: &eframe::egui::Context) -> usize {
    #[cfg(target_os = "android")]
    {
        let _ = ctx;
        2
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = ctx;
        3
    }
}

/// Helper for window height calculation based on dynamic columns.
pub fn category_rows(num_categories: usize, columns: usize) -> usize {
    if columns == 0 {
        return num_categories;
    }
    num_categories.div_ceil(columns)
}
