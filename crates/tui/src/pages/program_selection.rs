//! Program selection page: search field, program checkboxes with a per-program
//! category overlay, and the pinned ` Start Cleaning ` button — the terminal
//! counterpart of `gui::pages::program_selection`.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph};

use crate::app::{InputMode, TuiApp};
use crate::pages::{BUTTON_HEIGHT, button};
use crate::theme::Theme;

/// Draws the search field, the program list and the pinned button.
pub fn render(app: &mut TuiApp, frame: &mut Frame, area: Rect) {
    let [search_area, list_area, button_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(3),
        Constraint::Length(BUTTON_HEIGHT),
    ])
    .areas(area);

    render_search(app, frame, search_area);

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
            let list = List::new(program_items(app))
                .block(block)
                .highlight_style(Theme::selected());
            let mut state = ListState::default().with_selected(Some(app.program_cursor));
            frame.render_stateful_widget(list, list_area, &mut state);
        }
    }

    frame.render_widget(button(" Start Cleaning "), button_area);
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
