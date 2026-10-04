//! Cleaning results page: summary heading and a scrollable results table — the
//! terminal counterpart of `gui::pages::results`.
//!
//! The frame keeps all four borders, and the column labels sit *inside* the top
//! one, so the outline stays readable and the labels are pinned rather than
//! scrolling away with the rows. The values are laid out by ratatui's [`Table`],
//! and [`header_line`] reproduces its column geometry for the border, which is
//! what keeps the labels over their values.

use database::utils::get_file_size_string;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Cell, Paragraph, Row, Table, TableState};

use crate::app::TuiApp;
use crate::theme::Theme;

/// Column widths, mirroring the window frontend's table.
const PROGRAM: u16 = 30;
const SIZE: u16 = 10;
const COUNTS: u16 = 7;
/// Gap `Table::column_spacing` puts after every fixed column.
const GAP: u16 = 1;

fn widths() -> [Constraint; 5] {
    [
        Constraint::Length(PROGRAM),
        Constraint::Length(SIZE),
        Constraint::Length(COUNTS),
        Constraint::Length(COUNTS),
        // Whatever is left for the category list, which has no fixed size.
        Constraint::Min(10),
    ]
}

/// Draws the summary and the table.
pub fn render(app: &mut TuiApp, frame: &mut Frame, area: Rect) {
    let [summary, caption, table_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Min(3),
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

    // Above the frame: the box's top edge carries the column labels.
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" {} programs ", cleared.len()),
            Theme::column_header(),
        ))),
        caption,
    );

    let rows = cleared.iter().map(|entry| {
        Row::new(vec![
            Cell::from(entry.program.clone()),
            Cell::from(get_file_size_string(entry.removed_bytes))
                .style(Style::default().fg(Theme::GOOD)),
            Cell::from(entry.removed_files.to_string()).style(Theme::dim()),
            Cell::from(entry.removed_directories.to_string()).style(Theme::dim()),
            Cell::from(entry.affected_categories.join(", "))
                .style(Style::default().fg(Theme::TEXT_DIM)),
        ])
    });

    let table = Table::new(rows, widths())
        .block(frame_block(header_line(), cleared.len()))
        .column_spacing(GAP)
        .row_highlight_style(Theme::selected());

    let mut state = TableState::default().with_selected(Some(app.result_cursor));
    frame.render_stateful_widget(table, table_area, &mut state);
}

/// The box around the rows, with the column labels as its top title.
///
/// The block must not carry a second title: two top titles share the same border
/// row and overwrite each other, which is what used to shift the labels out of
/// line with the values.
fn frame_block(header: Line<'static>, programs: usize) -> Block<'static> {
    Block::bordered()
        .title_top(header)
        .title_bottom(
            Line::from(format!(" {programs} cleaned · esc back ")).right_aligned(),
        )
        .border_type(BorderType::Rounded)
        .border_style(Theme::border(true))
        .style(Style::default().bg(Theme::BG))
}

/// The column labels, laid out exactly like [`Table`]'s columns.
///
/// Each fixed column occupies `width + column_spacing` cells. The gaps are filled
/// with the border glyph rather than with spaces: a block title is drawn *over*
/// the top border, so every blank inside it erases the rule and the labels end up
/// floating on an otherwise bare line. Writing the rule explicitly keeps the
/// frame continuous between the labels.
///
/// The label comes first so it starts in the same column as the values below it.
fn header_line() -> Line<'static> {
    let mut spans = Vec::new();
    for label in ["Program", "Size", "Files", "Dirs"] {
        spans.push(header_label(label));
        spans.push(header_rule(width_of(label)));
    }
    // The last column takes whatever width is left, so its rule is simply what
    // the block fills in after the title ends.
    spans.push(Span::styled(
        " Categories ",
        Style::default().fg(Theme::TEXT_DIM).add_modifier(ratatui::style::Modifier::BOLD),
    ));
    Line::from(spans)
}

/// The fixed width of a named column.
fn width_of(label: &str) -> u16 {
    match label {
        "Program" => PROGRAM,
        "Size" => SIZE,
        _ => COUNTS,
    }
}

/// One label, immediately followed by its trailing space.
fn header_label(label: &str) -> Span<'static> {
    Span::styled(
        format!("{label} "),
        Style::default()
            .fg(Theme::TEXT_DIM)
            .add_modifier(ratatui::style::Modifier::BOLD),
    )
}

/// The part of a header cell that is not a label: the column itself plus the gap
/// after it, drawn in the border color so it merges with the rest of the frame.
fn header_rule(width: u16) -> Span<'static> {
    let len = match width {
        PROGRAM => "Program ".len(),
        SIZE => "Size ".len(),
        _ => "Files ".len(),
    } as u16;
    Span::styled(
        "─".repeat((width + GAP).saturating_sub(len) as usize),
        Theme::border(true),
    )
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
        .alignment(Alignment::Center),
        area,
    );
}