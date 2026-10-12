//! The settings page: the four sound volumes.
//!
//! [`SETTINGS`] lists the volumes in the order the window frontend lists them,
//! and [`volume_of`] / [`volume_slot`] are the only two places that know which
//! config field a slot is, so the page and the keys cannot drift apart.

use appcore::{AppConfig, config};
use crossterm::event::{KeyCode, KeyEvent};

use super::TuiApp;
use super::overlay::Toast;

/// The four volumes, in the order the window frontend lists them.
pub(super) const SETTINGS: [&str; 4] = ["Popup sound", "Click sound", "Check sound", "Done sound"];

impl TuiApp {
    pub(super) fn on_key_settings(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.settings_cursor = (self.settings_cursor + SETTINGS.len() - 1) % SETTINGS.len();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.settings_cursor = (self.settings_cursor + 1) % SETTINGS.len();
            }
            KeyCode::Left | KeyCode::Char('h') => self.adjust_volume(-0.1),
            KeyCode::Right | KeyCode::Char('l') => self.adjust_volume(0.1),
            // `r` resets to full, which is the only way back from silence.
            KeyCode::Char('r') | KeyCode::Char('R') => self.adjust_volume(0.0),
            _ => {}
        }
    }

    /// Steps the selected volume by `delta`, or resets it to full when `delta`
    /// is zero.
    fn adjust_volume(&mut self, delta: f32) {
        let slot = self.settings_cursor;
        let value = if delta == 0.0 {
            1.0
        } else {
            (volume_of(&config::get(), slot) + delta).clamp(0.0, 1.0)
        };
        self.set_volume(slot, value);
    }

    /// Sets one of the four volumes, moves the cursor onto it and reports the
    /// new level.
    ///
    /// Both the arrow keys and a click on the bar end up here, so the two ways
    /// of changing a level store it the same way and report it the same way.
    pub(super) fn set_volume(&mut self, slot: usize, value: f32) {
        let mut cfg = config::get();
        *volume_slot(&mut cfg, slot) = value.clamp(0.0, 1.0);
        let value = volume_of(&cfg, slot);
        config::update(|c| *c = cfg);
        self.settings_cursor = slot;
        self.toast = Some(Toast::info(format!(
            "{}: {:.0}%",
            SETTINGS[slot],
            value * 100.0
        )));
    }
}

/// The volume stored in slot `slot` of the shared config.
///
/// [`SETTINGS`] and the four fields are kept in step by this and
/// [`volume_slot`]; the settings page reads the same order.
pub(super) fn volume_of(cfg: &AppConfig, slot: usize) -> f32 {
    match slot {
        0 => cfg.sound_volume,
        1 => cfg.click_volume,
        2 => cfg.check_volume,
        _ => cfg.done_volume,
    }
}

/// [`volume_of`] as a mutable reference, for writing a new level.
fn volume_slot(cfg: &mut AppConfig, slot: usize) -> &mut f32 {
    match slot {
        0 => &mut cfg.sound_volume,
        1 => &mut cfg.click_volume,
        2 => &mut cfg.check_volume,
        _ => &mut cfg.done_volume,
    }
}
