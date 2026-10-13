//! Cleaning results page: summary heading and a scrollable results table — the
//! terminal counterpart of `gui::pages::results`.
//!
//! The frame keeps all four borders, and the column labels sit *inside* the top
//! one, so the outline stays readable and the labels are pinned rather than
//! scrolling away with the rows. The values are laid out by ratatui's [`Table`],
//! and [`header_line`] reproduces its column geometry for the border, which is
//! what keeps the labels over their values.
//!
//! The labels are also the sort controls. A report is read to find the biggest
//! line in it, and the run does not always put that line first — so clicking a
//! label puts that column into the sort and clicking it again turns the order
//! around ([`ResultSort`]).
//!
//! One column is rarely the whole of a question: *"by category, and inside each
//! category by size"* is a question two numbers cannot answer between them. So a
//! click walks its column into the chain rather than replacing the sort, and the
//! chain is read out in the frame's own bottom border, because the order of its
//! keys is not something an arrow over each label can say.
//!
//! The column clicked *last* is the one that decides. Read the other way round —
//! the first click deciding, the rest breaking its ties — every second click is
//! invisible, because two programs tie on their exact byte count too rarely for a
//! column behind one to ever show anything.
//!
//! All of it is the left button alone. Terminals do not all report a held
//! modifier with a mouse event — on Windows most of them do not — so a control
//! that needed one would be a control that is simply missing there. Clearing the
//! sort is the wheel, over the labels themselves.
//!
//! Every row the page deals in is counted in the drawn order ([`sorted_rows`]),
//! so the cursor, the row targets and the path overlay behind `Enter` cannot
//! drift onto a different program than the one they point at.
//!
//! The table aggregates: one row per program. Clicking a row — or pressing
//! `Enter` on it — opens [`render_details`] over the table, which lists the paths
//! that program deleted and how much each of them freed. The paths themselves
//! come from the run, stored segment-interned ([`database::structures::SharedPath`])
//! so a program that deleted ten thousand files does not hold ten thousand copies
//! of the directory they share.

use std::cmp::Ordering;

use database::structures::Cleared;
use database::utils::get_file_size_string;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
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

/// The columns of the report in the order they are drawn — and the order the
/// labels, their rules and their click targets are laid out in.
const COLUMNS: [SortColumn; 5] = [
    SortColumn::Program,
    SortColumn::Size,
    SortColumn::Files,
    SortColumn::Dirs,
    SortColumn::Categories,
];

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

/// A column of the report, and what it orders the rows by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SortColumn {
    Program,
    Size,
    Files,
    Dirs,
    Categories,
}

impl SortColumn {
    /// The label drawn over the column.
    fn label(self) -> &'static str {
        match self {
            Self::Program => "Program",
            Self::Size => "Size",
            Self::Files => "Files",
            Self::Dirs => "Dirs",
            Self::Categories => "Categories",
        }
    }

    /// The width [`widths`] gives the column, or `None` for the flexible last
    /// one, which has no width of its own.
    fn fixed_width(self) -> Option<u16> {
        match self {
            Self::Program => Some(PROGRAM),
            Self::Size => Some(SIZE),
            Self::Files | Self::Dirs => Some(COUNTS),
            Self::Categories => None,
        }
    }

    /// Whether the column reads best descending — which is the way a key joins
    /// the chain.
    ///
    /// The numbers are read largest-first — that is what a report is opened
    /// for — while text reads A-Z, which is how a list of names has always been
    /// read. Deciding it per column is what keeps `Z → A` from being the first
    /// thing a click on `Program` does.
    fn reads_descending(self) -> bool {
        matches!(self, Self::Size | Self::Files | Self::Dirs)
    }

    /// How two rows stand in this column, ignoring which way it is read.
    fn compare(self, a: &Cleared, b: &Cleared) -> Ordering {
        match self {
            Self::Program => cmp_text(&a.program, &b.program),
            Self::Size => a.removed_bytes.cmp(&b.removed_bytes),
            Self::Files => a.removed_files.cmp(&b.removed_files),
            Self::Dirs => a.removed_directories.cmp(&b.removed_directories),
            // Joined the way the cell draws them, so what the user reads as the
            // column's value is what the sort compares.
            Self::Categories => cmp_text(
                &a.affected_categories.join(", "),
                &b.affected_categories.join(", "),
            ),
        }
    }
}

/// One column of the chain, and the way it is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SortKey {
    column: SortColumn,
    descending: bool,
}

impl SortKey {
    /// How two rows stand in under this key.
    fn compare(&self, a: &Cleared, b: &Cleared) -> Ordering {
        let order = self.column.compare(a, b);
        if self.descending {
            order.reverse()
        } else {
            order
        }
    }

    /// The arrow drawn over the column.
    fn arrow(&self) -> &'static str {
        if self.descending { "↓" } else { "↑" }
    }
}

/// The order the results table is read in: a chain of columns, the first of which
/// decides, the next of which breaks its ties, and the next of those theirs.
///
/// The first is the column the reader clicked last — see [`Self::cycled`].
///
/// An empty chain is the order the cleaner produced — the state the report
/// arrives in, and the one no label claims, which is why the arrows have nothing
/// to explain until a column is picked.
///
/// It lives on the app rather than in the report because the report is the run's
/// data and the order is the reader's: the run has to come out of the cleaner the
/// way the cleaner produced it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResultSort {
    keys: Vec<SortKey>,
}

impl ResultSort {
    /// The order a click on `column` asks for: the column walks through the three
    /// states a sort key can be in — absent, read the way it reads best, the other
    /// way round, absent again — and a key that joins the chain goes to the
    /// *front* of it.
    ///
    /// The newest click deciding is what makes the control work with one button.
    /// A chain read the other way round — the first column deciding, the rest
    /// breaking its ties — is a click that usually does nothing: two programs tie
    /// on their exact byte count so rarely that a second column added behind one
    /// is invisible, and a control with an invisible click is a control nobody
    /// uses twice. Read this way, clicking `Categories` after `Size` does the
    /// thing the click meant: it groups the sizes by category.
    ///
    /// Cycling rather than merely flipping is what lets a chain of columns be
    /// built with the same button: a click adds, the next click reverses it where
    /// it stands, the next takes it out — so a secondary key is reversible, and
    /// reversible keys are half of what makes a chain worth having.
    pub(crate) fn cycled(&self, column: SortColumn) -> Self {
        let mut keys = self.keys.clone();
        match keys.iter().position(|key| key.column == column) {
            Some(index) if keys[index].descending == column.reads_descending() => {
                keys[index].descending = !column.reads_descending();
            }
            Some(index) => {
                keys.remove(index);
            }
            None => keys.insert(0, SortKey::new(column)),
        }
        Self { keys }
    }

    /// True while nothing has been chosen and the table shows the run's order.
    fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// The arrow drawn over `column`, or `None` when the chain does not use it.
    ///
    /// Every key gets an arrow, not only the one that decides: a column that
    /// breaks ties is still doing something, and the user has to be able to see
    /// what to click again to take it out.
    fn marker(&self, column: SortColumn) -> Option<&'static str> {
        self.keys
            .iter()
            .find(|key| key.column == column)
            .map(SortKey::arrow)
    }

    /// The chain as the bottom border spells it out: `size ↓ then program ↑`.
    ///
    /// In the order it is applied, which an arrow over each label cannot say —
    /// two columns both marked `↓` say nothing about which of them decides.
    fn summary(&self) -> String {
        let mut text = String::new();
        for (index, key) in self.keys.iter().enumerate() {
            if index > 0 {
                text.push_str(" then ");
            }
            // Lower case: this is a phrase in a line of hints, not a column of
            // the table.
            text.push_str(&key.column.label().to_lowercase());
            text.push(' ');
            text.push_str(key.arrow());
        }
        text
    }
}

impl SortKey {
    /// A key joining the chain, read the way its column reads best.
    fn new(column: SortColumn) -> Self {
        Self {
            column,
            descending: column.reads_descending(),
        }
    }
}

/// The report's rows in the order the table draws them: indices into the run's
/// own list.
///
/// Everything that counts rows — the drawn order, the row targets, the cursor,
/// the path overlay behind `Enter` — goes through this one mapping, so a row
/// cannot mean one program here and another there because the table was
/// re-sorted underneath it.
pub fn sorted_rows(cleared: &[Cleared], sort: &ResultSort) -> Vec<usize> {
    let mut rows: Vec<usize> = (0..cleared.len()).collect();
    if !sort.is_empty() {
        rows.sort_by(|&a, &b| compare(sort, &cleared[a], &cleared[b]));
    }
    rows
}

/// The order two rows stand in under `sort`.
///
/// The chain decides first and the name breaks whatever it leaves level, so the
/// table is in a fixed order whatever the reader has asked for — rows that tie on
/// every chosen column must not shuffle themselves between two frames.
///
/// The name is read the same way round either way: `size ↓` means *the biggest,
/// and among equals A–Z*, not *the biggest, and among equals Z–A*. Reversing it
/// with the key would make the same two rows swap places every time the direction
/// was turned, for no reason a reader could see.
fn compare(sort: &ResultSort, a: &Cleared, b: &Cleared) -> Ordering {
    for key in &sort.keys {
        let order = key.compare(a, b);
        if order != Ordering::Equal {
            return order;
        }
    }
    cmp_text(&a.program, &b.program)
}

/// Text order that ignores case, without a lowercase copy per comparison.
///
/// Program names come from a hand-written database, where `zlib` next to
/// `Zotero` would sort wherever the capitals happen to put it.
fn cmp_text(a: &str, b: &str) -> Ordering {
    a.chars()
        .flat_map(char::to_lowercase)
        .cmp(b.chars().flat_map(char::to_lowercase))
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

    // The order the rows are drawn in, and the one every row target below is
    // counted in: two orders would let a click open the paths of a different
    // program than the row it landed on.
    let order = app.result_order();
    let rows = order.iter().map(|&index| {
        let entry = &cleared[index];
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

    let block = frame_block(
        header_line(&app.result_sort),
        &app.result_sort,
        cleared.len(),
        table_area.width,
    );
    // Taken before the block is handed to the table, which consumes it: the row
    // targets are measured against the very area the rows are drawn into.
    let inner = block.inner(table_area);
    let table = Table::new(rows, widths())
        .block(block)
        .column_spacing(GAP)
        .row_highlight_style(Theme::selected());

    let mut state = TableState::default().with_selected(Some(app.result_cursor));
    frame.render_stateful_widget(table, table_area, &mut state);

    register_header(app, table_area, inner);
    register_rows(app, &state, inner, cleared.len());
}

/// Makes each column label walk its column into or out of the sort.
///
/// The labels are the sort control, so the target is the label itself — measured
/// from the geometry [`header_line`] draws it with, and clipped to the frame,
/// because a target that escaped the box would answer clicks aimed at whatever
/// is drawn there instead.
///
/// The whole border is recorded as well, for the wheel: a click lands on a label
/// and a wheel lands wherever the pointer is, and over the labels the wheel's
/// answer is to clear the sort. It is only recorded while there is a sort to
/// clear, or the wheel would stop scrolling over a frame with nothing to undo.
fn register_header(app: &mut TuiApp, block: Rect, inner: Rect) {
    let clearable = !app.result_sort.is_empty();
    app.result_header = clearable.then(|| {
        Rect {
            x: inner.x,
            y: block.y,
            width: inner.width,
            height: 1,
        }
        .intersection(block)
    });

    // One row above the rows: the labels sit *in* the top border.
    let mut x = inner.x;
    for column in COLUMNS {
        // A label a narrow frame cannot draw gets no target: the border under
        // it is only a border, and answering that click would order the report
        // by a column the user cannot see.
        if x >= inner.right() {
            break;
        }
        let width = match column.fixed_width() {
            Some(width) => width + GAP,
            // The last column takes whatever is left, so its target runs to the
            // edge of the frame.
            None => inner.right().saturating_sub(x),
        };
        app.hit(
            Rect {
                x,
                y: block.y,
                width,
                height: 1,
            }
            .intersection(block),
            Click::SortColumn(column),
        );
        x = x.saturating_add(width);
    }
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
///
/// The bottom border carries the sort, because that is the one thing about it an
/// arrow over each label cannot say: which of the marked columns decides. `width`
/// is the frame's own, because the chain grows with every column the reader adds
/// and a title that does not fit is clipped *from the left* — which would eat the
/// head of the chain, the one part they cannot reconstruct.
fn frame_block(
    header: Line<'static>,
    sort: &ResultSort,
    programs: usize,
    width: u16,
) -> Block<'static> {
    // What the reader has to see. Always kept: it is the state of the page.
    let state = match sort.is_empty() {
        true => format!(" {programs} cleaned "),
        false => format!(" {programs} cleaned · {} ", sort.summary()),
    };
    // What they can do with it — the whole group or none of it, because half a
    // hint teaches nothing. It is also the footer line's own text, so a frame too
    // narrow for both still has them one row below.
    const HINTS: &str = "click to sort · wheel clears · enter paths · esc back ";
    let fits =
        state.chars().count() + HINTS.chars().count() + 3 <= width.saturating_sub(2) as usize;
    let title = match fits {
        true => format!("{state}· {HINTS}"),
        false => state,
    };
    Block::bordered()
        .title_top(header)
        .title_bottom(Line::from(title).right_aligned())
        .border_type(BorderType::Rounded)
        .border_style(Theme::border(true))
        .style(Style::default().bg(Theme::BG))
}

/// The column labels, laid out exactly like [`Table`]'s columns, with every
/// column the chain uses marked.
///
/// Each fixed column occupies `width + column_spacing` cells. The gaps are filled
/// with the border glyph rather than with spaces: a block title is drawn *over*
/// the top border, so every blank inside it erases the rule and the labels end up
/// floating on an otherwise bare line. Writing the rule explicitly keeps the
/// frame continuous between the labels.
///
/// The label comes first so it starts in the same column as the values below it,
/// and the sort arrow goes *inside* the cell — in the space the rule would have
/// filled — so marking a column never pushes the labels after it out of line with
/// their values.
fn header_line(sort: &ResultSort) -> Line<'static> {
    let mut spans = Vec::new();
    for column in COLUMNS {
        let label = header_label(column, sort);
        match column.fixed_width() {
            Some(width) => {
                let filled = label.chars().count() as u16;
                spans.push(Span::styled(label, header_style(column, sort)));
                spans.push(Span::styled(
                    "─".repeat((width + GAP).saturating_sub(filled) as usize),
                    Theme::border(true),
                ));
            }
            // The last column takes whatever width is left, so its rule is
            // simply what the block fills in after the title ends. Nothing
            // precedes the label either: every fixed column before it has
            // already drawn its gap, so a space here would push the label one
            // column right of the values it belongs to.
            None => spans.push(Span::styled(label, header_style(column, sort))),
        }
    }
    Line::from(spans)
}

/// One label and the arrow that says the chain uses its column, if it does.
fn header_label(column: SortColumn, sort: &ResultSort) -> String {
    match sort.marker(column) {
        Some(marker) => format!("{} {marker} ", column.label()),
        None => format!("{} ", column.label()),
    }
}

/// A column the chain uses, in the accent color.
///
/// The arrow alone would do, but a label in a different color reads as chosen at
/// a glance — which matters here, because the arrow is one glyph in a border and
/// the column itself is the thing that was clicked.
fn header_style(column: SortColumn, sort: &ResultSort) -> Style {
    if sort.marker(column).is_some() {
        Style::default()
            .fg(Theme::ACCENT)
            .add_modifier(Modifier::BOLD)
    } else {
        Theme::column_header()
    }
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
    let Some(cleared) = app.state.cleared_data.as_ref().map(|data| data.3.clone()) else {
        app.details = None;
        return;
    };
    // The row is a position in the table as it is drawn, so it is resolved
    // through the current order: counting in the run's own list would send the
    // overlay to a different program's paths as soon as a sort was in force.
    let Some(entry) = app
        .result_order()
        .get(row)
        .and_then(|&index| cleared.get(index))
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
