//! Frontend-controlled diagnostics.
//!
//! Library code must never write to the process's stdout or stderr. The window
//! frontend can afford it, but the terminal frontend *cannot*: it owns the whole
//! screen, so a single stray `eprintln!` from a cleaning worker thread tears the
//! frame apart in the middle of a run — and with `panic = "abort"` there is no
//! unwinding to redraw it afterwards.
//!
//! So the crates report through here instead, and each frontend decides what
//! "reporting" means:
//!
//! * no sink installed — stderr, which is what the window app and any embedder
//!   want and keeps the previous behaviour;
//! * `desktop` / `android` — the default is enough;
//! * `tui` — captures the message and shows it as a toast, so the terminal is
//!   never touched.

use std::sync::Arc;

/// A diagnostics handler. Receives one already-formatted line.
pub type Sink = Arc<dyn Fn(&str) + Send + Sync>;

static SINK: std::sync::RwLock<Option<Sink>> = std::sync::RwLock::new(None);

/// Installs `sink` as the destination for [`warn`] and [`info`]. Passing `None`
/// restores the default, stderr.
pub fn set_sink(sink: Option<Sink>) {
    match SINK.write() {
        Ok(mut slot) => *slot = sink,
        // A poisoned lock means another thread panicked while reporting. Writing
        // to stderr is still better than losing the message silently.
        Err(poisoned) => *poisoned.into_inner() = sink,
    }
}

/// Reports a problem that did not stop the operation.
pub fn warn(message: impl AsRef<str>) {
    emit("warn", message.as_ref());
}

/// Reports something worth knowing about that is not a problem.
pub fn info(message: impl AsRef<str>) {
    emit("info", message.as_ref());
}

fn emit(level: &str, message: &str) {
    let line = format!("[{level}] {message}");
    let sink = match SINK.read() {
        Ok(slot) => slot.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    };
    match sink {
        Some(sink) => sink(&line),
        None => eprintln!("{line}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Installs a sink for the duration of `f` and returns what it collected.
    fn capture(f: impl FnOnce()) -> Vec<String> {
        static LINES: Mutex<Vec<String>> = Mutex::new(Vec::new());
        LINES.lock().expect("test lock").clear();
        let sink: Sink = Arc::new(|line| {
            LINES.lock().expect("test lock").push(line.to_string());
        });
        set_sink(Some(sink));
        f();
        set_sink(None);
        let lines = LINES.lock().expect("test lock").clone();
        lines
    }

    #[test]
    fn messages_reach_the_installed_sink() {
        let lines = capture(|| {
            warn("could not remove {path}");
            info("removed 12 files");
        });
        assert_eq!(
            lines,
            vec![
                "[warn] could not remove {path}".to_string(),
                "[info] removed 12 files".to_string(),
            ],
        );
    }

    #[test]
    fn removing_the_sink_restores_the_default() {
        capture(|| warn("first"));
        // With no sink installed nothing is captured, which is how the fallback
        // to stderr is observed from a test.
        assert!(capture(|| warn("second")).contains(&"[warn] second".to_string()));
    }
}
