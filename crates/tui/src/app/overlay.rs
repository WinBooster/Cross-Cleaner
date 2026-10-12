//! The values a page can be covered by.
//!
//! Each overlay the terminal frontend draws — where egui would put a window —
//! needs three things: the state of what is open, the rectangle it owns once
//! drawn, and a way to be dismissed. The states live here; what they are drawn
//! as lives in [`super::render`], and the keys that move them in
//! [`super::input`].

use std::time::{Duration, Instant};

use ratatui::style::Style;
use ratatui::widgets::ListState;

use crate::theme::Theme;

/// How long a transient toast stays on screen.
pub(super) const TOAST_LIFETIME: Duration = Duration::from_secs(6);

/// Where the program search field gets its keystrokes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    /// Keystrokes drive the list.
    Normal,
    /// Keystrokes are appended to the query.
    Editing,
}

/// What the open overlay is editing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupKind {
    /// Subcategories of the category at this index.
    Category(usize),
    /// Categories of the program at this index.
    Program(usize),
}

/// A centered overlay listing checkable entries.
pub struct Popup {
    pub kind: PopupKind,
    pub title: String,
    pub list: ListState,
    /// `(label, selected)` pairs, rebuilt from `AppState` on every render so
    /// they can never go stale.
    pub items: Vec<(String, bool)>,
}

/// The overlay listing which paths one results row deleted, and how much each of
/// them freed.
///
/// It stores the row rather than the paths themselves: the paths live in the
/// finished run, which the results page already owns, and a second copy here
/// would be a list that can go stale.
pub struct PathDetails {
    /// Row of the results table this describes.
    pub row: usize,
    /// First path line on screen.
    pub scroll: usize,
    /// Largest usable `scroll`, recomputed on every render from the number of
    /// paths and the height the overlay got.
    pub max_scroll: usize,
}

/// A transient status line shown in place of the key hints.
pub struct Toast {
    pub message: String,
    pub style: Style,
    pub(super) until: Instant,
}

impl Toast {
    pub(super) fn new(message: impl Into<String>, style: Style) -> Self {
        Self {
            message: message.into(),
            style,
            until: Instant::now() + TOAST_LIFETIME,
        }
    }

    pub(super) fn info(message: impl Into<String>) -> Self {
        Self::new(message, Style::default().fg(Theme::TEXT))
    }

    pub(super) fn warn(message: impl Into<String>) -> Self {
        Self::new(message, Style::default().fg(Theme::WARN))
    }
}
