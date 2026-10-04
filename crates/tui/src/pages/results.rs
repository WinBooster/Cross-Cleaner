//! Cleaning results page: summary heading and a scrollable results table — the
//! terminal counterpart of `gui::pages::results`.

use database::utils::get_file_size_string;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::app::TuiApp;
use crate::theme::Theme;

/// Column widths, mirroring the window frontend's table.
const PROGRAM: u16 = 30;
const SIZE: u16 = 10;
const COUNTS: u16 = 7;

/// Draws the summary and the table.
pub fn render(app: &mut TuiApp, frame: &mut Frame, area: Rect) {
    let [summary, table] = ratatui::layout::Layout::vertical([
        ratatui::layout::Constraint::Length(4),
        ratatui::layout::Constraint::Min(3),
    ])
    .areas(area);

    let Some((bytes, files, dirs, cleared)) = app.state.cleared_data.clone() else {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled("No results yet.", Theme::dim()))),
            area,
        );
        return;
    };

    render_summary(frame, summary, bytes, files, dirs, cleared.len());

    let items: Vec<ListItem<'static>> = cleared
        .iter()
        .map(|entry| {
            ListItem::new(Line::from(vec![
                Span::styled(pad(&entry.program, PROGRAM), Theme::text()),
                Span::styled(
                    pad(&get_file_size_string(entry.removed_bytes), SIZE),
                    Style::default().fg(Theme::GOOD),
                ),
                Span::styled(pad(&entry.removed_files.to_string(), COUNTS), Theme::dim()),
                Span::styled(
                    pad(&entry.removed_directories.to_string(), COUNTS),
                    Theme::dim(),
                ),
                Span::styled(
                    entry.affected_categories.join(", "),
                    Style::default().fg(Theme::TEXT_DIM),
                ),
            ]))
        })
        .collect();

    let header = Line::from(vec![
        Span::styled(pad("Program", PROGRAM), Theme::column_header()),
        Span::styled(pad("Size", SIZE), Theme::column_header()),
        Span::styled(pad("Files", COUNTS), Theme::column_header()),
        Span::styled(pad("Dirs", COUNTS), Theme::column_header()),
        Span::styled("Categories", Theme::column_header()),
    ]);

    let list = List::new(items).block(
        Theme::block(&format!(" {} programs ", cleared.len()), true)
            .title_top(header.left_aligned()),
    )
    .highlight_style(Theme::selected());

    let mut state = ListState::default().with_selected(Some(app.result_cursor));
    frame.render_stateful_widget(list, table, &mut state);
}

fn render_summary(
    frame: &mut Frame,
    area: Rect,
    bytes: u64,
    files: u64,
    dirs: u64,
    programs: usize,
) {
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled("Cleaning Results", Theme::heading())),
            Line::from(Span::styled(
                format!(
                    "Size: {}, Files: {}, Dirs: {}, Programs: {}",
                    get_file_size_string(bytes),
                    files,
                    dirs,
                    programs
                ),
                Theme::dim(),
            )),
        ])
        .alignment(ratatui::layout::Alignment::Center),
        area,
    );
}

/// Truncates or pads `text` to exactly `width` columns.
fn pad(text: &str, width: u16) -> String {
    let mut out = text.chars().take(width as usize).collect::<String>();
    let len = out.chars().count();
    out.push_str(&" ".repeat((width as usize).saturating_sub(len)));
    out.push(' ');
    out
}