//! Settings page: the four sound volumes, stepped with the arrow keys — the
//! terminal counterpart of `gui::pages::settings`.

use appcore::config;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState};

use crate::app::{Click, TuiApp};
use crate::pages::{row_of, span_of};
use crate::theme::Theme;

/// Width of a volume bar in cells.
///
/// `pub` so a test can place a click on the bar without repeating the number,
/// which is what would make the click land on the wrong column unnoticed.
pub const BAR_WIDTH: usize = 24;

/// Draws the volume list.
///
/// Each row is clickable: the bar sets the level under the pointer, which is the
/// terminal stand-in for the window frontend's slider, and the rest of the row
/// just moves the cursor there so the arrow keys start from what was clicked.
pub fn render(app: &mut TuiApp, frame: &mut Frame, area: Rect) {
    let cfg = config::get();
    let values = [
        cfg.sound_volume,
        cfg.click_volume,
        cfg.check_volume,
        cfg.done_volume,
    ];
    let labels = ["Popup sound", "Click sound", "Check sound", "Done sound"];

    let items: Vec<ListItem<'static>> = labels
        .iter()
        .zip(values)
        .map(|(label, value)| {
            let mut spans = vec![
                Span::styled(format!(" {label}"), Theme::text()),
                Span::raw(" "),
            ];
            spans.extend(bar(value, BAR_WIDTH));
            spans.push(Span::styled(
                format!(" {:>3.0}%", value * 100.0),
                Theme::dim(),
            ));
            ListItem::new(Line::from(spans))
        })
        .collect();

    let list = List::new(items)
        .block(
            Theme::block(" Sound volumes ", true)
                .title_bottom(Line::from(" ←→ adjust · r reset · esc back ").right_aligned()),
        )
        .highlight_style(
            ratatui::style::Style::default()
                .fg(Theme::TEXT)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸");

    let mut state = ListState::default().with_selected(Some(app.settings_cursor));
    frame.render_stateful_widget(list, area, &mut state);

    // The offset is ratatui's to decide — it scrolls to keep the cursor visible
    // — so the rows are only known once the list has been drawn.
    let inner = Theme::block(" Sound volumes ", true).inner(area);
    register_rows(app, &labels, inner, state.offset());
}

/// Registers the visible volume rows.
fn register_rows(app: &mut TuiApp, labels: &[&str], inner: Rect, offset: usize) {
    for row in 0..inner.height as usize {
        let Some(label) = labels.get(offset + row) else {
            break;
        };
        // A row is the highlight symbol, then the label with its leading space,
        // then one space, then the bar. The symbol takes a column on every row,
        // not only the focused one, so it is part of where the bar starts.
        let bar_left = 1 + 1 + label.chars().count() as u16 + 1;
        app.hit(
            row_of(inner, row as u16, inner.width),
            Click::Settings(offset + row),
        );
        // Registered after the row, so it wins: a click on the bar sets the level
        // instead of only moving the cursor there.
        app.hit(
            span_of(inner, row as u16, bar_left, BAR_WIDTH as u16),
            Click::Volume {
                slot: offset + row,
                left: inner.x + bar_left,
            },
        );
    }
}

/// The level a click at `column` sets a bar that starts at `left` to.
///
/// The bar spans the whole range, so the pointer position maps onto 0.0 to 1.0
/// directly — the point of a click on a slider, rather than a step per click.
pub(crate) fn level_at(left: u16, column: u16) -> f32 {
    let steps = column.saturating_sub(left);
    (steps as f32 / (BAR_WIDTH.saturating_sub(1)) as f32).clamp(0.0, 1.0)
}

/// A horizontal volume bar: filled for the level, then a track.
fn bar(value: f32, width: usize) -> Vec<Span<'static>> {
    Theme::volume_meter(value, width)
}
