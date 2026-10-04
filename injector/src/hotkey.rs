//! Hotkey description: which key opens the window, and on which platforms it
//! maps to a key code.
//!
//! The library is platform independent on purpose: the same `Hotkey` is matched
//! against a Windows virtual-key code by the Win32 watcher and against an evdev
//! key code by the Linux one.

use std::str::FromStr;

/// A key the hotkey can be bound to, with the codes needed by the two supported
/// platforms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Backspace,
    Tab,
    Enter,
    Escape,
    Space,
    Delete,
    Insert,
    Home,
    End,
    PageUp,
    PageDown,
    PrintScreen,
    Pause,
    ScrollLock,
    NumLock,
    F(u8),
}

impl Key {
    /// Windows virtual-key code.
    pub fn windows_vk(self) -> u32 {
        match self {
            Self::Backspace => 0x08,
            Self::Tab => 0x09,
            Self::Enter => 0x0D,
            Self::Escape => 0x1B,
            Self::Space => 0x20,
            Self::PrintScreen => 0x2C,
            Self::Insert => 0x2D,
            Self::Delete => 0x2E,
            Self::Home => 0x24,
            Self::End => 0x23,
            Self::PageUp => 0x21,
            Self::PageDown => 0x22,
            Self::Pause => 0x13,
            Self::NumLock => 0x90,
            Self::ScrollLock => 0x91,
            // Function keys are a contiguous range in both code sets.
            Self::F(n) => 0x6F + n as u32,
        }
    }

    /// Linux evdev key code (`KEY_*` from `linux/input-event-codes.h`).
    pub fn linux_code(self) -> u16 {
        match self {
            Self::Backspace => 14,
            Self::Tab => 15,
            Self::Enter => 28,
            Self::Escape => 1,
            Self::Space => 57,
            Self::PrintScreen => 99,
            Self::Insert => 110,
            Self::Delete => 111,
            Self::Home => 102,
            Self::End => 107,
            Self::PageUp => 104,
            Self::PageDown => 109,
            Self::Pause => 119,
            Self::NumLock => 69,
            Self::ScrollLock => 70,
            // `KEY_F1` is 59 on evdev and also contiguous.
            Self::F(n) => 58 + n as u16,
        }
    }

    /// Canonical name, as accepted by [`Hotkey::parse`].
    pub fn name(self) -> String {
        match self {
            Self::F(n) => format!("f{n}"),
            other => format!("{other:?}").to_lowercase(),
        }
    }
}

/// A key plus the modifiers that have to be held down with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hotkey {
    pub key: Key,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

impl Default for Hotkey {
    fn default() -> Self {
        Self {
            key: Key::Delete,
            ctrl: false,
            alt: false,
            shift: false,
        }
    }
}

impl Hotkey {
    /// Parses `"delete"`, `"f5"`, `"ctrl+alt+delete"`, `"shift+f12"`, ...
    ///
    /// An unknown key is an error instead of being ignored: a hotkey that is
    /// silently not bound would look like the library does not work at all.
    pub fn parse(spec: &str) -> Result<Self, String> {
        let mut hotkey = Self::default();
        let mut key_seen = false;
        for part in spec.split(['+', '-']) {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            match part.to_lowercase().as_str() {
                "ctrl" | "control" => hotkey.ctrl = true,
                "alt" | "option" => hotkey.alt = true,
                "shift" => hotkey.shift = true,
                name => {
                    if key_seen {
                        return Err(format!("more than one key in hotkey {spec:?}"));
                    }
                    hotkey.key = parse_key(name)
                        .ok_or_else(|| format!("unknown key {name:?} in hotkey {spec:?}"))?;
                    key_seen = true;
                }
            }
        }
        if !key_seen {
            return Err(format!("hotkey {spec:?} has no key"));
        }
        Ok(hotkey)
    }
}

impl FromStr for Hotkey {
    type Err = String;

    fn from_str(spec: &str) -> Result<Self, Self::Err> {
        Self::parse(spec)
    }
}

impl std::fmt::Display for Hotkey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.ctrl {
            f.write_str("ctrl+")?;
        }
        if self.alt {
            f.write_str("alt+")?;
        }
        if self.shift {
            f.write_str("shift+")?;
        }
        f.write_str(&self.key.name())
    }
}

fn parse_key(name: &str) -> Option<Key> {
    let fixed = match name {
        "backspace" | "back" => Key::Backspace,
        "tab" => Key::Tab,
        "enter" | "return" => Key::Enter,
        "esc" | "escape" => Key::Escape,
        "space" => Key::Space,
        "delete" | "del" => Key::Delete,
        "insert" | "ins" => Key::Insert,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" | "pgup" => Key::PageUp,
        "pagedown" | "pgdn" => Key::PageDown,
        "printscreen" | "print" => Key::PrintScreen,
        "pause" | "break" => Key::Pause,
        "scrolllock" => Key::ScrollLock,
        "numlock" => Key::NumLock,
        _ => {
            let digits = name.strip_prefix('f')?;
            let number: u8 = digits.parse().ok()?;
            return match number {
                1..=24 => Some(Key::F(number)),
                _ => None,
            };
        }
    };
    Some(fixed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_hotkey_is_delete() {
        assert_eq!(Hotkey::default().key, Key::Delete);
        assert_eq!(Hotkey::default().to_string(), "delete");
    }

    #[test]
    fn parses_plain_key() {
        assert_eq!(Hotkey::parse("insert").unwrap().key, Key::Insert);
        assert_eq!(Hotkey::parse("F5").unwrap().key, Key::F(5));
        assert_eq!(Hotkey::parse("pgup").unwrap().key, Key::PageUp);
    }

    #[test]
    fn parses_modifiers() {
        let hotkey = Hotkey::parse("Ctrl+Alt+Shift+F12").unwrap();
        assert_eq!(
            hotkey,
            Hotkey {
                key: Key::F(12),
                ctrl: true,
                alt: true,
                shift: true,
            }
        );
        assert_eq!(hotkey.to_string(), "ctrl+alt+shift+f12");
    }

    #[test]
    fn rejects_nonsense() {
        assert!(Hotkey::parse("ctrl").is_err());
        assert!(Hotkey::parse("nope").is_err());
        assert!(Hotkey::parse("f0").is_err());
        assert!(Hotkey::parse("delete+insert").is_err());
    }

    #[test]
    fn key_codes_match_the_platforms() {
        // Windows VK codes.
        assert_eq!(Key::Delete.windows_vk(), 0x2E);
        assert_eq!(Key::F(1).windows_vk(), 0x70);
        assert_eq!(Key::F(12).windows_vk(), 0x7B);
        // Linux evdev codes.
        assert_eq!(Key::Delete.linux_code(), 111);
        assert_eq!(Key::F(1).linux_code(), 59);
        assert_eq!(Key::F(12).linux_code(), 70);
    }

    #[test]
    fn every_parsed_key_has_a_name() {
        for key in [
            Key::Backspace,
            Key::Tab,
            Key::Enter,
            Key::Escape,
            Key::Space,
            Key::Delete,
            Key::Insert,
            Key::Home,
            Key::End,
            Key::PageUp,
            Key::PageDown,
            Key::PrintScreen,
            Key::Pause,
            Key::ScrollLock,
            Key::NumLock,
            Key::F(1),
            Key::F(24),
        ] {
            let round_trip = Hotkey::parse(&key.name()).unwrap();
            assert_eq!(round_trip.key, key, "{key:?} did not round trip");
        }
    }
}