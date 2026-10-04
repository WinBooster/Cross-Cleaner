//! Global key watcher.
//!
//! The cleaner has to be reachable while *another* program has the focus, so the
//! key is observed globally instead of through the window's own input: Windows
//! uses a low-level keyboard hook, Linux reads the input devices directly.
//! Neither of them grabs the key, so the host program still receives every
//! keystroke it would have received without the library loaded.

use std::fmt;
use std::sync::Arc;
use std::thread::JoinHandle;

use crate::hotkey::Hotkey;

#[cfg(windows)]
mod win;
#[cfg(windows)]
pub use self::win::set_module_handle;
#[cfg(windows)]
use self::win as platform;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use self::linux as platform;

/// Why the watcher could not be started.
#[derive(Debug)]
pub enum WatcherError {
    /// No global watcher exists for this platform.
    Unsupported(&'static str),
    /// The platform refused to install the hook.
    Failed(String),
}

impl fmt::Display for WatcherError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(what) => {
                write!(f, "no global key watcher on this platform ({what})")
            }
            Self::Failed(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for WatcherError {}

/// Spawns a thread that calls `on_press` for every press of `hotkey`.
///
/// The returned thread runs for as long as the process does; it is never joined,
/// because the library lives exactly as long as the program it was loaded into.
pub fn spawn(
    hotkey: Hotkey,
    debounce: std::time::Duration,
    on_press: Arc<dyn Fn() + Send + Sync>,
) -> Result<JoinHandle<()>, WatcherError> {
    platform::spawn(hotkey, debounce, on_press)
}

/// Stops reacting to input.
///
/// Called when the library is being unloaded. The watcher cannot join its own
/// thread from there, so this only makes the thread stop touching anything else;
/// it is a safety net, not a proper shutdown.
pub fn shutdown() {
    platform::shutdown();
}

/// Placeholder for platforms without a global watcher (macOS, BSD): the window
/// can still be opened by a host that links the rlib and calls
/// [`crate::session::show`] itself, but there is no way to watch a key globally
/// from inside another program.
#[cfg(not(any(windows, target_os = "linux")))]
mod unsupported {
    use super::*;

    pub fn spawn(
        _hotkey: Hotkey,
        _debounce: std::time::Duration,
        _on_press: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<JoinHandle<()>, WatcherError> {
        Err(WatcherError::Unsupported(
            "global hotkeys are implemented for Windows and Linux only",
        ))
    }

    pub fn shutdown() {}
}

#[cfg(not(any(windows, target_os = "linux")))]
use self::unsupported as platform;