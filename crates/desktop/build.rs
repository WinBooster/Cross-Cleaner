//! Windows resources for the window frontend.
//!
//! The details live in the `winicon` crate so the terminal frontend can reuse
//! them; only the two frontend-specific switches are decided here.

fn main() {
    winicon::apply(winicon::Options {
        // The crate lives in `crates/`, so the repo-root assets are two levels up.
        icon: "..\\..\\assets\\icon.ico",
        // Cleaning writes to system-wide locations, so a release build must be
        // elevated — same as the terminal frontend.
        require_admin: true,
        // A window app must not leave a console window behind it.
        no_console: true,
    });
}