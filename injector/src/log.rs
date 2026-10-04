//! Minimal logging.
//!
//! A loaded library usually has no console to print to, so everything goes to
//! stderr *and*, when `CROSS_CLEANER_LOG` names a file, to that file. The file
//! handle is opened once by [`enable`] and never closed: the library can be
//! unloaded while a cleaning run is still reporting, and closing on the way out
//! is not worth the bookkeeping.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;
use std::sync::OnceLock;

/// File the log is appended to, once [`enable`] ran.
fn log_file() -> &'static Mutex<Option<std::fs::File>> {
    static FILE: OnceLock<Mutex<Option<std::fs::File>>> = OnceLock::new();
    FILE.get_or_init(|| Mutex::new(None))
}

/// Starts appending log lines to the file named by `CROSS_CLEANER_LOG`.
///
/// Called before anything else runs, so that even a failure further along the
/// startup ends up on disk. A file that cannot be opened only disables the file
/// sink: stderr keeps working.
pub fn enable_from_env() {
    let path = std::env::var("CROSS_CLEANER_LOG").unwrap_or_default();
    if !path.is_empty() {
        enable(Path::new(&path));
    }
}

/// Whether the library should do nothing at all.
///
/// `CROSS_CLEANER_DISABLED` loads the library without starting anything, which
/// is the escape hatch for a program that cannot host the window.
pub fn is_disabled() -> bool {
    matches!(
        std::env::var("CROSS_CLEANER_DISABLED")
            .unwrap_or_default()
            .trim()
            .to_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
    )
}

/// Starts appending log lines to `path`. A file that cannot be opened only
/// disables the file sink: stderr keeps working.
pub fn enable(path: &Path) {
    match OpenOptions::new().create(true).append(true).open(path) {
        Ok(file) => {
            if let Ok(mut slot) = log_file().lock() {
                *slot = Some(file);
            }
        }
        Err(e) => {
            eprintln!("[injector] cannot open log file {}: {e}", path.display());
        }
    }
}

/// Writes one line to stderr and, if enabled, to the log file.
fn write_line(level: &str, message: &str) {
    let line = format!("[injector] {level}: {message}\n");
    let _ = std::io::stderr().write_all(line.as_bytes());
    if let Ok(mut slot) = log_file().lock()
        && let Some(file) = slot.as_mut()
    {
        let _ = file.write_all(line.as_bytes());
        let _ = file.flush();
    }
}

pub fn info(message: &str) {
    write_line("info", message);
}

pub fn warn(message: &str) {
    write_line("warn", message);
}

pub fn error(message: &str) {
    write_line("error", message);
}