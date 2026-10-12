use super::*;

/// Presses a key the way a Russian layout would produce it, as `crossterm`
/// would report it: the character, with the layout's case.
fn press_cyrillic(app: &mut TuiApp, c: char) {
    app.on_event(Event::Key(KeyEvent::new(
        KeyCode::Char(c),
        KeyModifiers::NONE,
    )));
}

#[test]
fn bindings_work_on_a_cyrillic_layout() {
    let mut app = sample_app();

    // Space is layout-independent, so it selects a category as usual.
    press(&mut app, KeyCode::Char(' '));
    assert!(app.state.has_selection());

    // `т` sits where `n` does, so it must advance to the program page.
    press_cyrillic(&mut app, 'т');
    assert_eq!(
        app.state.current_page,
        Page::ProgramSelection,
        "`n` must work as Cyrillic `т`",
    );

    // `ы` sits where `s` does: settings — bound on the main page, so step back
    // there first.
    press(&mut app, KeyCode::Esc);
    press_cyrillic(&mut app, 'ы');
    assert_eq!(app.state.current_page, Page::Settings);

    // `о` sits where `j` does: move down in the settings list.
    press_cyrillic(&mut app, 'о');
    assert_eq!(app.settings_cursor, 1);

    // `к` sits where `r` does: reset the selected volume to full.
    press_cyrillic(&mut app, 'у'); // where `e` is — nothing bound
    assert_eq!(app.settings_cursor, 1, "an unbound key must not move");
    press_cyrillic(&mut app, 'к'); // `r`
    assert!(app.toast.is_some(), "`r` must act as the reset key");
}

#[test]
fn case_sensitive_bindings_stay_distinct_on_a_cyrillic_layout() {
    let mut app = sample_app();
    press_cyrillic(&mut app, 'ф'); // `a` — free on the main page
    assert_eq!(app.state.current_page, Page::Main);

    // `ы` is where `s` is: settings. Uppercase `Ы` is `S`, which on the
    // program page starts cleaning — so the shift has to survive.
    press_cyrillic(&mut app, 'ы');
    assert_eq!(app.state.current_page, Page::Settings);
    press_cyrillic(&mut app, 'ф'); // `a` — nothing bound, stays put
    assert_eq!(app.state.current_page, Page::Settings);
}

#[test]
fn a_cyrillic_letter_does_not_match_the_letter_it_looks_like() {
    // `с` looks like `c` but sits on the physical `c` key, so it must
    // resolve to `c` and not to the `s` binding.
    let mut app = sample_app();
    press_cyrillic(&mut app, 'с');
    assert_eq!(app.state.current_page, Page::Main, "`с` is `c`, not `s`");
}

#[test]
fn the_search_key_is_reachable_on_a_cyrillic_layout() {
    let mut app = sample_app();
    press(&mut app, KeyCode::Char(' '));
    // `т` is where `n` is: advance to the program page.
    press_cyrillic(&mut app, 'т');
    assert_eq!(app.state.current_page, Page::ProgramSelection);

    // The key labelled `/` produces `.` in Russian; either opens search.
    press_cyrillic(&mut app, '.');
    assert_eq!(app.input, InputMode::Editing, "`/` must be reachable");
    press_cyrillic(&mut app, ',');
    assert_eq!(app.input, InputMode::Editing, "`,` must still open it too");
}

#[test]
fn the_search_key_also_still_works_on_an_english_layout() {
    let mut app = sample_app();
    press(&mut app, KeyCode::Char(' '));
    press(&mut app, KeyCode::Char('n'));
    press(&mut app, KeyCode::Char('/'));
    assert_eq!(app.input, InputMode::Editing);
}

#[test]
fn typing_in_the_search_field_keeps_cyrillic_verbatim() {
    let mut app = sample_app();
    press(&mut app, KeyCode::Char(' '));
    press(&mut app, KeyCode::Char('n'));
    press(&mut app, KeyCode::Char('/'));
    assert_eq!(app.input, InputMode::Editing);

    // Program names come from the database and are not necessarily ASCII.
    for c in "Почта".chars() {
        press_cyrillic(&mut app, c);
    }
    assert_eq!(
        app.state.search_query_visible, "Почта",
        "the query must not be transliterated",
    );
    assert_eq!(app.state.search_query, "почта", "matching lowercases it");
}

#[test]
fn ctrl_c_still_quits_on_a_cyrillic_layout() {
    let mut app = sample_app();
    app.on_event(Event::Key(KeyEvent::new(
        KeyCode::Char('с'), // `с` is what the `c` key reports in Russian
        KeyModifiers::CONTROL,
    )));
    assert!(app.should_quit, "Ctrl+C must not be remapped");
}
