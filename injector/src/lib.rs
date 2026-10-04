//! Injectable Cross Cleaner.
//!
//! The cleaner as a library instead of a program: load it into a running program
//! (Windows: `cross_cleaner_injector.dll`; Linux:
//! `libcross_cleaner_injector.so`) and it watches the keyboard for a hotkey —
//! `Delete` by default — and opens its window inside that program.
//!
//! # What happens when the library is loaded
//!
//! 1. A tiny bootstrap thread is started from `DllMain` (Windows) or from a
//!    `.init_array` constructor (Linux). Nothing else happens on the loading
//!    thread, which is what the loader lock requires.
//! 2. The bootstrap reads the [`Config`] from the `CROSS_CLEANER_*` environment
//!    variables, registers the built-in cleaners and starts the key watcher.
//! 3. The first hotkey press creates the window on a second thread, with its own
//!    winit event loop (`any_thread`) and its own tokio runtime. From then on the
//!    hotkey toggles the window.
//!
//! # Where the window appears
//!
//! By default it becomes a *child window* of the program's own window
//! ([`embed::Mode::Auto`]), so it looks and behaves like part of that program
//! instead of a second window on top of it: no second taskbar entry, it moves
//! with the program and it is hidden when the program is minimized.
//! `CROSS_CLEANER_WINDOW=window` keeps it an independent window, which is what a
//! fullscreen game wants.
//!
//! # Why the window lives in the host program
//!
//! A child process could not know when to appear, would need a taskbar entry of its
//! own and would have to be shut down separately. The window also keeps the host
//! program working, because neither watcher grabs the key: the key is only
//! observed, and the program it was loaded into keeps receiving it.
//!
//! # Safety of the host program
//!
//! * The window thread and the watcher thread are the only threads this library
//!   creates, and both are named.
//! * Nothing in the host program is patched or grabbed.
//! * Panics are caught at the thread boundary, so a bug in the cleaner cannot abort
//!   the program it was loaded into. That is also why the library is built with the
//!   `injected` profile (`panic = "unwind"`), unlike the rest of the workspace.
//! * The library must not be explicitly unloaded again (`FreeLibrary`, `dlclose`)
//!   while it is running: its threads would still be inside its code. Let the host
//!   program exit instead.

pub mod config;
pub mod embed;
pub mod hotkey;
pub mod log;
pub mod renderer;
pub mod session;
pub mod watcher;

use std::sync::OnceLock;

pub use config::{Action, Backend, Config};
pub use embed::Mode as WindowMode;
pub use hotkey::{Hotkey, Key};

/// The configuration of this process, set by the first call to [`start_with`].
static CONFIG: OnceLock<Config> = OnceLock::new();

/// Starts the cleaner session with an explicit configuration. Returns `false` when
/// a session is already running in this process.
pub fn start_with(config: Config) -> bool {
    if CONFIG.set(config).is_err() {
        return false;
    }
    start()
}

/// Starts the session with the configuration from the environment. Returns `false`
/// when a session is already running in this process.
pub fn start() -> bool {
    // Logging comes first, so that even a failure in the rest of the startup ends
    // up on disk instead of only on a stderr nobody reads.
    log::enable_from_env();
    if log::is_disabled() {
        log::info("CROSS_CLEANER_DISABLED is set: not starting the cleaner");
        return false;
    }

    let config = CONFIG.get().cloned().unwrap_or_else(Config::from_env);
    config.configure_logging();
    log::info(&format!(
        "loaded into pid {} ({}-bit), hotkey {}, action {}, renderer {}, window {}",
        std::process::id(),
        std::mem::size_of::<usize>() * 8,
        config.hotkey,
        config.action,
        config.backend,
        config.window,
    ));
    if cfg!(panic = "abort") {
        // With an aborting panic strategy the `catch_unwind` at the thread entry
        // points cannot work, so a bug here would take the host program down.
        log::warn(
            "this build aborts on panic: build the library with \
             `cargo build -p injector --profile injected` before loading it",
        );
    }

    if !session::start(config.clone()) {
        return false;
    }

    if config.show_on_start {
        // No watcher needed: the window is already what the user asked for. The
        // program that has the focus right now is this one, so that is the window
        // the cleaner will live in.
        embed::capture_host(0);
        session::show();
        return true;
    }

    let action = config.action;
    let hotkey = config.hotkey;
    let debounce = config.debounce;
    match watcher::spawn(
        hotkey,
        debounce,
        std::sync::Arc::new(move || session::activate(action)),
    ) {
        Ok(_) => log::info("hotkey watcher started"),
        Err(e) => {
            log::error(&format!("hotkey watcher not started: {e}"));
            // A window the host application cannot open at all would be useless, so
            // show it now and let the user close it again.
            session::show();
        }
    }
    true
}

// ============================================================================
// Windows entry point
// ============================================================================

#[cfg(windows)]
mod entry {
    use std::ffi::c_void;

    use windows::Win32::Foundation::HINSTANCE;
    use windows::Win32::System::Threading::{CreateThread, THREAD_CREATION_FLAGS};
    use windows::core::BOOL;

    use crate::{log, start};

    const DLL_PROCESS_DETACH: u32 = 0;
    const DLL_PROCESS_ATTACH: u32 = 1;

    /// The library entry point. Windows resolves it by name.
    ///
    /// SAFETY: called by the loader with the arguments of `DllMain`. It has to
    /// return quickly and must not panic, and it must not call anything that needs
    /// the loader lock, which is why the actual work is handed to a thread that has
    /// none of the restrictions of this function.
    #[unsafe(no_mangle)]
    #[allow(non_snake_case)]
    pub unsafe extern "system" fn DllMain(
        instance: HINSTANCE,
        reason: u32,
        _reserved: *mut c_void,
    ) -> BOOL {
        match reason {
            DLL_PROCESS_ATTACH => {
                // The keyboard hook has to name its own module.
                crate::watcher::set_module_handle(instance);

                // `DisableThreadLibraryCalls` is deliberately not called here: it
                // also stops the loader from initializing this library's thread-local
                // storage for the threads it creates afterwards, which the Rust
                // runtime needs.

                // Nothing here may fail loudly: a failure has to leave the host
                // program running. Reporting it means failing the load, which is the
                // loudest thing this function is allowed to do.
                if let Err(e) = unsafe {
                    CreateThread(None, 0, Some(bootstrap), None, THREAD_CREATION_FLAGS(0), None)
                } {
                    log::warn(&format!("CreateThread failed: {e}"));
                    return BOOL(0);
                }
            }
            DLL_PROCESS_DETACH => {
                // `reserved != NULL` means the program is exiting, so nothing of ours
                // has to be cleaned up. With `reserved == NULL` the library is being
                // unloaded while its threads are still inside it: stop reacting to
                // input, which is all that is left to do either way.
                crate::watcher::shutdown();
            }
            _ => {}
        }
        BOOL(1)
    }

    /// First code that runs outside the loader lock.
    unsafe extern "system" fn bootstrap(_parameter: *mut c_void) -> u32 {
        // A panic here would take the host program down with it.
        let _ = std::panic::catch_unwind(start);
        0
    }
}

// ============================================================================
// Linux entry point
// ============================================================================

/// Runs before the program's own constructors when the library is preloaded.
///
/// `#[used]` keeps the linker from dropping an unused `static`, and `.init_array`
/// is what the C runtime calls constructors from, so this is the hook every
/// `LD_PRELOAD` library uses.
#[cfg(target_os = "linux")]
#[used]
#[unsafe(link_section = ".init_array")]
static CONSTRUCTOR: extern "C" fn() = {
    extern "C" fn constructor() {
        // This still runs on the program's first thread inside `__libc_start_main`,
        // before the program itself, so it must not block: hand over to a thread
        // and let the program start.
        let _ = std::thread::Builder::new()
            .name("cross-clean-boot".to_string())
            .spawn(|| {
                let _ = std::panic::catch_unwind(start);
            });
    }
    constructor
};