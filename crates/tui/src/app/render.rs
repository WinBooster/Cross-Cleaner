//! Drawing one frame.
//!
//! [`TuiApp::render`] lays out the header, the page and the footer, then paints
//! the overlays on top of them in that order — the same order the click targets
//! are registered in, so what answers a click is what is on screen.

use std::time::Instant;

use appcore::app::Page;
use database::version::Changelog;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Padding, Paragraph, Wrap};

use crate::pages;
use crate::theme::Theme;

use super::overlay::Popup;
use super::{Click, HitAreas, TuiApp};

impl TuiApp {
    /// Draws one frame.
    ///
    /// Also rebuilds the click targets: every page registers the rectangles it
    /// just drew, in paint order, so what answers a click is exactly what is on
    /// screen. The previous frame's targets go first — a widget that is no
    /// longer drawn must stop answering clicks.
    pub fn render(&mut self, frame: &mut Frame) {
        self.refresh_popup();
        self.refresh_changelog();
        self.hits.clear();

        let area = frame.area();
        frame.render_widget(
            Paragraph::new("").style(Style::default().bg(Theme::BG)),
            area,
        );

        // The toast takes the footer's line, so the list never gets pushed
        // under the bottom edge.
        let [header, body, footer] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(if self.toast.is_some() { 2 } else { 1 }),
        ])
        .areas(area);

        self.render_header(frame, header);
        // The page area the overlays are drawn into. The checkable overlay takes
        // the whole screen because it belongs to no one page; the path list takes
        // the results page's own area, because it describes one row of it and has
        // to stay inside the report it belongs to.
        match self.state.current_page {
            Page::Main => pages::main::render(self, frame, body),
            Page::ProgramSelection => pages::program_selection::render(self, frame, body),
            Page::Clearing => pages::clearing::render(self, frame, body),
            // The path overlay is drawn into the same area the report was, below
            // in [`TuiApp::render_details`]: it describes one row of it and has
            // to stay inside the frame the report drew around itself.
            Page::Results => pages::results::render(self, frame, body),
            Page::Settings => pages::settings::render(self, frame, body),
        }
        self.render_footer(frame, footer);

        // The overlay is drawn from `self.popup` while its targets go into
        // `self.hits`, so the borrow is split rather than taken twice. It comes
        // after the page, which is what puts it on top of the page's targets.
        let Self { popup, hits, .. } = self;
        if let Some(popup) = popup {
            render_popup(popup, hits, frame, area);
        }
        // Same arrangement for the path overlay, drawn into the results page's
        // area rather than the screen: its backdrop covers the report only, so a
        // click beside the box inside it closes the list, and the header and
        // footer outside it are never covered.
        if self.details.is_some() && self.state.current_page == Page::Results {
            pages::results::render_details(self, frame, body);
        }
        pages::update::render(self, frame, area);
        if self.changelog_open {
            self.render_changelog(frame, area);
        }
    }

    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let line = Line::from(vec![
            Span::styled(" Cross Cleaner ", Theme::heading()),
            Span::raw("  "),
            Span::styled(
                format!("[{}]", self.state.current_page.title()),
                Style::default().fg(Theme::ACCENT),
            ),
        ]);
        frame.render_widget(Paragraph::new(line), area);
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        if let Some(toast) = &self.toast {
            let remaining = toast.until.saturating_duration_since(Instant::now());
            let line = Line::from(vec![
                Span::styled(format!(" {} ", toast.message), toast.style),
                Span::styled(format!("· {}s", remaining.as_secs() + 1), Theme::dim()),
            ]);
            frame.render_widget(Paragraph::new(line), area);
            return;
        }
        // A toast outranks the download: it reports something that happened to
        // the user's data, while the download is ambient and still visible a
        // moment later.
        if let Some(progress) = &self.update_progress {
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(format!(" {progress} "), Style::default().fg(Theme::ACCENT)),
                    Span::styled("· U for details", Theme::dim()),
                ])),
                area,
            );
            return;
        }
        frame.render_widget(
            Paragraph::new(Line::from(self.footer_hints())).style(Theme::dim()),
            area,
        );
    }

    /// Context-sensitive key hints, the terminal stand-in for tooltips.
    fn footer_hints(&self) -> Vec<Span<'static>> {
        let page: &[&str] = match self.state.current_page {
            // The right button opens the same overlay as `Enter`, so it shares
            // the hint rather than adding a line the footer has no room for —
            // a hint that gets clipped away teaches nothing.
            Page::Main => &[
                "↑↓ row",
                "tab column",
                "space select",
                "enter/right subs",
                "n next",
                "s settings",
            ],
            Page::ProgramSelection => &[
                "↑↓ move",
                "space select",
                "→/right cats",
                "/ search",
                "S start",
            ],
            Page::Clearing => &["cleaning, please wait"],
            Page::Results => &["↑↓ scroll", "enter paths", "esc back"],
            Page::Settings => &["↑↓ pick", "←→ volume", "r reset", "esc back"],
        };
        let mut global: Vec<&str> = vec!["? changelog", "G repo"];
        if self.update_release.is_some() {
            global.push("O release page");
            global.push("U update");
        }
        global.push("q quit");
        let global = global.as_slice();

        let mut spans = Vec::new();
        for group in [page, global] {
            for (index, hint) in group.iter().enumerate() {
                if index > 0 || !spans.is_empty() {
                    spans.push(Span::styled(" · ", Theme::dim()));
                }
                spans.push(Span::styled(*hint, Style::default().fg(Theme::TEXT_DIM)));
            }
        }
        spans
    }

    /// Draws the "What's New" changelog overlay.
    ///
    /// A release note is far longer than the overlay, so the content is scrolled
    /// with an explicit offset and the position is shown in the title; a
    /// `Paragraph` on its own would just clip whatever does not fit.
    fn render_changelog(&mut self, frame: &mut Frame, area: Rect) {
        let width = (area.width * 3 / 4).clamp(40, 84);
        let height = (area.height * 3 / 4).max(10).min(area.height);
        let rect = centered(area, width, height);
        frame.render_widget(Clear, rect);

        let lines = changelog_lines(self.changelog.as_ref());

        let block = Theme::block(" What's New ", true).padding(Padding::uniform(1));
        // Borders and padding eat two rows and one column on each side; a very
        // short terminal can leave nothing at all, hence the saturating math.
        let viewport_height = block.inner(rect).height as usize;

        let max_scroll = lines.len().saturating_sub(viewport_height);
        // Clamp here rather than on the key press: the length is only known once
        // the fetch lands, and it can grow while the overlay is open.
        self.changelog_scroll = self.changelog_scroll.min(max_scroll);
        self.changelog_max_scroll = max_scroll;

        let position = format!(
            " {}/{} · {}esc close ",
            self.changelog_scroll + 1,
            lines.len().max(1),
            if max_scroll > 0 {
                "↑↓ scroll · "
            } else {
                ""
            },
        );
        let block = block.title_bottom(Line::from(position).right_aligned());

        frame.render_widget(
            Paragraph::new(lines)
                .block(block)
                .wrap(Wrap { trim: false })
                .scroll((self.changelog_scroll as u16, 0)),
            rect,
        );

        // Nothing inside the notes is clickable — they are text — so the whole
        // screen is the dismissal, exactly as `Esc` is.
        self.hits.push(area, Click::CloseChangelog);
    }
}

/// Draws the centered checkable overlay, and registers what it answers.
///
/// An entry toggles its own checkbox; a click anywhere else closes the overlay,
/// the way `Esc` does. The backdrop goes in before the entries so the entries
/// are found first — a lookup takes the topmost target.
fn render_popup(popup: &Popup, hits: &mut HitAreas, frame: &mut Frame, area: Rect) {
    let width = (area.width * 2 / 3).clamp(30, 64);
    // `clamp` panics when its bounds are inverted, which a terminal shorter than
    // the minimum would do — the overlay then fills the screen rather than
    // taking the frame down.
    let height = (popup.items.len() as u16 + 4)
        .max(6)
        .min(area.height.saturating_sub(2).max(6));
    let rect = centered(area, width, height);
    frame.render_widget(Clear, rect);

    let items: Vec<ratatui::widgets::ListItem> = popup
        .items
        .iter()
        .map(|(label, selected)| {
            let (mark, mark_style) = Theme::checkbox(*selected, false);
            ratatui::widgets::ListItem::new(Line::from(vec![
                Span::styled(format!("{mark} "), mark_style),
                Span::styled(label.clone(), Theme::text()),
            ]))
        })
        .collect();

    let block = Theme::block(&popup.title, true)
        .title_bottom(Line::from(" ↑↓ move · space toggle · esc close ").right_aligned());
    // Taken before the block is handed to the list, which consumes it.
    let inner = block.inner(rect);
    let list = ratatui::widgets::List::new(items)
        .block(block)
        .highlight_style(Theme::selected());

    let mut list_state = popup.list;
    frame.render_stateful_widget(list, rect, &mut list_state);

    // The backdrop covers the whole screen, so a click outside the box closes the
    // overlay instead of reaching the page underneath. Registered first, so the
    // entries below win where they overlap.
    hits.push(area, Click::ClosePopup);
    // The list is scrolled to the selected entry, and `List` writes the offset it
    // ended up with back into the state — so the rows on screen are only known
    // after drawing, not before.
    let offset = list_state.offset();
    for row in 0..inner.height {
        hits.push(
            pages::row_of(inner, row, inner.width),
            Click::PopupItem(offset + row as usize),
        );
    }
}

/// Flattens a changelog into display lines.
fn changelog_lines(changelog: Option<&Changelog>) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let bullet = |item: &str| {
        Line::from(vec![
            Span::styled(format!(" {} ", Theme::BULLET), Theme::dim()),
            Span::styled(item.to_string(), Theme::text()),
        ])
    };

    let Some(changelog) = changelog else {
        lines.push(Line::styled("Loading changelog...", Theme::dim()));
        return lines;
    };
    if changelog.groups.is_empty() && changelog.contributors.is_empty() {
        lines.push(Line::styled("No changes found.", Theme::dim()));
        return lines;
    }
    for group in &changelog.groups {
        lines.push(Line::default());
        lines.push(Line::styled(group.title.clone(), Theme::heading()));
        lines.extend(group.items.iter().map(|item| bullet(item)));
    }
    if !changelog.contributors.is_empty() {
        lines.push(Line::default());
        lines.push(Line::styled("Contributors", Theme::heading()));
        lines.extend(changelog.contributors.iter().map(|c| bullet(c)));
    }
    lines
}

/// A `width` x `height` rectangle centered inside `area`.
pub(crate) fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}
