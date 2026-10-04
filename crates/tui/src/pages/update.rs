//! Update dialog: the terminal counterpart of the window frontend's update
//! notification. It reads everything from the shared [`appcore::updater`]
//! state, so the worker thread stays the single source of truth and the dialog
//! never shows a stale stage.

use appcore::updater::{self, UpdateStage};
use database::version::NewRelease;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph, Wrap};

use crate::app::{TuiApp, centered};
use crate::theme::Theme;

/// Width of the download bar, in columns.
const BAR_WIDTH: usize = 46;

/// What the dialog should show right now.
pub enum Prompt {
    /// Nothing to report, or the user dismissed it.
    Hidden,
    /// A release was found and the user has not acted yet. A release without a
    /// binary for this platform lands here too, with the download step removed.
    Offer,
    /// The worker is running, or its result is waiting for an answer.
    Stage(UpdateStage),
}

/// Picks the dialog content from the pending release and the worker stage.
pub fn prompt(app: &TuiApp) -> Prompt {
    let stage = updater::current(&app.updater_state);
    // A running download or install always wins: it must not be dismissible.
    if stage.is_running() {
        return Prompt::Stage(stage);
    }
    if !app.update_open || app.update_release.is_none() {
        return Prompt::Hidden;
    }
    match stage {
        // The worker never started, so this is still the initial offer.
        UpdateStage::Idle => Prompt::Offer,
        _ => Prompt::Stage(stage),
    }
}

/// Draws the dialog when there is something to show.
pub fn render(app: &mut TuiApp, frame: &mut Frame, area: Rect) {
    let time = app.started.elapsed().as_secs_f64();
    let spinner = app.spinner_frame();

    let Some(release) = app.update_release.clone() else {
        return;
    };

    let (title, body) = match prompt(app) {
        Prompt::Hidden => return,
        Prompt::Offer => (
            format!(" Update to v{} ", release.version),
            offer_body(&release),
        ),
        Prompt::Stage(stage) => (
            format!(" {} ", updater::stage_heading(&stage)),
            stage_body(&stage, time, spinner),
        ),
    };

    let height = body.len() as u16 + 4;
    let rect = centered(area, 64, height.min(area.height));
    frame.render_widget(Clear, rect);

    frame.render_widget(
        Paragraph::new(body)
            .block(Theme::block(&title, true))
            .wrap(Wrap { trim: false }),
        rect,
    );
}

/// The release was found and can be installed: offer to do it.
fn offer_body(release: &NewRelease) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(Span::styled(
            format!(
                "v{} is available. You have v{}.",
                release.version,
                database::get_version()
            ),
            Theme::text(),
        )),
        Line::default(),
        Line::from(Span::styled(
            "d   download and install",
            Style::default().fg(Theme::ACCENT),
        )),
        Line::from(Span::styled("o   open the release page", Theme::dim())),
        Line::from(Span::styled("?   what changed", Theme::dim())),
        Line::from(Span::styled("esc later", Theme::dim())),
    ];
    if !release.has_asset() {
        lines.insert(
            1,
            Line::from(Span::styled(
                "No terminal binary for this platform — download it manually.",
                Style::default().fg(Theme::WARN),
            )),
        );
        lines.retain(|line| !line.to_string().starts_with("d   "));
    }
    lines
}

/// The worker owns the update: show what it is doing and what to press next.
fn stage_body(stage: &UpdateStage, time: f64, spinner: &'static str) -> Vec<Line<'static>> {
    match stage {
        UpdateStage::Idle => Vec::new(),
        UpdateStage::Downloading {
            version,
            done,
            total,
        } => vec![
            Line::from(Span::styled(
                format!(
                    "downloading v{version} — {}",
                    updater::format_progress(*done, *total)
                ),
                Theme::text(),
            )),
            Line::default(),
            Line::from(Theme::meter(
                updater::progress_fraction(*done, *total, time),
                BAR_WIDTH,
                Theme::ACCENT,
                Theme::BORDER,
            )),
            Line::default(),
            Line::from(Span::styled("please wait", Theme::dim())),
        ],
        UpdateStage::Installing { version } => vec![
            Line::from(vec![
                Span::styled(format!(" {spinner} "), Style::default().fg(Theme::ACCENT)),
                Span::styled(format!("installing v{version}"), Theme::text()),
            ]),
            Line::default(),
            Line::from(Span::styled(
                "the new version starts after a restart",
                Theme::dim(),
            )),
        ],
        UpdateStage::Installed { version } => vec![
            Line::from(Span::styled(
                format!("v{version} is installed. Restart to use it?"),
                Theme::text(),
            )),
            Line::default(),
            key("r", "restart now"),
            key("esc", "later"),
        ],
        UpdateStage::RestartFailed { version, error } => vec![
            Line::from(Span::styled(
                format!("v{version} is installed, but the restart failed."),
                Theme::text(),
            )),
            Line::from(Span::styled(
                error.clone(),
                Style::default().fg(Theme::WARN),
            )),
            Line::default(),
            Line::from(Span::styled(
                "close and reopen Cross Cleaner to use the new version",
                Theme::dim(),
            )),
            Line::default(),
            key("r", "try again"),
            key("esc", "later"),
        ],
        UpdateStage::Failed { version, error } => vec![
            Line::from(Span::styled(
                format!("update to v{version} failed"),
                Theme::text(),
            )),
            Line::from(Span::styled(
                error.clone(),
                Style::default().fg(Theme::WARN),
            )),
            Line::default(),
            key("r", "retry"),
            key("o", "release page"),
            key("?", "what changed"),
            key("esc", "later"),
        ],
    }
}

/// One `key   label` hint line.
fn key(key: &str, label: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!(" {key}"), Style::default().fg(Theme::ACCENT)),
        Span::styled(format!("   {label}"), Theme::dim()),
    ])
}
