//! Layout-independent key handling.
//!
//! A terminal reports the character the *active keyboard layout* produces, not
//! the key the user pressed. With an English layout a binding on `n` works; on a
//! Russian one the same physical key produces `т`, and every letter binding
//! silently stops working.
//!
//! [`normalize`] undoes that by mapping the produced character back to the key
//! it would be on an English layout — positionally, exactly like the ЙЦУКЕН
//! table does. So the user always presses the key they see next to the hint.
//!
//! Two things are deliberately *not* touched:
//!
//! * Keys that have no character (`Esc`, arrows, `F1`, …) — layout-independent
//!   already.
//! * Text input: while the user is typing a search query their characters must
//!   survive verbatim, otherwise Cyrillic program names would be unsearchable.
//!
//! Only letters are remapped. Punctuation is ambiguous — the key labelled `/` on
//! an English layout produces `.` in Russian, but `.` is also a perfectly valid
//! English `.` — so a table entry could not tell the two apart and would
//! shadow the English key. The one binding that needs such a key accepts both
//! characters instead; see `KeyCode::Char('/')` in the program page.

use crossterm::event::KeyCode;

/// The ЙЦУКЕН letter table: each Cyrillic letter mapped to the key it occupies
/// on an English layout (`т` sits where `n` is, and so on).
const CYRILLIC: [(char, char); 26] = [
    ('ф', 'a'),
    ('и', 'b'),
    ('с', 'c'),
    ('в', 'd'),
    ('у', 'e'),
    ('а', 'f'),
    ('п', 'g'),
    ('р', 'h'),
    ('ш', 'i'),
    ('о', 'j'),
    ('л', 'k'),
    ('д', 'l'),
    ('ь', 'm'),
    ('т', 'n'),
    ('щ', 'o'),
    ('з', 'p'),
    ('й', 'q'),
    ('к', 'r'),
    ('ы', 's'),
    ('е', 't'),
    ('г', 'u'),
    ('м', 'v'),
    ('ц', 'w'),
    ('ч', 'x'),
    ('н', 'y'),
    ('я', 'z'),
];

/// Maps a key reported by the terminal onto the key an English layout would
/// have produced, so bindings work regardless of the active layout.
///
/// Case is preserved: `Ы` maps to `S`, not `s`, because the app distinguishes
/// `s` (settings) from `S` (start cleaning).
pub fn normalize(code: KeyCode) -> KeyCode {
    match code {
        KeyCode::Char(c) => KeyCode::Char(normalize_char(c)),
        other => other,
    }
}

fn normalize_char(c: char) -> char {
    let lower = c.to_lowercase().next().unwrap_or(c);
    let Some((_, latin)) = CYRILLIC.iter().find(|(from, _)| *from == lower) else {
        // Latin, a digit, or a symbol: already what the bindings expect.
        return c;
    };
    if c.is_uppercase() {
        latin.to_ascii_uppercase()
    } else {
        *latin
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latin_keys_are_untouched() {
        for c in ['a', 'z', 'N', 'S', '7', '?', '/', ' ', ',', '.', '[', '`'] {
            assert_eq!(
                normalize(KeyCode::Char(c)),
                KeyCode::Char(c),
                "{c:?} must survive",
            );
        }
    }

    #[test]
    fn punctuation_is_never_remapped() {
        // `.` is the character the `/` key produces on a Russian layout, but it
        // is also a valid English `.`. Remapping it would break the English one,
        // so the table stops at letters and the binding accepts both instead.
        for c in ['.', ',', ';', '\'', '[', ']', '`', '-', '=', ';', '7'] {
            assert_eq!(
                normalize(KeyCode::Char(c)),
                KeyCode::Char(c),
                "{c:?} must survive",
            );
        }
    }

    #[test]
    fn cyrillic_letters_map_onto_the_english_keys_they_sit_on() {
        // ЙЦУКЕН: `т` is where `n` is, so pressing the key labelled `n` in the
        // hints has to fire the `n` binding.
        let cases = [
            ('т', 'n'),
            ('ы', 's'),
            ('в', 'd'),
            ('ф', 'a'),
            ('у', 'e'),
            ('а', 'f'),
            ('п', 'g'),
            ('р', 'h'),
            ('о', 'j'),
            ('л', 'k'),
            ('д', 'l'),
            ('ь', 'm'),
            ('щ', 'o'),
            ('з', 'p'),
            ('й', 'q'),
            ('к', 'r'),
            ('е', 't'),
            ('г', 'u'),
            ('м', 'v'),
            ('ц', 'w'),
            ('ч', 'x'),
            ('н', 'y'),
            ('я', 'z'),
            ('с', 'c'),
            ('и', 'b'),
        ];
        for (cyr, latin) in cases {
            assert_eq!(
                normalize(KeyCode::Char(cyr)),
                KeyCode::Char(latin),
                "{cyr:?} should press {latin:?}",
            );
        }
    }

    #[test]
    fn case_is_preserved() {
        // `S` starts cleaning and `s` opens settings, so the shift must survive.
        assert_eq!(normalize(KeyCode::Char('Ы')), KeyCode::Char('S'));
        assert_eq!(normalize(KeyCode::Char('Т')), KeyCode::Char('N'));
        assert_eq!(normalize(KeyCode::Char('Н')), KeyCode::Char('Y'));
    }

    #[test]
    fn non_character_keys_pass_through() {
        for code in [
            KeyCode::Esc,
            KeyCode::Enter,
            KeyCode::Tab,
            KeyCode::BackTab,
            KeyCode::Backspace,
            KeyCode::Up,
            KeyCode::Down,
            KeyCode::Left,
            KeyCode::Right,
            KeyCode::Home,
            KeyCode::End,
            KeyCode::PageUp,
            KeyCode::PageDown,
            KeyCode::F(1),
        ] {
            assert_eq!(normalize(code), code, "{code:?} must survive");
        }
    }

    #[test]
    fn the_table_has_no_duplicate_targets() {
        // A collision would silently shadow a binding.
        let mut latin: Vec<char> = CYRILLIC.iter().map(|(_, to)| *to).collect();
        latin.sort_unstable();
        let count = latin.len();
        latin.dedup();
        assert_eq!(latin.len(), count, "duplicate target in {CYRILLIC:?}");
    }
}
