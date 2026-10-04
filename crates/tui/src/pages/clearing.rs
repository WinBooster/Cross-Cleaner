//! Cleaning progress page: the program being worked on, the progress meter, the
//! cleaned size and the ETA — the terminal counterpart of
//! `gui::pages::clearing`.

use database::utils::get_file_size_string;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::TuiApp;
use crate::pages::meter;
use crate::theme::Theme;

/// Draws the progress page.
pub fn render(app: &mut TuiApp, frame: &mut Frame, area: Rect) {
    let program = app.state.current_program().to_string();
    let fraction = app.state.progress_fraction();
    let eta = app.state.eta();
    let done = app.state.current_task;
    let total = app.state.total_tasks;
    let bytes = app.state.cleaned_bytes;
    let spinner = app.spinner_frame();

    let block = Theme::block(" Cleaning ", true);
    let inner = block.inner(area);
    // The footer is its own row, so the meter above it keeps its full width
    // instead of having to share it with the counters.
    let [body, footer] = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);

    let mut lines: Vec<Line<'static>> = Vec::new();
    if !program.is_empty() {
        lines.push(Line::from(Span::styled(
            program,
            Style::default()
                .fg(Theme::TEXT)
                .add_modifier(ratatui::style::Modifier::BOLD),
        )));
        lines.push(Line::default());
    }

    match fraction {
        Some(ratio) => {
            lines.push(Line::from(vec![
                Span::styled(format!(" {done}/{total}"), Theme::dim()),
                Span::styled(format!("  {:>3.0}%", ratio * 100.0), Theme::text()),
            ]));
            lines.push(Line::default());
            lines.push(Line::from(meter(ratio, body.width as usize)));
        }
        // Before the first progress tick there is nothing to fill, so a spinner
        // is the honest indicator.
        None => lines.push(Line::from(vec![
            Span::styled(format!(" {spinner} "), Style::default().fg(Theme::ACCENT)),
            Span::styled("preparing the work list…", Theme::dim()),
        ])),
    }

    frame.render_widget(Paragraph::new(lines).block(block), area);

    // Cleaned size on the left, ETA on the right, like the window page.
    let [left, right] =
        Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).areas(footer);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" {} cleaned", get_file_size_string(bytes)),
            Theme::text(),
        ))),
        left,
    );
    if let Some(eta) = eta {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                eta,
                Style::default().fg(Theme::WARN),
            )))
            .alignment(Alignment::Right),
            right,
        );
    }
}
