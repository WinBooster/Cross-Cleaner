//! UI sounds for the window frontend.
//!
//! The player itself lives in [`appcore::sounds`] because `tui` plays the same
//! five clips for the same events. This module is only a re-export, so the
//! existing `crate::sounds::…` call sites keep working and there is exactly one
//! definition of which clip goes with which event.
//!
//! (Previously `gui` owned the rodio sink and the compressed MP3s. Splitting
//! that left the two frontends free to pick different clips for "check", which
//! is exactly the kind of drift this crate exists to prevent.)

pub use appcore::sounds::{check, click, done, init, pop, uncheck};
