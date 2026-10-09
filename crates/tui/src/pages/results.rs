//! Cleaning results page: summary heading and a scrollable results table — the
//! terminal counterpart of `gui::pages::results`.
//!
//! The frame keeps all four borders, and the column labels sit *inside* the top
//! one, so the outline stays readable and the labels are pinned rather than
//! scrolling away with the rows. The values are laid out by ratatui's [`Table`],
//! and [`header_line`] reproduces its column geometry for the border, which is
//! what keeps the labels over their values.
//!
//! The table aggregates: one row per program. Clicking a row — or pressing
//! `Enter` on it — opens [`render_details`] over the table, which lists the paths
//! that program deleted and how much each of them freed. The paths themselves
//! come from the run, stored segment-interned ([`database::structures::SharedPath`])
//! so a program that deleted ten thousand files does not hold ten thousand copies
//! of the directory they share.

use database::structures::Cleared;
use database::utils::get_file_size_string;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Cell, Clear, Paragraph, Row, Table, TableState, Wrap};

use crate::app::{Click, TuiApp};
use crate::pages::row_of;
use crate::theme::Theme;

/// Column widths, mirroring the window frontend's table.
const PROGRAM: u16 = 30;
const SIZE: u16 = 10;
const COUNTS: u16 = 7;
/// Gap `Table::column_spacing` puts after every fixed column.
const GAP: u16 = 1;

/// Columns kept between the path overlay and the left and right edges of the
/// results page, and rows kept between it and the top.
///
/// The box is a child of the table, not of the screen: it sits inside the report
/// it describes, so the summary above it, the header row and the footer stay
/// where they were. The margin is what makes that visible — the table's own frame
/// is drawn under it, so the report reads as the page the list belongs to.
pub const DETAIL_MARGIN: u16 = 5;

/// Rows left below the path overlay.
///
/// Smaller than [`DETAIL_MARGIN`] on purpose: the overlay carries its own
/// ` n/m · ↑↓ scroll · esc close ` line in its bottom border, and with the same
/// margin as the sides that border sat far enough above the foot of the table to
/// look detached from it. Two rows leave the report visibly underneath it — which
/// is the whole point of drawing this inside the report — without opening a gap.
pub const DETAIL_MARGIN_BOTTOM: u16 = 2;

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

    let block = frame_block(header_line(), cleared.len());
    // Taken before the block is handed to the table, which consumes it: the row
    // targets are measured against the very area the rows are drawn into.
    let inner = block.inner(table_area);
    let table = Table::new(rows, widths())
        .block(block)
        .column_spacing(GAP)
        .row_highlight_style(Theme::selected());

    let mut state = TableState::default().with_selected(Some(app.result_cursor));
    frame.render_stateful_widget(table, table_area, &mut state);

    register_rows(app, &state, inner, cleared.len());
}

/// Makes each visible row open its own path list.
///
/// Only the rows that exist are registered: on a terminal taller than the report
/// the lower rows are bare background, and a target there would answer a click
/// for a program that was never cleaned.
fn register_rows(app: &mut TuiApp, state: &TableState, inner: Rect, rows: usize) {
    // The table scrolls to keep the selection visible and writes back the offset
    // it ended up at, so the row under a click is only known after drawing —
    // the same reason the overlays register their rows late.
    let offset = state.offset();
    let visible = inner.height.min(rows.saturating_sub(offset) as u16);
    for row in 0..visible {
        app.hit(
            row_of(inner, row, inner.width),
            Click::ResultRow(offset + row as usize),
        );
    }
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
            Line::from(format!(" {programs} cleaned · enter paths · esc back ")).right_aligned(),
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
        Style::default()
            .fg(Theme::TEXT_DIM)
            .add_modifier(ratatui::style::Modifier::BOLD),
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
    )
}

/// Draws the paths one results row deleted, and what each of them freed.
///
/// A row of the table is an aggregate, and an aggregate is the one number a user
/// cannot check: the question it provokes is *where did this go*. This is the
/// answer, one line per path, largest first — the order [`database`] stores them
/// in, so the biggest deletion is never below the fold.
pub fn render_details(app: &mut TuiApp, frame: &mut Frame, area: Rect) {
    let Some((row, scroll)) = app
        .details
        .as_ref()
        .map(|details| (details.row, details.scroll))
    else {
        return;
    };
    let Some(entry) = app
        .state
        .cleared_data
        .as_ref()
        .and_then(|data| data.3.get(row))
    else {
        // The run behind the overlay is gone — the page was left, or a new run
        // replaced it. An overlay over nothing is worse than none.
        app.details = None;
        return;
    };

    let lines = line_count(entry);
    // The report inset by its margins, and placed explicitly rather than through
    // `centered`: the margins differ on the foot, and centring cannot express
    // that — it would put the box back in the middle with equal gaps.
    let width = area
        .width
        .saturating_sub(DETAIL_MARGIN * 2)
        .max(20)
        .min(area.width);
    let height = area
        .height
        .saturating_sub(DETAIL_MARGIN + DETAIL_MARGIN_BOTTOM)
        .max(8)
        .min(area.height);
    let rect = Rect {
        x: area.x + DETAIL_MARGIN,
        y: area.y + DETAIL_MARGIN,
        width,
        height,
    };
    frame.render_widget(Clear, rect);

    let block = Theme::block(
        &format!(
            " {} — {} {} ",
            entry.program,
            entry.paths.len(),
            plural(entry.paths.len(), "path", "paths")
        ),
        true,
    );
    let inner = block.inner(rect);
    let max_scroll = lines.saturating_sub(inner.height as usize);
    // Clamped here rather than on the key press: only the render knows how many
    // lines fit, and it changes with the terminal.
    let scroll = scroll.min(max_scroll);
    if let Some(details) = &mut app.details {
        details.scroll = scroll;
        details.max_scroll = max_scroll;
    }

    let block = block.title_bottom(
        Line::from(format!(
            " {}/{} · {}esc close ",
            scroll + 1,
            lines.max(1),
            if max_scroll > 0 {
                "↑↓ scroll · "
            } else {
                ""
            },
        ))
        .right_aligned(),
    );
    // Only the lines on screen are formatted, and the paragraph is not told to
    // scroll: it has already been handed exactly the slice on screen. Scrolling
    // as well would skip that many lines a second time, which is what turned the
    // list blank once the offset grew past a screenful.
    frame.render_widget(
        Paragraph::new(visible_lines(entry, scroll, scroll + inner.height as usize))
            .block(block)
            // Wrapped, not truncated: a Windows path does not fit beside its
            // size on a narrow terminal, and the size is half the answer.
            .wrap(Wrap { trim: true }),
        rect,
    );

    // The backdrop covers the results page, so a click beside the box closes the
    // list instead of reaching the row underneath it. Registered first, so the
    // lines inside win where they overlap. It stops at the page: the header and
    // the footer are outside `area`, and nothing outside the report is covered by
    // this overlay in the first place.
    app.hit(area, Click::CloseDetails);
    for line in 0..inner.height {
        app.hit(
            row_of(inner, line, inner.width),
            Click::DetailRow(scroll + line as usize),
        );
    }
}

/// How many lines the list of `entry` takes: the truncation notice, if there is
/// one, plus a line per deleted path.
///
/// Counted rather than collected, because the overlay only ever formats the
/// lines on screen — but the scroll arithmetic needs the total.
fn line_count(entry: &Cleared) -> usize {
    entry.paths.len() + usize::from(entry.paths_omitted > 0)
}

/// The lines of `entry` in `[start, end)` — the ones on screen.
///
/// The notice, when there is one, is the first line, so path `n` sits one line
/// further down than it would without it. Getting that offset wrong by one is
/// what makes a scrolled list show the wrong path for the position it claims.
fn visible_lines(entry: &Cleared, start: usize, end: usize) -> Vec<Line<'static>> {
    let notice = usize::from(entry.paths_omitted > 0);
    let mut lines = Vec::with_capacity(end.saturating_sub(start));
    if notice == 1 && start == 0 {
        // Said out loud: a list that stops early must not read as the whole of
        // what was deleted.
        lines.push(Line::from(Span::styled(
            format!(
                " {} more deleted {} are not listed.",
                entry.paths_omitted,
                plural(entry.paths_omitted, "path", "paths")
            ),
            Theme::dim(),
        )));
    }
    let first = start.saturating_sub(notice);
    let last = end.saturating_sub(notice).min(entry.paths.len());
    lines.extend(entry.paths[first..last].iter().map(path_line));
    lines
}

/// One deleted path: the path itself, then what it freed.
///
/// The counts are omitted when they are zero, so a single removed file reads as
/// `1 file` rather than `1 file  0 dirs`.
fn path_line(detail: &database::structures::ClearedPath) -> Line<'static> {
    let mut counts = Vec::new();
    if detail.removed_files > 0 {
        counts.push(format!(
            "{} {}",
            detail.removed_files,
            plural(detail.removed_files as usize, "file", "files")
        ));
    }
    if detail.removed_directories > 0 {
        counts.push(format!(
            "{} {}",
            detail.removed_directories,
            plural(detail.removed_directories as usize, "dir", "dirs")
        ));
    }
    Line::from(vec![
        Span::styled(detail.path.to_string(), Theme::text()),
        Span::styled(
            format!("  {}", get_file_size_string(detail.removed_bytes)),
            Style::default().fg(Theme::GOOD),
        ),
        Span::styled(format!(" {}", counts.join(" ")), Theme::dim()),
    ])
}

/// `one path` / `two paths`, from the count rather than by hand.
fn plural(count: usize, one: &'static str, many: &'static str) -> &'static str {
    if count == 1 { one } else { many }
}
