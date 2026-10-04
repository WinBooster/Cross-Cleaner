//! Settings page: the four sound volumes, stepped with the arrow keys — the
//! terminal counterpart of `gui::pages::settings`.

use appcore::config;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState};

use crate::app::TuiApp;
use crate::theme::Theme;

/// Width of a volume bar in cells.
const BAR_WIDTH: usize = 24;

/// Draws the volume list.
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
}

/// A horizontal volume bar: filled for the level, then a track.
fn bar(value: f32, width: usize) -> Vec<Span<'static>> {
    Theme::volume_meter(value, width)
}
