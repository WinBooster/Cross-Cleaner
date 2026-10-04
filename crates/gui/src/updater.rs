//! Automatic update protocol, shared with the terminal frontend.
//!
//! See [`appcore::updater`] for the state machine and the worker contract; this
//! module only re-exports it so `gui::updater::*` keeps working.

pub use appcore::updater::{
    UpdateStage, UpdateState, UpdaterCommand, current, format_progress, new_state,
    progress_fraction, publish, stage_heading,
};
