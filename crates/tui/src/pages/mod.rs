//! Page renderers, one module per [`appcore::Page`] variant.
//!
//! Each page follows the shape of its counterpart in `gui::pages`: the same
//! content, laid out for a character grid.

pub mod clearing;
pub mod main;
pub mod program_selection;
pub mod results;
pub mod settings;
pub mod update;

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::theme::Theme;

/// Height of a pinned action button.
///
/// One row, because that is all a label is. A taller area only left the rows
/// below it as bare background, which read as dead space hanging under the
/// button rather than as part of it.
pub(crate) const BUTTON_HEIGHT: u16 = 1;

/// Splits `area` into a scrolling body, a blank separator and the pinned bottom
/// button row.
pub(crate) fn split_body(area: Rect, gap: u16) -> (Rect, Rect) {
    let [body, _, button] = Layout::vertical([
        Constraint::Min(3),
        Constraint::Length(gap),
        Constraint::Length(BUTTON_HEIGHT),
    ])
    .areas(area);
    (body, button)
}

/// A full-width primary action button, e.g. ` Next `.
pub(crate) fn button(label: &str) -> Paragraph<'static> {
    Paragraph::new(Line::from(Span::styled(
        format!(" {label} "),
        Style::default()
            .fg(Color::Rgb(255, 255, 255))
            .bg(Theme::ACCENT)
            .add_modifier(Modifier::BOLD),
    )))
    .style(Style::default().bg(Theme::PANEL))
}

/// Renders a horizontal meter into `width` columns.
pub(crate) fn meter(fraction: f32, width: usize) -> Vec<Span<'static>> {
    Theme::meter(fraction, width, Theme::ACCENT, Theme::BORDER)
}
