//! `cli update` — the self-update path, printed as lines.
//!
//! [`selfupdate::run`] is the worker the other frontends spawn onto a thread
//! and render as a dialog; here it runs on its own thread and its
//! [`appcore::updater::UpdateStage`] is polled and printed. The state machine,
//! the download and the file replacement are all the shared code — this only
//! decides what a stage looks like on a terminal.
//!
//! Restarting is the one thing that needs care: the worker calls
//! `std::process::exit` to hand the executable back to `self_replace`, so
//! anything still buffered has to be flushed before the stage reaches
//! `Installed`.

use std::sync::Arc;
use std::sync::mpsc::channel;
use std::time::Duration;

use appcore::updater::{UpdateStage, UpdaterCommand, current, format_progress, new_state};

use database::version::Frontend;

use crate::args::UpdateArgs;
use crate::term::Ui;

/// How often the shared state is read while a download is in flight.
const POLL: Duration = Duration::from_millis(120);

/// Checks for a newer release and, unless `--check`, installs it.
pub fn run(ui: &Ui, args: &UpdateArgs) -> Result<(), String> {
    let release = database::version::check_new_version_for(Frontend::Cli)?;
    let Some(release) = release else {
        ui.line(&format!(
            "Cross Cleaner {} is up to date.",
            database::get_version()
        ));
        return Ok(());
    };

    ui.line(&format!(
        "Cross Cleaner {} is available (you have {}).",
        release.version,
        database::get_version()
    ));

    if args.check {
        ui.line(&release.url);
        return Ok(());
    }

    if !args.yes && !confirm(ui, &release.version) {
        ui.line("Not updating.");
        return Ok(());
    }

    install(ui, release)
}

/// Downloads the release and replaces this executable with it.
fn install(ui: &Ui, release: database::version::NewRelease) -> Result<(), String> {
    if release.asset_url.is_none() {
        return Err(format!(
            "The release has no binary for this platform: {}",
            release.url
        ));
    }

    let state = new_state();
    let (sender, receiver) = channel();
    // The worker owns the download and the replacement; this thread only reads
    // the state it publishes, which is why it keeps its own handle.
    let for_worker = Arc::clone(&state);
    std::thread::spawn(move || selfupdate::run(receiver, for_worker));

    let _ = sender.send(UpdaterCommand::Install(release));
    let mut seen = UpdateStage::Idle;
    loop {
        let stage = current(&state);
        // `publish` only reports a change, so the same stage is not printed
        // again on every tick — the download alone would otherwise redraw the
        // same line a hundred times a second.
        if stage != seen {
            ui.clear_progress();
            ui.line(&stage_line(ui, &stage));
            seen = stage.clone();
        }
        match &stage {
            UpdateStage::Installed { .. } => {
                // The worker exits the process a moment later; without a restart
                // the download would sit in a temp file forever.
                ui.clear_progress();
                let _ = sender.send(UpdaterCommand::Restart);
                return Ok(());
            }
            UpdateStage::Failed { error, .. } => return Err(error.clone()),
            UpdateStage::RestartFailed { error, .. } => return Err(error.clone()),
            _ => std::thread::sleep(POLL),
        }
    }
}

/// One line for one stage.
fn stage_line(ui: &Ui, stage: &UpdateStage) -> String {
    match stage {
        UpdateStage::Downloading {
            version,
            done,
            total,
        } => format!("Downloading {version} — {}", format_progress(*done, *total)),
        UpdateStage::Installing { version } => format!("Installing {version}…"),
        UpdateStage::Installed { version } => {
            format!("{} Restarting…", ui.good(&format!("Installed {version}.")))
        }
        UpdateStage::Failed { version, error } => {
            format!(
                "{} {error}",
                ui.bad(&format!("Could not install {version}:"))
            )
        }
        UpdateStage::RestartFailed { version, error } => {
            format!(
                "{} {error}",
                ui.bad(&format!("Installed {version} but could not restart:"))
            )
        }
        _ => String::new(),
    }
}

/// Asks before replacing the executable.
///
/// Skipped when stdin is not a terminal, like the confirmation before a
/// cleaning run: a pipeline cannot answer, and a `--yes` flag is the way a
/// script says yes.
fn confirm(ui: &Ui, version: &str) -> bool {
    use std::io::{IsTerminal, Read, Write};

    if !std::io::stdin().is_terminal() {
        return true;
    }
    ui.line(&format!("Replace this executable with {version}?"));
    print!("Continue? [y/N] ");
    let _ = std::io::stdout().flush();
    let mut answer = [0u8; 8];
    let Ok(read) = std::io::stdin().read(&mut answer) else {
        return false;
    };
    let answer = String::from_utf8_lossy(&answer[..read])
        .trim()
        .to_lowercase();
    answer == "y" || answer == "yes"
}
