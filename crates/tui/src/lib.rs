//! Terminal frontend for Cross Cleaner.
//!
//! A ratatui rendering of the exact same [`appcore::AppState`] the window
//! frontend uses: the same five pages, the same selection rules, the same
//! cleaning job. Only the presentation and the key bindings live here.

pub mod app;
pub mod pages;
pub mod theme;

pub use app::{InputMode, Popup, PopupKind, Toast, TuiApp};
pub use theme::Theme;