//! Library diagnostics captured while this app owns the terminal.
//!
//! Cleaning runs on worker threads and reports failures through
//! [`database::diag`]. Those must never reach stderr: the alternate screen would
//! be torn in the middle of the run, and with `panic = "abort"` there is no
//! redraw afterwards. The sink installed here collects them instead, and
//! [`TuiApp::tick`](super::TuiApp::tick) turns whatever arrived into a toast.

use std::collections::VecDeque;
use std::time::Instant;

/// Diagnostics captured while this process owns the terminal.
///
/// Cleaning runs on worker threads and reports failures through
/// [`database::diag`]. Those must never reach stderr: the alternate screen would
/// be torn in the middle of the run, and with `panic = "abort"` there is no
/// redraw afterwards. They are collected here and surfaced as a toast instead.
static DIAGNOSTICS: std::sync::Mutex<VecDeque<(String, Instant)>> =
    std::sync::Mutex::new(VecDeque::new());

/// How many diagnostics to keep. Bounded because nothing drains this buffer
/// except [`TuiApp::tick`], and a long run must not grow it without limit.
pub(super) const DIAGNOSTIC_HISTORY: usize = 32;

/// Routes library diagnostics into [`DIAGNOSTICS`].
///
/// Without this every `warn` from a cleaner would be written straight to the
/// terminal this app is drawing into.
pub fn install_diagnostic_sink() {
    database::diag::set_sink(Some(std::sync::Arc::new(push_diagnostic)));
}

/// Removes the sink again, restoring the stderr default.
pub fn remove_diagnostic_sink() {
    database::diag::set_sink(None);
}

/// Takes the diagnostics collected since the last call.
pub(super) fn take_diagnostics() -> Vec<String> {
    DIAGNOSTICS
        .lock()
        .map(|mut queue| queue.drain(..).map(|(line, _)| line).collect())
        .unwrap_or_default()
}

/// A diagnostic arrived for the buffer, in arrival order.
fn push_diagnostic(line: &str) {
    let Ok(mut queue) = DIAGNOSTICS.lock() else {
        return;
    };
    if queue.len() >= DIAGNOSTIC_HISTORY {
        queue.pop_front();
    }
    queue.push_back((line.to_string(), Instant::now()));
}
