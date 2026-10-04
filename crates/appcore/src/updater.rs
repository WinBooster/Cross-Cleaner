// ============================================================================
// Automatic update: shared state
// ============================================================================
// The UI never touches the executable itself. The platform front end owns a
// worker thread that downloads the release and swaps the running binary with
// `self-replace` (`desktop`), and publishes what it is doing through
// [`UpdateState`]. The UI only renders that state and sends
// [`UpdaterCommand`]s back.
//
// Keeping the protocol here (instead of in `desktop`) means neither frontend
// depends on `self-replace`: when no worker is registered the UI simply keeps
// the old "new version available" banner and points at the release page.

use std::sync::{Arc, Mutex};

use database::version::NewRelease;

/// What the updater worker is currently doing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum UpdateStage {
    /// No update in progress.
    #[default]
    Idle,
    /// Streaming `version` to disk: `done` of `total` bytes. `total` stays
    /// `None` until the length is known (or never, for a chunked response).
    Downloading {
        version: String,
        done: u64,
        total: Option<u64>,
    },
    /// Download finished, the running executable is being replaced.
    Installing { version: String },
    /// `version` is installed; the app has to be restarted to run it.
    Installed { version: String },
    /// `version` is installed, but the app could not be relaunched
    /// automatically. The user has to start it again by hand; `error` says
    /// why the automatic restart did not work.
    RestartFailed { version: String, error: String },
    /// The update was aborted; `error` is meant to be shown to the user.
    Failed { version: String, error: String },
}

impl UpdateStage {
    /// Version this stage belongs to, `None` while idle.
    pub fn version(&self) -> Option<&str> {
        match self {
            UpdateStage::Idle => None,
            UpdateStage::Downloading { version, .. }
            | UpdateStage::Installing { version }
            | UpdateStage::Installed { version }
            | UpdateStage::RestartFailed { version, .. }
            | UpdateStage::Failed { version, .. } => Some(version),
        }
    }

    /// True when the new version is on disk, so all that is left is a restart.
    pub fn is_installed(&self) -> bool {
        matches!(
            self,
            UpdateStage::Installed { .. } | UpdateStage::RestartFailed { .. }
        )
    }

    /// True while the worker still owns the update, so the user cannot
    /// cancel or restart in the middle of a download or an install.
    pub fn is_running(&self) -> bool {
        matches!(
            self,
            UpdateStage::Downloading { .. } | UpdateStage::Installing { .. }
        )
    }
}

/// Progress of the automatic update, shared by the worker thread and the UI.
pub type UpdateState = Arc<Mutex<UpdateStage>>;

/// Creates the slot shared between the updater worker and the UI.
pub fn new_state() -> UpdateState {
    Arc::new(Mutex::new(UpdateStage::Idle))
}

/// Publishes `stage` and reports whether it changed, so the caller can repaint
/// on transitions instead of on every progress tick.
pub fn publish(state: &UpdateState, stage: UpdateStage) -> bool {
    let Ok(mut slot) = state.lock() else {
        // A poisoned lock means the worker panicked mid-update; there is
        // nothing left to report progress through.
        return false;
    };
    if *slot == stage {
        return false;
    }
    *slot = stage;
    true
}

/// Reads the current stage, falling back to [`UpdateStage::Idle`] when the
/// worker panicked while holding the lock.
pub fn current(state: &UpdateState) -> UpdateStage {
    state.lock().map(|slot| slot.clone()).unwrap_or_default()
}

/// Command sent from the UI to the updater worker.
pub enum UpdaterCommand {
    /// Download `release` and replace the running executable with it.
    Install(NewRelease),
    /// Start a second copy of the app and quit, so the freshly installed
    /// version is the one that keeps running.
    Restart,
}

/// `"3.4 MB / 8.1 MB (42%)"`, or just the downloaded size while the total
/// length is still unknown.
pub fn format_progress(done: u64, total: Option<u64>) -> String {
    use database::utils::get_file_size_string;

    match total.filter(|total| *total > 0) {
        Some(total) => format!(
            "{} / {} ({}%)",
            get_file_size_string(done),
            get_file_size_string(total),
            (done.saturating_mul(100) / total).min(100),
        ),
        None => get_file_size_string(done),
    }
}

/// Fill ratio of the progress bar: the real fraction when the total length is
/// known, otherwise a bar that breathes back and forth (indeterminate).
pub fn progress_fraction(done: u64, total: Option<u64>, time: f64) -> f32 {
    match total.filter(|total| *total > 0) {
        Some(total) => (done as f32 / total as f32).clamp(0.0, 1.0),
        // 1.2 rad/s is slow enough to read as "working" rather than "busy".
        None => (0.5 + 0.5 * (time as f32 * 1.2 * std::f32::consts::TAU).sin()).clamp(0.0, 1.0),
    }
}

/// First line of the update notification for `stage`.
pub fn stage_heading(stage: &UpdateStage) -> String {
    let version = stage.version().unwrap_or("?");
    match stage {
        UpdateStage::Idle => String::new(),
        UpdateStage::Downloading { .. } => format!("Updating to v{version}"),
        UpdateStage::Installing { .. } => format!("Installing v{version}"),
        UpdateStage::Installed { .. } | UpdateStage::RestartFailed { .. } => {
            format!("v{version} installed")
        }
        UpdateStage::Failed { .. } => format!("Update to v{version} failed"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_starts_idle() {
        let state = new_state();
        assert_eq!(current(&state), UpdateStage::Idle);
        assert_eq!(current(&state).version(), None);
        assert!(!current(&state).is_running());
    }

    #[test]
    fn test_publish_reports_changes_only() {
        let state = new_state();
        let stage = UpdateStage::Downloading {
            version: "2.0.5".to_string(),
            done: 0,
            total: Some(100),
        };
        assert!(publish(&state, stage.clone()));
        // A repeated progress tick is not a change the UI has to react to.
        assert!(!publish(&state, stage));
        assert!(publish(
            &state,
            UpdateStage::Downloading {
                version: "2.0.5".to_string(),
                done: 100,
                total: Some(100),
            }
        ));
        assert_eq!(
            current(&state),
            UpdateStage::Downloading {
                version: "2.0.5".to_string(),
                done: 100,
                total: Some(100),
            }
        );
    }

    #[test]
    fn test_stage_helpers() {
        let downloading = UpdateStage::Downloading {
            version: "2.0.5".to_string(),
            done: 5,
            total: None,
        };
        assert!(downloading.is_running());
        assert!(!downloading.is_installed());
        assert_eq!(downloading.version(), Some("2.0.5"));

        let installing = UpdateStage::Installing {
            version: "2.0.5".to_string(),
        };
        assert!(installing.is_running());
        assert!(!installing.is_installed());

        // Once installed (or failed) the user is in charge again.
        let installed = UpdateStage::Installed {
            version: "2.0.5".to_string(),
        };
        assert!(!installed.is_running());
        assert!(installed.is_installed());
        assert_eq!(installed.version(), Some("2.0.5"));

        // A failed restart still leaves the update installed, so it must not
        // be retried — only relaunched.
        let restart_failed = UpdateStage::RestartFailed {
            version: "2.0.5".to_string(),
            error: "elevation required".to_string(),
        };
        assert!(!restart_failed.is_running());
        assert!(restart_failed.is_installed());

        let failed = UpdateStage::Failed {
            version: "2.0.5".to_string(),
            error: "boom".to_string(),
        };
        assert!(!failed.is_running());
        assert!(!failed.is_installed());
    }

    #[test]
    fn test_restart_failed_keeps_update_installed() {
        // A failed relaunch must not offer "Retry" as a download: the new
        // version is already on disk, so only starting it again makes sense.
        assert!(
            UpdateStage::RestartFailed {
                version: "2.0.5".to_string(),
                error: "elevation declined".to_string(),
            }
            .is_installed()
        );
    }

    #[test]
    fn test_format_progress() {
        assert_eq!(format_progress(0, None), "0 B");
        assert_eq!(format_progress(1024, Some(2048)), "1.0 KB / 2.0 KB (50%)");
        assert_eq!(format_progress(1024, Some(0)), "1.0 KB");
        // Never reports more than 100% if more bytes arrive than announced.
        assert_eq!(format_progress(4096, Some(1024)), "4.0 KB / 1.0 KB (100%)");
    }

    #[test]
    fn test_progress_fraction() {
        assert_eq!(progress_fraction(50, Some(100), 0.0), 0.5);
        assert_eq!(progress_fraction(500, Some(100), 0.0), 1.0);
        // Unknown length: an animation that always stays inside the bar.
        for step in 0..64 {
            let fraction = progress_fraction(0, None, f64::from(step) / 8.0);
            assert!((0.0..=1.0).contains(&fraction), "{fraction}");
        }
    }

    #[test]
    fn test_stage_heading() {
        assert_eq!(stage_heading(&UpdateStage::Idle), "");
        assert_eq!(
            stage_heading(&UpdateStage::Downloading {
                version: "2.1.0".to_string(),
                done: 0,
                total: None,
            }),
            "Updating to v2.1.0"
        );
        assert_eq!(
            stage_heading(&UpdateStage::Installed {
                version: "2.1.0".to_string(),
            }),
            "v2.1.0 installed"
        );
        assert_eq!(
            stage_heading(&UpdateStage::Failed {
                version: "2.1.0".to_string(),
                error: "boom".to_string(),
            }),
            "Update to v2.1.0 failed"
        );
    }
}
