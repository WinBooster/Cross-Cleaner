//! Program selection page: search field, program checkboxes with a per-program
//! category overlay, and the pinned ` Start Cleaning ` button — the terminal
//! counterpart of `gui::pages::program_selection`.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph};

use crate::app::{Click, InputMode, TuiApp};
use crate::pages::{BUTTON_HEIGHT, button, count_digits, row_of, span_of};
use crate::theme::Theme;

/// Width of the checkbox glyph, `[x]`.
const CHECK: u16 = 3;

/// Draws the search field, the program list and the pinned button.
///
/// The three are clickable too: the search field takes the caret, a row ticks
/// its program, the ` Start Cleaning ` button is the button `S` presses.
pub fn render(app: &mut TuiApp, frame: &mut Frame, area: Rect) {
    let [search_area, list_area, button_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(3),
        Constraint::Length(BUTTON_HEIGHT),
    ])
    .areas(area);

    render_search(app, frame, search_area);
    app.hit(search_area, Click::Search);

    let shown = app.state.filtered_programs.len();
    let selected = app
        .state
        .program_checkboxes
        .iter()
        .filter(|(flag, _)| *flag.borrow())
        .count();
    let title = format!(" Programs ({selected}/{shown} shown) ");
    let block = Theme::block(&title, true);

    match empty_hint(app) {
        Some(hint) => {
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(hint, Theme::dim()))).block(block),
                list_area,
            );
        }
        None => {
            let inner = block.inner(list_area);
            let list = List::new(program_items(app))
                .block(block)
                .highlight_style(Theme::selected());
            let mut state = ListState::default().with_selected(Some(app.program_cursor));
            frame.render_stateful_widget(list, list_area, &mut state);
            register_rows(app, inner, state.offset());
        }
    }

    frame.render_widget(button(" Start Cleaning "), button_area);
    app.hit(button_area, Click::StartCleaning);
}

/// Registers the visible program rows as click targets.
///
/// `offset` is the row the list was scrolled to, which only ratatui knows after
/// drawing: it scrolls to keep the cursor visible, so the index of the topmost
/// row is a result of the render rather than an input to it. Rows below the box
/// are never drawn and so are never registered — a click there falls through
/// instead of ticking a program the user cannot see.
fn register_rows(app: &mut TuiApp, inner: Rect, offset: usize) {
    for row in 0..inner.height as usize {
        let Some(&index) = app.state.filtered_programs.get(offset + row) else {
            break;
        };
        app.hit(
            row_of(inner, row as u16, inner.width),
            Click::Program(offset + row),
        );
        // Only programs in several categories carry the marker, the same rule
        // that decides whether the row draws one at all. Registered after the
        // row, so it is found first: clicking the marker opens the overlay
        // instead of ticking.
        if let Some(marker) = marker_span(app, index, inner, row as u16) {
            app.hit(marker, Click::ProgramCategories(offset + row));
        }
    }
}

/// The rectangle of the `→ N` marker on the row of program `index`, or `None`
/// when the row has none.
///
/// Measured from the same prefix the row is drawn with — checkbox, a space, the
/// name — so the target covers the glyph and not the label next to it.
fn marker_span(app: &TuiApp, index: usize, inner: Rect, row: u16) -> Option<Rect> {
    let categories = app.state.program_categories.get(index)?;
    if categories.len() <= 1 {
        return None;
    }
    let (_, name) = app.state.program_checkboxes.get(index)?;
    // Two spaces, the arrow, one space, then the digits of the count.
    let marker = 4 + count_digits(categories.len()) as u16;
    Some(span_of(
        inner,
        row,
        CHECK + 1 + name.chars().count() as u16,
        marker,
    ))
}

/// Explains why the list is empty, instead of drawing a blank box.
fn empty_hint(app: &TuiApp) -> Option<&'static str> {
    match (
        app.state.filtered_programs.is_empty(),
        app.state.search_query.is_empty(),
    ) {
        (true, true) => Some("No programs for the selected categories."),
        (true, false) => Some("No program matches the search."),
        _ => None,
    }
}

/// The search field, with a caret while it has focus.
fn render_search(app: &TuiApp, frame: &mut Frame, area: Rect) {
    let editing = app.input == InputMode::Editing;
    let query = if app.state.search_query_visible.is_empty() {
        Span::styled("type a program name…", Theme::dim())
    } else {
        Span::styled(
            app.state.search_query_visible.clone(),
            Style::default().fg(Theme::TEXT),
        )
    };
    let caret = if editing {
        Span::styled("▏", Style::default().fg(Theme::ACCENT))
    } else {
        Span::raw("  ")
    };
    let hint = if editing {
        "esc/enter done · backspace delete"
    } else {
        "/ to search · u to clear"
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" Search: ", Theme::column_header()),
            query,
            caret,
            Span::raw("  "),
            Span::styled(hint, Theme::dim()),
        ]))
        .block(Theme::block("", false)),
        area,
    );
}

/// One row per visible program.
fn program_items(app: &TuiApp) -> Vec<ListItem<'static>> {
    app.state
        .filtered_programs
        .iter()
        .map(|&index| {
            let name = app.state.program_checkboxes[index].1.to_string();
            let (checked, indeterminate) = (
                app.state.is_program_checked(index),
                app.state.is_program_indeterminate(index),
            );
            let (mark, mark_style) = Theme::checkbox(checked, indeterminate);

            let mut spans = vec![
                Span::styled(mark.to_string(), mark_style),
                Span::styled(format!(" {name}"), Theme::text()),
            ];

            // Only programs in several categories get the overlay marker, the
            // same rule the window frontend uses to decide whether to draw its
            // menu button.
            let categories = app
                .state
                .program_categories
                .get(index)
                .map_or(0, |cats| cats.len());
            if categories > 1 {
                let excluded = app
                    .state
                    .program_disabled
                    .get(index)
                    .map_or(0, |disabled| disabled.len());
                spans.push(Span::styled(
                    format!("  → {categories}"),
                    if excluded > 0 {
                        Style::default().fg(Theme::WARN)
                    } else {
                        Theme::dim()
                    },
                ));
            }
            ListItem::new(Line::from(spans))
        })
        .collect()
}
