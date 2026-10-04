//! Frontend-agnostic application logic for Cross Cleaner.
//!
//! Everything here is state and behaviour only, with no widget toolkit and no
//! windowing system: the category/program selection model, the cleaning job,
//! the configuration file and the updater protocol. The frontends (`gui` on
//! egui/eframe and `tui` on ratatui) build their widgets on top of
//! [`AppState`] instead of re-implementing the selection rules, so the two
//! frontends can never drift apart.
//!
//! What stays in a frontend is presentation: widgets, textures, sounds,
//! notifications, window chrome and the key/mouse bindings.

pub mod app;
pub mod browser;
pub mod categories;
pub mod cleaning;
pub mod config;
pub mod updater;

pub use app::{AppState, CATEGORY_COLUMNS, CleanResult, Page};
pub use categories::{CategoryState, Toggle, effective_sub};
pub use config::AppConfig;