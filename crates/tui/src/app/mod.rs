//! Terminal application state, input handling and rendering entry point.
//!
//! Mirrors [`gui::app::MyApp`] one-to-one: both wrap the same
//! [`appcore::AppState`] and offer the same five pages. The differences follow
//! from the medium — there is no texture and no native window, so an egui popup
//! becomes a centered overlay, the search field becomes an input mode, and the
//! title bar becomes a header line plus a footer of key hints.
//!
//! The mouse is the one input the window frontend gets for free. It is emulated
//! by hit testing: every widget that answers a click registers the rectangle it
//! was drawn into ([`TuiApp::hit`]), and a click is dispatched to the same
//! method its key press would have reached — see [`Click`]. Registering while
//! drawing is what keeps the two in step; a rectangle computed a second time, by
//! hand, would eventually point somewhere the glyph is not.
//!
//! This module holds only what belongs to no other concern: the state the pages
//! share, its construction, and the frame loop. Everything the app object drives
//! lives beside it, split by what it is —
//!
//! * [`input`] — every key and every mouse event, page by page;
//! * [`render`] — the header, the footer and the overlays drawn over a page;
//! * [`update`] — the version check, the self-update worker and its dialog;
//! * [`settings`] — the four sound volumes;
//! * [`click`] — what a click on a rectangle means, and where those rectangles
//!   are recorded;
//! * [`overlay`] — the small values drawn over a page: the checkable list, the
//!   deleted-path list, the toast;
//! * [`sound`] — the clip an action plays;
//! * [`diagnostics`] — the capture of library diagnostics that must never reach
//!   the screen this app is drawing into.

mod click;
mod diagnostics;
mod input;
mod overlay;
mod render;
mod settings;
mod sound;
mod update;

pub use diagnostics::{install_diagnostic_sink, remove_diagnostic_sink};
pub use overlay::{InputMode, PathDetails, Popup, PopupKind, Toast};

#[cfg(test)]
mod tests;

use std::time::Instant;

use appcore::app::AppState;
use appcore::sounds;
use appcore::updater::{self, UpdateState, UpdaterCommand};
use database::version::{Changelog, NewRelease};
use ratatui::style::Style;

use crate::theme::Theme;

pub(crate) use click::{Click, HitAreas, UpdateAction};
pub(crate) use render::centered;

use diagnostics::take_diagnostics;
use sound::sfx;

/// The terminal frontend's application object.
pub struct TuiApp {
    /// Selection, program list and cleaning job, shared with `gui`.
    pub state: AppState,

    /// Cursor on the category page.
    pub category_cursor: usize,
    /// Cursor on the program page, in *filtered* coordinates.
    pub program_cursor: usize,
    /// Cursor on the results table.
    pub result_cursor: usize,
    /// Cursor on the settings page.
    pub settings_cursor: usize,

    pub input: InputMode,
    pub popup: Option<Popup>,
    /// The open "which paths did this row delete" overlay.
    pub details: Option<PathDetails>,
    pub toast: Option<Toast>,

    /// Rectangles drawn by the last frame that answer a click, rebuilt by every
    /// [`TuiApp::render`].
    hits: HitAreas,

    /// "What's New" overlay state.
    pub changelog_open: bool,
    /// First visible line of the changelog.
    pub changelog_scroll: usize,
    /// Largest valid `changelog_scroll`, recomputed on every render from the
    /// fetched content and the overlay height.
    changelog_max_scroll: usize,
    changelog: Option<Changelog>,
    changelog_handle: Option<std::thread::JoinHandle<Changelog>>,

    /// Release offered by the background version check.
    pub update_release: Option<NewRelease>,
    update_receiver: Option<std::sync::mpsc::Receiver<Result<Option<NewRelease>, String>>>,
    /// True while the update dialog is on screen.
    pub update_open: bool,
    /// Set once the version check found a release this build can install and the
    /// download was started without waiting for a key press, the way the window
    /// frontend does it.
    auto_update: bool,
    /// Set once the dialog has been raised for a finished automatic update, so a
    /// user who answered it does not get it back on the next tick.
    update_announced: bool,
    /// Footer line for the automatic update while the worker runs.
    ///
    /// Its own field rather than a toast: a toast is transient and gets
    /// overwritten by the next one — a cleaner warning or "Cleaning finished." —
    /// which would make the download flicker in and out. A toast still wins the
    /// line, because it reports something that happened to the user's data.
    update_progress: Option<String>,
    /// Progress of the self-update, written by the `selfupdate` worker thread.
    pub updater_state: UpdateState,
    /// Channel to the self-update worker. `None` when no worker was started,
    /// which disables the in-app update and falls back to the release page.
    pub updater_tx: Option<std::sync::mpsc::Sender<UpdaterCommand>>,

    /// Set when the user asked to quit.
    pub should_quit: bool,

    /// App start, used to advance the spinner animation and the indeterminate
    /// download bar.
    pub started: Instant,
}

impl TuiApp {
    /// Builds the app around a prepared [`AppState`].
    pub fn new(state: AppState) -> Self {
        // Only the version check below writes to `app`, so without the
        // `self-update` feature it is never mutated after construction.
        #[allow(unused_mut)]
        let mut app = Self {
            state,
            category_cursor: 0,
            program_cursor: 0,
            result_cursor: 0,
            settings_cursor: 0,
            input: InputMode::Normal,
            popup: None,
            details: None,
            toast: None,
            hits: HitAreas::default(),
            changelog_open: false,
            changelog_scroll: 0,
            changelog_max_scroll: 0,
            changelog: None,
            changelog_handle: None,
            update_release: None,
            update_receiver: None,
            update_open: false,
            auto_update: false,
            update_announced: false,
            update_progress: None,
            updater_state: updater::new_state(),
            updater_tx: None,
            should_quit: false,
            started: Instant::now(),
        };
        #[cfg(feature = "self-update")]
        app.start_update_check();
        app
    }

    // --- frame loop ------------------------------------------------------

    /// Advances time-based state and drains the channels the cleaning job and
    /// the version check write to. Cheap, so it can run on every tick.
    pub fn tick(&mut self) {
        self.state.drain_progress();
        if self.state.poll_result() {
            self.result_cursor = 0;
            // A new run replaces the report, so an overlay pointing into the old
            // one has nothing left to describe.
            self.details = None;
            self.toast = Some(Toast::info("Cleaning finished."));
            // The end of a run, so the window frontend's done clip belongs here
            // too — otherwise a long clean gives no sign it ever finished.
            sfx("done", sounds::done);
        }

        if let Some(receiver) = &self.update_receiver {
            match receiver.try_recv() {
                Ok(Ok(Some(release))) => {
                    self.update_receiver = None;
                    self.accept_release(release);
                }
                // Nothing newer: the ordinary case, nothing worth a toast.
                Ok(Ok(None)) => self.update_receiver = None,
                // A failed check used to be dropped on the floor, which made
                // "the updater did nothing" indistinguishable from "there was
                // nothing to do" — the one thing a user reporting a silent
                // updater needs to be able to tell apart. GitHub also answers an
                // unauthenticated client with 403 once the hourly rate limit is
                // spent, so this is not a rare path.
                Ok(Err(error)) => {
                    self.update_receiver = None;
                    self.toast = Some(Toast::warn(format!("Update check failed: {error}")));
                }
                // Nothing to read *yet*, which is the normal state for the first
                // second or so: the request is still in flight. Keep polling.
                //
                // This must not be lumped in with `Disconnected`. The tick is 100 ms
                // and the request takes about a second and a half, so treating
                // "not ready" as "not coming" dropped the receiver before the answer
                // could ever arrive — which is why the terminal app looked like it
                // had no updater at all.
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                // The checking thread is gone and sent nothing: it panicked.
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.update_receiver = None;
                    self.toast = Some(Toast::warn("Update check did not finish."));
                }
            }
        }
        self.poll_update();

        if self
            .toast
            .as_ref()
            .is_some_and(|t| Instant::now() >= t.until)
        {
            self.toast = None;
        }

        // Surface whatever the cleaners reported. Only the newest is shown and
        // the rest are counted, so a run that fails on every entry cannot bury
        // the interface under a wall of toasts.
        let diagnostics = take_diagnostics();
        if let Some(last) = diagnostics.last() {
            let more = diagnostics.len().saturating_sub(1);
            let message = if more == 0 {
                last.clone()
            } else {
                format!("{last} (+{more} more)")
            };
            self.toast = Some(Toast::new(message, Style::default().fg(Theme::WARN)));
        }
    }

    /// Current frame of the spinner animation, for the pages that show one.
    pub fn spinner_frame(&self) -> &'static str {
        let step = (self.started.elapsed().as_millis() / 100) as usize;
        Theme::SPINNER[step % Theme::SPINNER.len()]
    }

    // --- cursors ----------------------------------------------------------

    /// Clamps every cursor after a page change, so a stale index can never
    /// highlight a row that no longer exists.
    pub(super) fn sync_cursors(&mut self) {
        self.category_cursor = self
            .category_cursor
            .min(self.state.categories.len().saturating_sub(1));
        self.program_cursor = self
            .program_cursor
            .min(self.state.filtered_programs.len().saturating_sub(1));
        self.result_cursor = self
            .result_cursor
            .min(self.result_count().saturating_sub(1));
    }
}
