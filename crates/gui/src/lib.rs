//! Window frontend: egui/eframe widgets on top of the shared [`appcore`] logic.
//!
//! The split is deliberate — every selection rule, the program list and the
//! cleaning job live in `appcore`, and the `tui` crate renders the very same
//! state with ratatui. Anything in here that is not a widget is a
//! window-specific concern.

#![allow(dead_code)]

pub mod app;
pub mod categories;
pub mod config;
pub mod display;
pub mod icons;
pub mod notifications;
pub mod pages;
pub mod sounds;
pub mod taskbar;
pub mod title_bar;
pub mod updater;

pub use app::{AppState, MyApp, Page};
pub use categories::{CategoryState, Toggle};
pub use title_bar::TITLE_BAR_HEIGHT;
pub use updater::UpdaterCommand;

/// Number of category checkboxes per row.
pub use appcore::app::CATEGORY_COLUMNS;

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

#[cfg(test)]
mod tests {
    use super::*;
    use database::cleaner_database::CleanerDatabase;
    use std::sync::Arc;

    #[test]
    fn category_rows_rounds_up() {
        assert_eq!(category_rows(5, 2), 3);
        assert_eq!(category_rows(4, 2), 2);
        // A zero column count must not divide by zero.
        assert_eq!(category_rows(3, 0), 3);
    }

    /// A finished run, so the results page is the one on screen.
    fn app_with_a_finished_run() -> MyApp {
        let database = CleanerDatabase::from_vec(Vec::new());
        let custom = Arc::from(Vec::new());
        #[cfg(windows)]
        let mut app = MyApp::from_database(
            database,
            database::registry_database::RegistryDatabase::from_vec(Vec::new()),
            custom,
        );
        #[cfg(not(windows))]
        let mut app = MyApp::from_database(database, custom);
        app.state.cleared_data = Some((0, 0, 0, Arc::from(Vec::new())));
        app.state.current_page = Page::Results;
        app
    }

    /// The deleted-path list is one level below the report: the first back press
    /// returns to the report, and only the next one leaves the results page.
    /// A single press that did both would throw away the report the user came
    /// from, and the Back button in the list would be a lie.
    #[test]
    fn back_steps_out_of_the_path_list_before_out_of_the_report() {
        let mut app = app_with_a_finished_run();
        app.results_detail = Some(0);

        assert!(app.go_back(), "the app answered it");
        assert_eq!(app.results_detail, None, "the list closed");
        assert_eq!(
            app.state.current_page,
            Page::Results,
            "and the report is still there",
        );

        assert!(app.go_back());
        assert_eq!(app.state.current_page, Page::Main);
        // The main page has nothing to go back to, which is how Android decides
        // whether to cancel the system close.
        assert!(!app.go_back());
    }
}
