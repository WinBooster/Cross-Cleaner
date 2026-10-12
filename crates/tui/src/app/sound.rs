//! The clip an action plays.
//!
//! The window frontend plays five different clips; a keyboard has no checkbox to
//! fall back on, so here the sound is the only cue that tells a check from an
//! uncheck. The mapping therefore lives in one place instead of at every call
//! site, and it is named at the call site so the tests can assert on it.

use appcore::Toggle;
use appcore::sounds;

/// Plays the sound for a selection that was just toggled.
///
/// Check and uncheck are separate clips in the window frontend, and a keyboard
/// has no checkbox to fall back on — this is the only cue that tells the two
/// apart, so the mapping is kept in one place instead of at every call site.
pub(super) fn play_toggle(toggle: Toggle) {
    match toggle {
        Toggle::On => sfx("check", sounds::check),
        Toggle::Off => sfx("uncheck", sounds::uncheck),
    }
}

/// Plays one clip and names it, which is what lets the tests assert the sound
/// map.
///
/// Playing needs an audio device, so a test cannot hear anything; it can only
/// check that an action *reached* a clip. Without a name at the call site the
/// map is invisible to the suite — which is how this app came to ship without a
/// single sound while the window one had five.
pub(super) fn sfx(clip: &'static str, play: impl FnOnce()) {
    #[cfg(test)]
    PLAYED.with(|played| played.borrow_mut().push(clip));
    #[cfg(not(test))]
    let _ = clip;
    play();
}

#[cfg(test)]
thread_local! {
    static PLAYED: std::cell::RefCell<Vec<&'static str>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Empties and returns the clips played on this thread so far.
#[cfg(test)]
pub(super) fn take_played() -> Vec<&'static str> {
    PLAYED.with(|played| played.take())
}
