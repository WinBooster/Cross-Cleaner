//! Replacing this executable with a newer release.
//!
//! The version check runs on a background thread and answers into a channel that
//! [`TuiApp::tick`](super::TuiApp::tick) drains; a release this build can install
//! is downloaded straight away, exactly as the window frontend does it. Every one
//! of the dialog's choices arrives here — from a key through
//! [`TuiApp::on_key_update`](super::TuiApp::on_key_update), from a click through
//! [`Click::Update`](super::Click::Update) — so the two ways in cannot disagree.

use appcore::sounds;
use appcore::updater::{self, UpdaterCommand};
use crossterm::event::{KeyCode, KeyEvent};
#[cfg(feature = "self-update")]
use database::version::Frontend;
use database::version::NewRelease;

use crate::theme::Theme;
use ratatui::style::Style;

use super::overlay::Toast;
use super::sound::sfx;
use super::{TuiApp, UpdateAction};

impl TuiApp {
    /// Starts the self-update worker.
    ///
    /// The worker is what lets the terminal app replace its own executable, and
    /// it lives in the `selfupdate` crate so the window frontend shares it.
    /// Nothing to start without the `self-update` feature: `updater_tx` stays
    /// empty, and without the version check there is no update to begin with.
    pub fn start_self_update(&mut self) {
        #[cfg(feature = "self-update")]
        if self.updater_tx.is_none() {
            let (tx, rx) = std::sync::mpsc::channel();
            let state = self.updater_state.clone();
            self.updater_tx = Some(tx);
            std::thread::spawn(move || selfupdate::run(rx, state));
        }
    }

    /// Asks GitHub for the latest release.
    ///
    /// [`Frontend::Tui`] matters: the release ships one binary per frontend, and
    /// resolving the GUI asset would install the window app over the terminal one.
    ///
    /// Without the `self-update` feature the check is not made at all, which is
    /// what keeps the update notification out of a distribution package: no
    /// release is ever found, so there is nothing to announce or offer.
    #[cfg(feature = "self-update")]
    pub(super) fn start_update_check(&mut self) {
        let (tx, rx) = std::sync::mpsc::channel();
        self.update_receiver = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(database::version::check_new_version_for(Frontend::Tui));
        });
    }

    /// Handles a release the background check found.
    ///
    /// A release this build can install is downloaded straight away, exactly as
    /// the window frontend does it: waiting for a key press made the terminal app
    /// look like it had no updater at all, when it only had one behind a
    /// shortcut. Anything else — no worker, or no binary for this platform —
    /// still points at the release page, which is all it can honestly offer.
    pub(super) fn accept_release(&mut self, release: NewRelease) {
        let version = release.version.clone();
        self.update_release = Some(release.clone());
        if !self.can_install(&release) {
            database::diag::info(format!(
                "[updater] v{version} found, but this build cannot install it (worker={}, asset={})",
                self.updater_tx.is_some(),
                release.has_asset(),
            ));
            self.toast = Some(Toast::new(
                format!("New version v{version} available — press O for the release page"),
                Style::default().fg(Theme::GOOD),
            ));
            return;
        }
        database::diag::info(format!("[updater] v{version} found, downloading it now"));
        self.start_update();
    }

    /// True when this build can replace its own executable with `release`.
    pub(super) fn can_install(&self, release: &NewRelease) -> bool {
        self.updater_tx.is_some() && release.has_asset()
    }

    /// Watches the worker while an automatic update is in flight.
    ///
    /// Two things have to happen without a key press: the footer keeps reporting
    /// the download, and the dialog appears as soon as the worker needs an
    /// answer. It waits for that moment rather than opening on the first byte —
    /// a modal that swallows the keyboard for the length of a download would
    /// lock the app out of its own cleaning run.
    pub(super) fn poll_update(&mut self) {
        if !self.auto_update {
            return;
        }
        let stage = updater::current(&self.updater_state);
        if stage.is_running() {
            // Also covers a retry started from the dialog: the footer has to
            // follow it, not just the first attempt.
            self.update_progress = Some(update_progress_line(&stage));
            return;
        }
        self.update_progress = None;
        if self.update_announced {
            return;
        }
        // Done either way: installed, or failed with something to retry. Raised
        // once, so dismissing it is final.
        self.update_announced = true;
        let was_open = self.update_open;
        self.update_open = true;
        if !was_open {
            sfx("pop", sounds::pop);
        }
    }

    /// Keys of the update dialog.
    ///
    /// Only the mapping lives here: every key names an [`UpdateAction`] and
    /// hands it to [`TuiApp::run_update`], so a click on the same row cannot end
    /// up doing something else.
    pub(super) fn on_key_update(&mut self, key: KeyEvent) {
        let action = match key.code {
            KeyCode::Esc => Some(UpdateAction::Later),
            KeyCode::Char('?') | KeyCode::F(1) => Some(UpdateAction::Changelog),
            KeyCode::Char('o') | KeyCode::Char('O') => Some(UpdateAction::ReleasePage),
            KeyCode::Char('d') | KeyCode::Char('D') => Some(UpdateAction::Install),
            KeyCode::Char('r') | KeyCode::Char('R') => Some(UpdateAction::Confirm),
            _ => None,
        };
        if let Some(action) = action {
            self.run_update(action);
        }
    }

    /// Carries out one of the dialog's choices, from a key or from a click.
    ///
    /// The dialog is the only place that can start an install or a relaunch, so
    /// it owns both inputs while it is up — except for the running stages,
    /// where there is nothing safe to press or click. Starting a second download
    /// on top of a running one would interleave two writes to the same
    /// executable, and closing mid-download would strand the user on the old
    /// binary. The changelog and the release page stay open: neither touches the
    /// worker.
    pub(super) fn run_update(&mut self, action: UpdateAction) {
        let unsafe_while_running = matches!(
            action,
            UpdateAction::Install | UpdateAction::Confirm | UpdateAction::Later
        );
        if unsafe_while_running && updater::current(&self.updater_state).is_running() {
            return;
        }
        match action {
            UpdateAction::Install => self.start_update(),
            UpdateAction::Confirm => self.confirm_or_retry(),
            UpdateAction::ReleasePage => self.open_release_page(),
            UpdateAction::Changelog => self.open_changelog(),
            UpdateAction::Later => {
                self.update_open = false;
                self.toast = Some(Toast::info("Update postponed."));
            }
        }
    }

    /// Opens the release page of the pending update.
    pub(super) fn open_release_page(&mut self) {
        match &self.update_release {
            Some(release) => {
                appcore::browser::open_in_browser(&release.url);
                self.toast = Some(Toast::info("Opened the release page."));
            }
            None => self.toast = Some(Toast::warn("No pending update.")),
        }
    }

    /// Downloads the pending release and replaces the running executable.
    pub(super) fn start_update(&mut self) {
        if self.updater_tx.is_none() {
            self.toast = Some(Toast::warn("Self-update is not available in this build."));
            return;
        }
        let Some(release) = self.update_release.clone() else {
            return;
        };
        if !release.has_asset() {
            self.toast = Some(Toast::warn(
                "This release has no terminal binary for your platform.",
            ));
            return;
        }
        let version = release.version.clone();
        let total = release.asset_size;
        // From here on this app is the one driving the update, so `tick` keeps
        // the footer on it and raises the dialog when it needs an answer. Set
        // before the send, because a full channel already ends the attempt.
        self.auto_update = true;
        // Publish the opening stage right away so the dialog never shows an idle
        // bar while the worker picks the command up.
        updater::publish(
            &self.updater_state,
            updater::UpdateStage::Downloading {
                version,
                done: 0,
                total,
            },
        );
        if let Err(e) = self
            .updater_tx
            .as_ref()
            .expect("checked above")
            .send(UpdaterCommand::Install(release))
        {
            updater::publish(
                &self.updater_state,
                updater::UpdateStage::Failed {
                    version: "?".to_string(),
                    error: format!("The updater stopped responding: {e}"),
                },
            );
        }
    }

    /// `r` either restarts into the freshly installed version, or retries a
    /// failed download.
    pub(super) fn confirm_or_retry(&mut self) {
        let stage = updater::current(&self.updater_state);
        if stage.is_running() {
            return;
        }
        let send = |command: UpdaterCommand| -> Result<(), String> {
            let Some(tx) = &self.updater_tx else {
                return Err("Self-update is not available in this build.".to_string());
            };
            tx.send(command)
                .map_err(|e| format!("The updater stopped responding: {e}"))
        };
        let outcome = if stage.is_installed() {
            // Never restart mid-download: the worker owns the executable then.
            send(UpdaterCommand::Restart)
        } else {
            self.start_update();
            Ok(())
        };
        if let Err(error) = outcome {
            self.toast = Some(Toast::warn(error));
        }
    }
}

/// Footer text for an update the worker is still performing.
///
/// Separate from the dialog's own wording: the dialog is modal and spells out
/// the keys, while this line is a status readout that has to fit on one row
/// next to a download already in motion.
fn update_progress_line(stage: &updater::UpdateStage) -> String {
    match stage {
        updater::UpdateStage::Downloading {
            version,
            done,
            total,
        } => format!(
            "updating to v{version} — {}",
            updater::format_progress(*done, *total)
        ),
        updater::UpdateStage::Installing { version } => format!("installing v{version}"),
        // Unreachable through `poll_update`, which only calls this while the
        // stage is running. Kept exhaustive so a new stage cannot be forgotten.
        _ => updater::stage_heading(stage),
    }
}
