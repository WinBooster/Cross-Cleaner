//! Windows resources for the command line frontend.
//!
//! Same icon and version block as the window and terminal frontends. What
//! differs is decided here; the details live in the `winicon` crate.

fn main() {
    winicon::apply(winicon::Options {
        // Same reasoning as `desktop`: the cleaner writes to system-wide
        // locations, so an unelevated run would silently under-clean.
        require_admin: true,
        // Deliberately off. `NO_CONSOLE` switches the binary to the GUI
        // subsystem, which detaches it from the terminal it was started
        // from — this app exists to write to that terminal.
        no_console: false,
    });
}
