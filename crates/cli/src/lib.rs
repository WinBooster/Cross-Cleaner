//! Command line frontend for Cross Cleaner.
//!
//! The third frontend, next to the window app (`desktop`) and the terminal app
//! (`tui`). It drives the exact same [`appcore::AppState`] the other two drive,
//! so a selection spelled on the command line resolves through the same
//! category/subcategory/program rules a checkbox ticked in the window app goes
//! through, and a run it starts is the same run.
//!
//! What belongs here is only the translation: a selection written as flags, and
//! a report printed as lines. The cleaning job, the category model and the
//! result types all live in `appcore`.
//!
//! ```text
//! cli categories                       # what can be cleaned
//! cli programs --category Cache        # which programs a category covers
//! cli plan -c Cache -c Logs            # what a selection would touch
//! cli clean -c Cache -c Logs           # clean it
//! cli clean -p 'Chrome=Logs'           # one program, one of its categories
//! ```

pub mod args;
pub mod plan;
pub mod report;
pub mod run;
pub mod select;
pub mod term;

#[cfg(feature = "self-update")]
pub mod update;

pub use args::{Cli, Command, SelectionArgs};
pub use term::Ui;
