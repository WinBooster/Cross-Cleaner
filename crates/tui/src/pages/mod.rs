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

/// A rectangle covering `width` columns of the row `row` of `area`, starting
/// `column` columns in.
///
/// The shared shape behind every click target: a whole row of a list, or the
/// narrow `→ N` marker inside one. Clipped to `area`, because a target that
/// escaped its box would answer clicks aimed at whatever is drawn underneath —
/// on a cramped terminal, or on the mirrored column of the category grid.
pub(crate) fn span_of(area: Rect, row: u16, column: u16, width: u16) -> Rect {
    Rect {
        x: area.x.saturating_add(column),
        y: area.y.saturating_add(row),
        width,
        height: 1,
    }
    .intersection(area)
}

/// [`span_of`] for a target that spans the full inner width, from `area.x`.
pub(crate) fn row_of(area: Rect, row: u16, width: u16) -> Rect {
    span_of(area, row, 0, width)
}

/// Number of decimal digits in `value`, at least one.
///
/// Every `→ N` marker reserves this many columns for its count, so both the
/// glyph and its click target have to agree on it.
pub(crate) fn count_digits(value: usize) -> usize {
    value.checked_ilog10().map_or(1, |d| d as usize + 1)
}
