//! Windows resources for the terminal frontend.
//!
//! Same icon, version block and elevation requirement as the window app, so the
//! two look and behave identically in Explorer and in the UAC prompt. The
//! details live in the `winicon` crate; only the frontend-specific switches are
//! decided here.

fn main() {
    winicon::apply(winicon::Options {
        // Same reasoning as `desktop`: the cleaner writes to system-wide
        // locations, so an unelevated run would silently under-clean.
        require_admin: true,
        // Deliberately off. `NO_CONSOLE` switches the binary to the GUI
        // subsystem, which would detach it from the terminal it was started
        // from — this app exists to draw into that terminal.
        no_console: false,
    });
}
