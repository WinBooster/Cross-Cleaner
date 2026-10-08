//! Main (home) page: a two-column grid of tristate category checkboxes with a
//! subcategory overlay and a pinned ` Next ` button — the terminal counterpart
//! of `gui::pages::main`.
//!
//! Two details mirror the window frontend:
//!
//! * The grid is laid out **right to left**: the last column is flush against
//!   the right window edge instead of floating in the middle.
//! * Every cell is padded to one fixed width, so a category whose `→ N` marker
//!   is longer (or missing) cannot shift the next row out of alignment.
//!
//! Navigation is grid-aware and matches what is drawn: `Up`/`Down` move a whole
//! row and `Tab` changes column, while the highlight always sits on the exact
//! cell that `Space` would toggle.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem};

use appcore::{CATEGORY_COLUMNS, CategoryState};

use crate::app::TuiApp;
use crate::pages::{button, split_body};
use crate::theme::Theme;

/// Narrowest cell, so a very short category name does not make a ragged grid.
const MIN_CELL: usize = 16;
/// Widest cell: past this the second column no longer fits a 80-column
/// terminal, and the grid falls back to a single column.
const MAX_CELL: usize = 34;
/// Blank columns kept between the two cells when they are packed left to right
/// in a terminal too narrow to place them at both edges.
const GAP: usize = 2;
/// Marker in front of the focused cell. Every cell emits one slot so the
/// columns stay aligned whether or not it is the focused one.
const MARKER: &str = "▸";
/// Width of the checkbox glyph, `[x]`.
const CHECK: usize = 3;

/// Draws the category grid and the pinned button.
pub fn render(app: &mut TuiApp, frame: &mut Frame, area: Rect) {
    let (list_area, button_area) = split_body(area, 1);

    let selected = app
        .state
        .categories
        .iter()
        .filter(|c| !c.is_unchecked())
        .count();
    let title = format!(
        " Categories ({selected}/{} selected) ",
        app.state.categories.len()
    );

    let block = Theme::block(&title, true);
    let inner_width = block.inner(list_area).width as usize;
    // No `highlight_style`: the focus is drawn per cell, because ratatui can
    // only highlight a whole list row and the grid has two cells per row.
    let list = List::new(category_rows(app, inner_width)).block(block);

    frame.render_widget(list, list_area);
    frame.render_widget(button(" Next "), button_area);
}

/// One list item per row, each holding up to [`CATEGORY_COLUMNS`] cells.
fn category_rows(app: &TuiApp, inner_width: usize) -> Vec<ListItem<'static>> {
    let cell = cell_width(app, inner_width);
    // Placing one cell at each edge needs room for both plus a visible gap;
    // otherwise the grid packs them left to right and wraps on narrow terminals.
    let spread = inner_width >= cell * 2 + GAP;

    app.state
        .categories
        .chunks(CATEGORY_COLUMNS)
        .enumerate()
        .map(|(row, chunk)| {
            let base = row * CATEGORY_COLUMNS;
            // The left cell reads left to right; the right one is mirrored, so
            // its checkbox sits against the right window edge.
            let mut spans = cell_spans(app, base, cell, Side::Left);

            match chunk.len() {
                0 => {}
                1 => {
                    // A short last row keeps its cell at the left edge, the way
                    // the window frontend's first column does.
                    if spread {
                        spans.push(Span::raw(" ".repeat(inner_width - cell)));
                    }
                }
                _ => {
                    if spread {
                        // Push the mirrored cell against the right edge.
                        spans.push(Span::raw(" ".repeat(inner_width - cell * 2)));
                    } else {
                        spans.push(Span::raw(" ".repeat(GAP)));
                    }
                    spans.extend(cell_spans(app, base + 1, cell, Side::Right));
                }
            }
            ListItem::new(Line::from(spans))
        })
        .collect()
}

/// Width every cell is padded to, so the columns line up on every row.
///
/// It is the widest *natural* cell in the grid, which is what keeps a category
/// with a long label or a two-digit `→ 12` marker from pushing the next row out
/// of alignment.
fn cell_width(app: &TuiApp, inner_width: usize) -> usize {
    let natural = app
        .state
        .categories
        .iter()
        .enumerate()
        .map(|(index, category)| natural_width(app, index, category))
        .max()
        .unwrap_or(MIN_CELL)
        .clamp(MIN_CELL, MAX_CELL);
    // Never wider than the space two columns can afford. Otherwise a long
    // category name would push the mirrored cell past the right edge and the
    // buffer would clip its checkbox away entirely, leaving a row that looks
    // like it holds one entry.
    let affordable = inner_width.saturating_sub(GAP) / CATEGORY_COLUMNS;
    natural.min(affordable).max(1)
}

/// Printable width a cell needs: marker, checkbox, space, label, subcategory
/// arrow. The mirrored cell has the same elements in the opposite order, so the
/// width is identical either way.
fn natural_width(app: &TuiApp, index: usize, category: &CategoryState) -> usize {
    let label = app.state.category_label(index).chars().count();
    MARKER.chars().count() + CHECK + 1 + label + hint_len(category)
}

/// Width of the subcategory arrow with its count, or zero when the category has
/// no subcategories.
///
/// Both directions use the same number of columns — `  → 3` on the left,
/// `3 ←  ` mirrored — so switching a cell between them cannot shift the grid.
fn hint_len(category: &CategoryState) -> usize {
    if category.subs.is_empty() {
        return 0;
    }
    // Two spaces, the arrow, one space, then the digits of the count.
    4 + count_digits(category.selected.len())
}

/// The subcategory arrow and its count, pointing away from the label.
///
/// Mirroring the arrow matters: in the right column the count sits *outside* the
/// label, so a `→` there would point back at the text instead of away from it.
fn sub_hint(category: &CategoryState, side: Side, style: Style) -> Span<'static> {
    if category.subs.is_empty() {
        return Span::raw("");
    }
    let selected = category.selected.len();
    Span::styled(
        match side {
            Side::Left => format!("  → {selected}"),
            Side::Right => format!("{selected} ←  "),
        },
        style,
    )
}

/// Number of decimal digits in `value`.
fn count_digits(value: usize) -> usize {
    value.checked_ilog10().map_or(1, |d| d as usize + 1)
}

/// Which way a cell is laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    /// Left column: marker, checkbox, label, then the subcategory arrow.
    Left,
    /// Right column: the exact mirror, so the checkbox lands on the right window
    /// edge and the arrow points back the other way.
    Right,
}

/// One category cell, padded out to exactly `width` columns.
///
/// [`Side::Right`] renders the same four elements in the opposite order:
///
/// ```text
/// left    ▸[x] Cache (1)  → 2 ▸
/// right   2 ←  (1) Cache [x]▸
/// ```
///
/// The single space that separates the label from the checkbox moves with it, so
/// both variants occupy the same number of columns — that is what keeps the grid
/// aligned when one category has a `→ 12` marker and its neighbour has none.
///
/// When `index` is the focused category the marker is filled and the spans carry
/// [`Theme::selected`], so the highlight covers exactly the cell `Space` toggles.
fn cell_spans(app: &TuiApp, index: usize, width: usize, side: Side) -> Vec<Span<'static>> {
    let category = &app.state.categories[index];
    let focused = index == app.category_cursor;
    let (check, check_style) = Theme::checkbox(category.is_checked(), category.is_indeterminate());

    // Everything except the label has a known width, so the label gets whatever
    // is left over and is truncated when it does not fit.
    let fixed = MARKER.chars().count() + CHECK + 1 + hint_len(category);
    let budget = width.saturating_sub(fixed).max(1);
    let label = truncate(app.state.category_label(index), budget);

    let label_style = if focused {
        Theme::selected()
    } else {
        Theme::text()
    };
    let hint_style = if focused {
        Theme::selected()
    } else {
        sub_hint_style(category.selected.len())
    };

    let marker = Span::styled(
        if focused { MARKER } else { " " },
        Style::default().fg(Theme::ACCENT),
    );
    let check = Span::styled(check.to_string(), check_style);
    // The separating space belongs to the side the label faces.
    let label_span = Span::styled(
        match side {
            Side::Left => format!(" {label}"),
            Side::Right => format!("{label} "),
        },
        label_style,
    );
    let hint = sub_hint(category, side, hint_style);

    // `fixed` already accounts for the hint, so this is the only padding needed,
    // and it always goes on the far side of the cell.
    let used = fixed + label.chars().count();
    let pad = Span::raw(" ".repeat(width.saturating_sub(used)));
    match side {
        Side::Left => vec![marker, check, label_span, hint, pad],
        Side::Right => vec![pad, hint, label_span, check, marker],
    }
}

fn sub_hint_style(selected: usize) -> Style {
    if selected > 0 {
        Style::default().fg(Theme::ACCENT)
    } else {
        Theme::dim()
    }
}

/// Shortens `text` to `budget` columns, marking the cut with an ellipsis.
fn truncate(text: &str, budget: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= budget {
        return text.to_string();
    }
    if budget == 1 {
        return "…".to_string();
    }
    let mut out: String = chars[..budget - 1].iter().collect();
    out.push('…');
    out
}
