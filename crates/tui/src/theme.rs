//! Colors, borders and glyphs, mirroring the dark palette the window frontend
//! uses so both apps look like the same product.

use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols;
use ratatui::text::Span;
use ratatui::widgets::{Block, BorderType};

/// Named palette entries, so pages never hardcode a raw color.
pub struct Theme;

impl Theme {
    pub const BG: Color = Color::Rgb(24, 24, 28);
    pub const PANEL: Color = Color::Rgb(32, 32, 38);
    pub const BORDER: Color = Color::Rgb(72, 72, 82);
    pub const BORDER_FOCUSED: Color = Color::Rgb(0, 120, 215);
    pub const TEXT: Color = Color::Rgb(222, 222, 228);
    pub const TEXT_DIM: Color = Color::Rgb(140, 140, 152);
    pub const ACCENT: Color = Color::Rgb(0, 120, 215);
    pub const GOOD: Color = Color::Rgb(60, 180, 90);
    pub const WARN: Color = Color::Rgb(220, 170, 40);
    pub const BAD: Color = Color::Rgb(220, 70, 70);

    /// Ordinary body text.
    pub fn text() -> Style {
        Style::default().fg(Self::TEXT)
    }

    /// De-emphasised text: counts, hints, secondary columns.
    pub fn dim() -> Style {
        Style::default().fg(Self::TEXT_DIM)
    }

    /// The currently highlighted row / focused entry.
    pub fn selected() -> Style {
        Style::default()
            .bg(Self::ACCENT)
            .fg(Color::Rgb(255, 255, 255))
            .add_modifier(Modifier::BOLD)
    }

    /// A heading (`Select Programs to Clean`, `Cleaning Results`, ...).
    pub fn heading() -> Style {
        Style::default()
            .fg(Self::TEXT)
            .add_modifier(Modifier::BOLD)
    }

    /// A column header row.
    pub fn column_header() -> Style {
        Style::default()
            .fg(Self::TEXT_DIM)
            .add_modifier(Modifier::BOLD)
    }

    /// A block border; `focused` switches it to the accent color.
    pub fn border(focused: bool) -> Style {
        Style::default().fg(if focused { Self::BORDER_FOCUSED } else { Self::BORDER })
    }

    /// A titled block around a page area.
    pub fn block(title: &str, focused: bool) -> Block<'static> {
        Block::bordered()
            .title(title.to_string())
            .border_type(BorderType::Rounded)
            .border_style(Self::border(focused))
            .style(Style::default().bg(Self::BG))
    }

    /// The three states a checkbox can be in.
    pub fn checkbox(checked: bool, indeterminate: bool) -> (&'static str, Style) {
        match (checked, indeterminate) {
            (true, _) => ("[x]", Style::default().fg(Self::GOOD)),
            (false, true) => ("[-]", Style::default().fg(Self::WARN)),
            (false, false) => ("[ ]", Style::default().fg(Self::TEXT_DIM)),
        }
    }

    /// Glyphs used by the popup and the progress spinner.
    pub const BULLET: &'static str = "•";
    pub const SPINNER: [&'static str; 8] = [
        "⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧",
    ];

    /// A horizontal meter drawn from eighth blocks, used for the cleaning progress
    /// bar and the volume bars.
    pub fn meter(fraction: f32, width: usize, fill: Color, track: Color) -> Vec<Span<'static>> {
        let fraction = fraction.clamp(0.0, 1.0);
        let eighths = (((fraction * width as f32) * 8.0).round() as usize).min(width * 8);
        let full = eighths / 8;
        let rest = eighths % 8;

        let mut bar = String::new();
        for _ in 0..full {
            bar.push_str(symbols::block::FULL);
        }
        // A partially filled cell, unless the bar is already complete.
        let partial = rest > 0 && full < width;
        if partial {
            bar.push_str(EIGHTHS[rest - 1]);
        }

        let mut spans = Vec::new();
        if !bar.is_empty() {
            spans.push(Span::styled(bar, Style::default().fg(fill)));
        }
        let used = full + usize::from(partial);
        if used < width {
            spans.push(Span::styled(" ".repeat(width - used), Style::default().fg(track)));
        }
        spans
    }

    /// A meter colored by meaning: silence is red, a real level is green.
    pub fn volume_meter(fraction: f32, width: usize) -> Vec<Span<'static>> {
        let level = if fraction <= 0.001 { Self::BAD } else { Self::GOOD };
        Self::meter(fraction, width, level, Self::BORDER)
    }
}

/// The partial block glyphs from 1/8 to 7/8. Ratatui exposes them as separate
/// constants rather than an array, so the table is assembled once here.
const EIGHTHS: [&str; 7] = [
    symbols::block::ONE_EIGHTH,
    symbols::block::ONE_QUARTER,
    symbols::block::THREE_EIGHTHS,
    symbols::block::HALF,
    symbols::block::FIVE_EIGHTHS,
    symbols::block::THREE_QUARTERS,
    symbols::block::SEVEN_EIGHTHS,
];