use super::*;

/// The mouse must reach the same state the keyboard does, because the hints
/// in the footer promise one set of actions and there is only one app.
#[test]
fn clicking_a_category_cell_selects_exactly_that_category() {
    let mut app = many_categories(6);
    assert!(!app.state.has_selection());

    // Second category: the grid is two columns wide, so this is the right
    // one, and it is mirrored — the checkbox trails the label there.
    click_on(&mut app, 100, 30, "Cat1");
    assert!(
        app.state.categories[1].is_checked(),
        "the clicked cell is the one that changed",
    );
    for (index, category) in app.state.categories.iter().enumerate() {
        assert_eq!(
            category.is_unchecked(),
            index != 1,
            "only the clicked category changed",
        );
    }
    // And the cursor followed the click, so the keyboard keeps acting on the
    // category the mouse just touched.
    assert_eq!(app.category_cursor, 1);
}

/// The `→ N` marker is a narrow target at the end of the cell. Everything
/// else in the cell — the label above all — must still tick, or the marker
/// target would swallow the whole cell and a click on a name would open an
/// overlay instead of selecting the category.
#[test]
fn clicking_a_label_ticks_even_though_the_cell_ends_in_a_marker() {
    let mut app = many_categories(6);
    for name in ["Cat0", "Cat2", "Cat4"] {
        click_on(&mut app, 100, 30, name);
        assert!(
            app.popup.is_none(),
            "{name}: clicking a label must not open the overlay",
        );
    }
    // Three distinct categories, one per left-column row.
    let ticked = app
        .state
        .categories
        .iter()
        .filter(|category| !category.is_unchecked())
        .count();
    assert_eq!(ticked, 3, "each click ticked its own left-column cell");
}

/// The right button is the terminal's context menu. On a checkbox its one
/// meaning is "show me the subcategories" — the same overlay `Enter` and the
/// `→ N` marker open — and it must leave the tick alone, or a right click
/// would select the category the user only asked to look inside.
#[test]
fn right_clicking_a_checkbox_opens_its_subcategories_without_ticking_it() {
    let mut app = sample_app();
    right_click_on(&mut app, 100, 30, "Cache");

    let popup = app
        .popup
        .as_ref()
        .expect("the right button opens the overlay");
    assert_eq!(popup.items.len(), 2, "Browser and Code");
    assert!(
        app.state.categories[0].is_unchecked(),
        "the right button must not also tick the category",
    );
}

/// The mirror exists to be read from the other edge, so the right button has
/// to resolve to the category the mirrored cell shows — not its neighbour on
/// the same row.
#[test]
fn right_clicking_the_mirrored_cell_opens_that_category() {
    let mut app = many_categories(6);
    right_click_on(&mut app, 100, 30, "Cat3");
    match app.popup.as_ref().map(|popup| popup.kind) {
        Some(PopupKind::Category(index)) => assert_eq!(
            index, 3,
            "the overlay belongs to the category that was clicked",
        ),
        other => panic!("expected Cat3's overlay, got {other:?}"),
    }
}

/// Anywhere in the cell opens the same menu, marker or not — the right button
/// is not restricted to the few columns the `→ N` glyph occupies.
#[test]
fn a_right_click_anywhere_in_the_cell_opens_the_same_menu() {
    for needle in ["Cat0", "→"] {
        let mut app = many_categories(6);
        right_click_on(&mut app, 100, 30, needle);
        assert!(
            app.popup.is_some(),
            "a right click on {needle:?} must open the overlay",
        );
        assert!(
            app.state.categories[0].is_unchecked(),
            "a right click on {needle:?} must not tick",
        );
    }
}

/// A category with no subcategories has no menu, so the right button has
/// nothing to open and must not fall back to ticking it — that is the left
/// button's job, and conflating the two would be the surprise the split
/// exists to avoid.
#[test]
fn right_clicking_a_category_without_subcategories_does_nothing() {
    // In the uneven grid `Logs` is the category with no subcategories at
    // all, so it has no overlay to open.
    let mut app = uneven_grid_app();
    right_click_on(&mut app, 100, 30, "Logs");
    assert!(app.popup.is_none(), "there is no overlay to open",);
    assert!(
        app.state.categories[1].is_unchecked(),
        "and the right button must not tick it either",
    );
}

/// The program rows get the same split: right button opens the category
/// overlay, left button ticks.
#[test]
fn right_clicking_a_program_opens_its_categories_without_ticking_it() {
    let mut app = sample_app();
    // Both categories, so Firefox really spans two of them and has a menu.
    press(&mut app, KeyCode::Char(' '));
    press(&mut app, KeyCode::Tab);
    press(&mut app, KeyCode::Char(' '));
    press(&mut app, KeyCode::Char('n'));

    right_click_on(&mut app, 100, 30, "Firefox");
    match app.popup.as_ref().map(|popup| popup.kind) {
        Some(PopupKind::Program(_)) => {}
        other => panic!("expected the category overlay, got {other:?}"),
    }
    assert!(
        app.state.is_program_checked(1),
        "the right button must not exclude the program",
    );
}

/// Everything with no menu behind it stays inert. This is the whole reason
/// the right button is routed through `open_menu` instead of `run`: a right
/// click that started a cleaning run, or set a volume, would be a genuinely
/// destructive surprise.
#[test]
fn a_right_click_on_anything_without_a_menu_does_nothing() {
    // The pinned button: a cleaning run must not start from a right click.
    let mut app = sample_app();
    app.state.toggle_category(0);
    app.state.current_page = Page::ProgramSelection;
    app.state.build_program_list();
    right_click_on(&mut app, 100, 30, "Start Cleaning");
    assert_eq!(
        app.state.current_page,
        Page::ProgramSelection,
        "a right click must not start the run",
    );
    assert!(app.popup.is_none());

    // A volume bar: nothing may happen. Asserted through the toast rather
    // than through the stored level, which is process-global — see
    // `config_lock` for why these tests must not read it.
    app.state.current_page = Page::Settings;
    let screen = draw(&mut app, 100, 30);
    let (column, row) = position_of(&screen, "Popup sound");
    let bar_left = column + "Popup sound ".len();
    right_click(&mut app, (bar_left + 2) as u16, row as u16);
    assert!(
        app.toast.is_none(),
        "a right click must not report a new level, so it set none",
    );

    // The backdrop of the update dialog must not dismiss it either, which is
    // what a stray right click near the dialog would otherwise do.
    let (mut app, _rx) = app_with_update();
    app.update_open = true;
    let screen = draw(&mut app, 100, 30);
    let (column, row) = position_of(&screen, "Categories");
    right_click(&mut app, column as u16, row as u16);
    assert!(app.update_open, "a right click is not a dismissal");
}

/// Both buttons stay live at once: right click opens, left click still ticks
/// the same cell afterwards.
#[test]
fn the_right_button_does_not_take_the_checkbox_away_from_the_left_one() {
    let mut app = sample_app();
    right_click_on(&mut app, 100, 30, "Cache");
    assert!(app.popup.is_some());

    // Closing the overlay, then ticking by left click as before.
    press(&mut app, KeyCode::Esc);
    assert!(app.popup.is_none());
    click_on(&mut app, 100, 30, "Cache");
    assert!(
        app.state.categories[0].is_checked(),
        "the left button must still tick",
    );
}

/// An overlay entry is already inside the menu, so there is nothing deeper
/// for the right button to open — and it must not toggle the entry either,
/// which would make a right click a hidden second way to change the
/// selection.
#[test]
fn right_clicking_an_overlay_entry_does_not_toggle_it() {
    let mut app = sample_app();
    press(&mut app, KeyCode::Right);
    right_click_on(&mut app, 100, 30, "Browser");
    assert!(app.popup.is_some(), "the overlay stays open");
    assert!(
        !app.state.categories[0].selected.contains("Browser"),
        "a right click must not tick the entry",
    );
}

#[test]
fn clicking_the_mirrored_cell_does_not_hit_its_neighbour() {
    // The regression: the right-hand cell is laid out backwards, so a target
    // measured from the left edge lands on the wrong category — and the
    // wrong one is the one the user can see they did not click.
    let mut app = many_categories(6);
    click_on(&mut app, 100, 30, "Cat3");
    assert!(app.state.categories[3].is_checked(), "Cat3 was clicked");
    assert!(
        app.state.categories[2].is_unchecked(),
        "Cat2 shares the row and must stay untouched",
    );
}

#[test]
fn clicking_a_cell_toggles_it_back_off() {
    let mut app = sample_app();
    click_on(&mut app, 100, 30, "Cache");
    assert!(app.state.has_selection());
    click_on(&mut app, 100, 30, "Cache");
    assert!(!app.state.has_selection());
}

/// A program in exactly one category says which one, after the arrow. The
/// count would be `→ 1`, which tells nobody anything.
#[test]
fn a_program_in_one_category_shows_the_category_name() {
    let mut app = single_category_program();
    assert_eq!(app.state.program_categories[0].len(), 1, "the premise");
    let screen = draw(&mut app, 100, 30);
    let line = screen
        .lines()
        .find(|line| line.contains("Solo"))
        .unwrap_or_else(|| panic!("the program is not listed:\n{screen}"));
    assert!(
        line.contains("→ Cache"),
        "the row must name the one category: {line:?}",
    );
    assert!(
        !line.contains("→ 1"),
        "a count of one says nothing: {line:?}",
    );
}

/// A program in several categories keeps the count, because the names would
/// not fit on a row and the number is what makes the menu behind it worth
/// opening. This is the split the window frontend draws as text versus a
/// menu button.
#[test]
fn a_program_in_several_categories_shows_the_count() {
    let mut app = sample_app();
    // Both categories, so Firefox really spans two of them.
    press(&mut app, KeyCode::Char(' '));
    press(&mut app, KeyCode::Tab);
    press(&mut app, KeyCode::Char(' '));
    press(&mut app, KeyCode::Char('n'));

    let screen = draw(&mut app, 100, 30);
    let line = screen
        .lines()
        .find(|line| line.contains("Firefox"))
        .unwrap_or_else(|| panic!("the program is not listed:\n{screen}"));
    assert!(
        line.contains("→ 2"),
        "two categories count as two: {line:?}"
    );
    // And the single-category program on the same list still names itself.
    let chrome = screen
        .lines()
        .find(|line| line.contains("Chrome"))
        .unwrap_or_else(|| panic!("Chrome is not listed:\n{screen}"));
    assert!(
        chrome.contains("→ Cache"),
        "Chrome is in one category, so it names it: {chrome:?}",
    );
}

/// The single-category text is not a control, so it must not be clickable.
/// A target there would either open an overlay with nothing to choose or
/// take the click away from ticking the row.
#[test]
fn a_single_category_name_is_not_clickable() {
    let mut app = single_category_program();
    let screen = draw(&mut app, 100, 30);
    let line = screen
        .lines()
        .find(|line| line.contains("→ Cache"))
        .unwrap_or_else(|| panic!("no category name on screen:\n{screen}"));
    let column = column_of(line, "→ Cache").expect("checked above");
    let row = screen
        .lines()
        .position(|l| l == line)
        .expect("the row exists");
    assert_eq!(
        app.hits.at(column as u16, row as u16),
        Some(Click::Program(0)),
        "the name belongs to the row, so it ticks the program",
    );

    click(&mut app, column as u16, row as u16);
    assert!(app.popup.is_none(), "there is nothing to open");
    assert!(
        !app.state.is_program_checked(0),
        "and the click must reach the row, not a menu",
    );
}

/// Right-clicking a single-category program must not open a one-item menu
/// either: there is no choice to offer, so the right button stays inert,
/// exactly as it does on a category with no subcategories.
#[test]
fn right_clicking_a_single_category_program_opens_nothing() {
    let mut app = single_category_program();
    let screen = draw(&mut app, 100, 30);
    let (column, row) = position_of(&screen, "Solo");
    right_click(&mut app, column as u16, row as u16);
    assert!(app.popup.is_none(), "one category offers nothing to choose",);
    assert!(app.state.is_program_checked(0), "and nothing was changed");
}

/// The `→ N` marker is the mouse's route to the subcategories, the way
/// `Enter` is the keyboard's. Without it a mouse user could never open the
/// overlay at all.
#[test]
fn clicking_the_marker_opens_the_subcategories_instead_of_ticking() {
    let mut app = sample_app();
    let screen = draw(&mut app, 100, 30);
    let (column, row) = position_of(&screen, "→");
    click(&mut app, column as u16, row as u16);

    let popup = app.popup.as_ref().expect("the marker opens the overlay");
    assert_eq!(popup.items.len(), 2, "Browser and Code");
    assert!(
        app.state.categories[0].is_unchecked(),
        "the click must not also tick the category",
    );
}

/// A category with no subcategories draws no marker, so it must have no
/// marker target either — otherwise the target would sit on top of the cell
/// and quietly take its clicks away.
#[test]
fn a_category_without_subcategories_keeps_its_whole_cell_clickable() {
    let mut app = uneven_grid_app();
    // `Logs` has no subcategories, so its cell has no arrow: clicking the
    // name has to tick it, not open an overlay.
    let screen = draw(&mut app, 100, 30);
    let (column, row) = position_of(&screen, "Logs");
    click(&mut app, column as u16, row as u16);
    assert!(
        app.state.categories[1].is_checked(),
        "the whole cell ticks, with no overlay target in the way",
    );
    assert!(app.popup.is_none(), "there is nothing to open");
}

#[test]
fn clicking_next_advances_to_the_program_page() {
    let mut app = sample_app();
    // Nothing selected yet: the click must be refused, with the same answer
    // `n` gives.
    click_on(&mut app, 100, 30, "Next");
    assert_eq!(app.state.current_page, Page::Main, "nothing selected yet");

    click_on(&mut app, 100, 30, "Cache");
    click_on(&mut app, 100, 30, "Next");
    assert_eq!(app.state.current_page, Page::ProgramSelection);
}

#[test]
fn clicking_a_program_row_toggles_it() {
    let mut app = sample_app();
    press(&mut app, KeyCode::Char(' '));
    press(&mut app, KeyCode::Char('n'));
    // The list arrives pre-selected, so this clears and re-selects it.
    click_on(&mut app, 100, 30, "Chrome");
    assert!(!app.state.is_program_checked(0), "Chrome was excluded");
    assert_eq!(app.program_cursor, 0);
    click_on(&mut app, 100, 30, "Chrome");
    assert!(app.state.is_program_checked(0), "and back on");
}

/// A program in several categories carries the `→ N` marker, which opens its
/// category overlay rather than ticking it — the same split as the category
/// grid, and the only mouse route to per-category exclusions.
#[test]
fn clicking_a_program_marker_opens_its_categories() {
    let mut app = sample_app();
    // Both categories, so Firefox really spans two of them and gets a marker.
    press(&mut app, KeyCode::Char(' '));
    press(&mut app, KeyCode::Tab);
    press(&mut app, KeyCode::Char(' '));
    press(&mut app, KeyCode::Char('n'));

    let screen = draw(&mut app, 100, 30);
    let (_, row) = position_of(&screen, "Firefox");
    // The marker follows the name on that row.
    let line = screen.lines().nth(row).expect("the row is on screen");
    let marker = column_of(line, "→").expect("Firefox is in two categories");
    click(&mut app, marker as u16, row as u16);

    let popup = app.popup.as_ref().expect("the marker opens the overlay");
    assert_eq!(popup.items.len(), 2, "Cache and Logs");
}

/// The overlay is modal for the mouse exactly as it is for the keyboard: a
/// click beside it dismisses it instead of reaching the page underneath.
#[test]
fn clicking_outside_the_overlay_dismisses_it() {
    let mut app = sample_app();
    press(&mut app, KeyCode::Right);
    assert!(app.popup.is_some());

    click_on(&mut app, 100, 30, "Categories");
    assert!(app.popup.is_none(), "a click outside dismisses");
    assert!(!app.state.has_selection(), "and changes nothing else");
}

#[test]
fn clicking_an_overlay_entry_toggles_it() {
    let mut app = sample_app();
    press(&mut app, KeyCode::Right);
    click_on(&mut app, 100, 30, "Browser");
    assert!(
        app.state.categories[0].selected.contains("Browser"),
        "the clicked entry is the one that changed",
    );
    assert!(app.popup.is_some(), "the overlay stays open");

    click_on(&mut app, 100, 30, "Code");
    assert!(
        app.state.categories[0].selected.contains("Code"),
        "the second entry toggles on its own",
    );
}

#[test]
fn clicking_the_search_field_puts_the_caret_in_it() {
    let mut app = sample_app();
    press(&mut app, KeyCode::Char(' '));
    press(&mut app, KeyCode::Char('n'));
    assert_eq!(app.input, InputMode::Normal);

    click_on(&mut app, 100, 30, "Search:");
    assert_eq!(
        app.input,
        InputMode::Editing,
        "clicking the field types into it",
    );
    // ...and the keystrokes reach the query.
    for c in "fox".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    assert_eq!(app.state.search_query_visible, "fox");
}

/// The volume bar is the terminal stand-in for the window frontend's slider:
/// a click sets the level under the pointer rather than stepping it.
#[test]
fn clicking_a_volume_bar_sets_the_level_under_the_pointer() {
    let _serial = config_lock();
    let mut app = sample_app();
    app.state.current_page = Page::Settings;
    let screen = draw(&mut app, 100, 30);
    let (column, row) = position_of(&screen, "Popup sound");
    // The bar follows the label, one space along — `register_rows` measures
    // the same columns.
    let bar_left = column + "Popup sound ".len();
    let bar_width = crate::pages::settings::BAR_WIDTH;
    click(&mut app, (bar_left + bar_width / 4) as u16, row as u16);
    let level = appcore::config::get().sound_volume;
    assert!(
        (0.2..0.3).contains(&level),
        "expected about a quarter, got {level}",
    );

    // The far left of the bar is silence, and the right end is full.
    click(&mut app, bar_left as u16, row as u16);
    assert_eq!(appcore::config::get().sound_volume, 0.0);
    click(&mut app, (bar_left + bar_width - 1) as u16, row as u16);
    assert_eq!(appcore::config::get().sound_volume, 1.0);

    // Clicking the row outside the bar only moves the cursor.
    click(&mut app, column as u16, row as u16);
    assert_eq!(app.settings_cursor, 0);
}

#[test]
fn clicking_a_volume_row_selects_it_for_the_arrow_keys() {
    let _serial = config_lock();
    let mut app = sample_app();
    app.state.current_page = Page::Settings;
    let screen = draw(&mut app, 100, 30);
    let (column, row) = position_of(&screen, "Done sound");
    click(&mut app, column as u16, row as u16);
    assert_eq!(app.settings_cursor, 3);

    // ...so the arrow keys act on the row the mouse chose.
    press(&mut app, KeyCode::Left);
    assert!(
        appcore::config::get().done_volume < 1.0,
        "the click's row is the one that gets adjusted",
    );
}

/// A click is a press. Acting on the release as well would fire every action
/// twice, which for `Start Cleaning` would start two cleaning jobs.
#[test]
fn a_click_does_not_fire_twice_on_the_release() {
    let mut app = sample_app();
    app.state.toggle_category(0);
    app.state.current_page = Page::ProgramSelection;
    app.state.build_program_list();
    draw(&mut app, 100, 30);
    let screen = draw(&mut app, 100, 30);
    let (column, row) = position_of(&screen, "Chrome");

    app.on_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: column as u16,
        row: row as u16,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(
        app.state.is_program_checked(0),
        "the release must not toggle it back",
    );
}

/// Dragging across the screen must not tick everything it passes over.
#[test]
fn dragging_the_button_across_the_grid_ticks_one_category() {
    let mut app = many_categories(6);
    let screen = draw(&mut app, 100, 30);
    let (column, row) = position_of(&screen, "Cat0");
    click(&mut app, column as u16, row as u16);
    let (column, _) = position_of(&screen, "Cat5");
    app.on_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: column as u16,
        row: row as u16,
        modifiers: KeyModifiers::NONE,
    }));
    let ticked = app
        .state
        .categories
        .iter()
        .filter(|category| !category.is_unchecked())
        .count();
    assert_eq!(ticked, 1, "only the press is an action");
}

/// A target from a page that is no longer on screen must not answer clicks.
#[test]
fn a_widget_stops_answering_once_the_page_is_gone() {
    let mut app = sample_app();
    app.state.toggle_category(0);
    let screen = draw(&mut app, 100, 30);
    let (column, row) = position_of(&screen, "Next");
    assert_eq!(
        app.hits.at(column as u16, row as u16),
        Some(Click::Next),
        "the button answers while it is on screen",
    );

    // Same position, but a page with nothing drawn there. The target is
    // rebuilt on every frame, so the one from the previous frame is gone.
    app.state.current_page = Page::Settings;
    let _ = draw(&mut app, 100, 30);
    assert_eq!(
        app.hits.at(column as u16, row as u16),
        None,
        "last frame's target must not survive the page change",
    );
}

/// Every page has to survive being drawn and clicked at its own corners with
/// either button, including while an overlay is up.
#[test]
fn clicking_never_panics_on_any_page_or_terminal_size() {
    // Clicking every cell of the settings page lands on the volume bars, and a
    // click on one of those writes the shared config — so this test is another
    // writer of the process-global volumes, and has to queue behind the tests
    // that assert on them.
    let _serial = config_lock();
    let mut app = sample_app();
    app.state.toggle_category(0);
    for (width, height) in [(100, 30), (20, 6), (8, 3), (1, 1)] {
        for page in [
            Page::Main,
            Page::ProgramSelection,
            Page::Clearing,
            Page::Results,
            Page::Settings,
        ] {
            app.state.current_page = page;
            draw(&mut app, width, height);
            for column in 0..width {
                for row in 0..height {
                    click(&mut app, column, row);
                    right_click(&mut app, column, row);
                }
            }
        }
    }
}

#[test]
fn the_wheel_scrolls_the_program_list_and_the_changelog() {
    let mut app = sample_app();
    press(&mut app, KeyCode::Char(' '));
    press(&mut app, KeyCode::Char('n'));
    assert_eq!(app.program_cursor, 0);
    wheel(&mut app, 1);
    assert!(
        app.program_cursor > 0,
        "the wheel moves the cursor on a list",
    );

    // On the changelog the wheel scrolls the content itself, which is the one
    // place the content is longer than the window.
    app.changelog = Some(long_changelog());
    press(&mut app, KeyCode::Char('?'));
    assert_eq!(app.changelog_scroll, 0);
    wheel(&mut app, 2);
    let scrolled = app.changelog_scroll;
    assert!(scrolled > 0, "the wheel scrolls the changelog");
    wheel(&mut app, -1);
    assert!(app.changelog_scroll < scrolled, "and back up");

    // And it is clamped by the render, not by the wheel.
    wheel(&mut app, 1_000);
    draw(&mut app, 60, 24);
    assert!(app.changelog_scroll <= app.changelog_max_scroll);
}

/// The overlay stays modal for the wheel too: it moves the overlay's
/// selection instead of the page behind it.
#[test]
fn the_wheel_moves_the_overlay_selection_not_the_page() {
    let mut app = sample_app();
    press(&mut app, KeyCode::Right);
    assert!(app.popup.is_some());
    wheel(&mut app, 1);
    assert_eq!(
        app.popup.as_ref().and_then(|popup| popup.list.selected()),
        Some(1),
        "the overlay's selection moved",
    );
}

/// Every dialog choice is clickable, and a click reaches the same action its
/// key does — including the guard that a running download refuses.
#[test]
fn the_update_dialog_answers_clicks_like_keys() {
    let (mut app, rx) = app_with_update();
    app.update_open = true;

    let screen = draw(&mut app, 100, 30);
    let (column, row) = position_of(&screen, "download and install");
    click(&mut app, column as u16, row as u16);
    assert!(
        updater::current(&app.updater_state).is_running(),
        "clicking `d` starts the download, as pressing it does",
    );
    assert!(
        matches!(rx.try_recv(), Ok(UpdaterCommand::Install(_))),
        "and hands the release to the worker",
    );

    // A click is a dismissal, so a running download has to refuse it exactly
    // as it refuses `Esc` — otherwise the mouse is a way around the guard.
    // The backdrop is the whole screen, so any column will do.
    let screen = draw(&mut app, 100, 30);
    let (column, row) = position_of(&screen, "downloading");
    click(&mut app, column as u16, row as u16);
    assert!(
        app.update_open,
        "a running download must not be dismissible by a click",
    );
}

/// A click beside the dialog dismisses it, and `Esc` still does.
#[test]
fn clicking_beside_the_update_dialog_dismisses_it() {
    let (mut app, _rx) = app_with_update();
    app.update_open = true;
    let screen = draw(&mut app, 100, 30);
    let (column, row) = position_of(&screen, "Categories");
    click(&mut app, column as u16, row as u16);
    assert!(!app.update_open, "the backdrop dismisses the dialog");
}

/// The changelog is text, so a click anywhere on it closes it — there is
/// nothing in it to hit but the dismissal.
#[test]
fn clicking_the_changelog_closes_it() {
    let mut app = sample_app();
    app.changelog = Some(long_changelog());
    press(&mut app, KeyCode::Char('?'));
    let screen = draw(&mut app, 60, 24);
    let (column, row) = position_of(&screen, "What's New");
    click(&mut app, column as u16, row as u16);
    assert!(!app.changelog_open);
}

/// A click must be routed while an overlay owns the screen, and must not
/// double-handle an overlay's key press.
#[test]
fn a_click_is_routed_through_the_overlay_that_is_open() {
    let mut app = sample_app();
    press(&mut app, KeyCode::Right);
    assert!(app.popup.is_some());
    // The popup is registered after the page, so its targets sit on top: a
    // click on a page button must not fire while the overlay is up.
    let screen = draw(&mut app, 100, 30);
    let (column, row) = position_of(&screen, "Next");
    click(&mut app, column as u16, row as u16);
    assert_eq!(
        app.state.current_page,
        Page::Main,
        "the page underneath must not act",
    );
    assert!(app.popup.is_none(), "the click dismissed the overlay");
}

/// The right button has to be discoverable, and it may only be advertised
/// where it does something — a hint that lies on the results table is worse
/// than no hint at all.
/// The right button has to be discoverable, and a hint that names it may
/// only sit on a page where it does something — the results table and the
/// progress meter have no menu behind them, and a lie there is worse than
/// silence.
#[test]
fn the_right_button_is_hinted_only_where_it_opens_something() {
    for (page, offered) in [
        (Page::Main, true),
        (Page::ProgramSelection, true),
        (Page::Clearing, false),
        (Page::Results, false),
        (Page::Settings, false),
    ] {
        let mut app = sample_app();
        app.state.toggle_category(0);
        app.state.current_page = page;
        // The hint line is one row and clips, so a phrase can be cut in half;
        // collapsed, the page's own hints read the same either way.
        let screen = draw(&mut app, 110, 30);
        let footer = screen.split_whitespace().collect::<Vec<_>>().join(" ");
        assert_eq!(
            footer.contains("right"),
            offered,
            "{page:?} hints the right button wrongly:\n{footer}",
        );
    }
}

/// The hint must survive being clipped, which is the whole reason it lives
/// inside the page's own hints rather than in the global group at the far
/// right of a line that is already too long.
#[test]
fn the_right_button_hint_is_not_clipped_away() {
    let mut app = sample_app();
    // The main page has the longest hint list of all, so it is the tight one.
    let screen = draw(&mut app, 110, 30);
    assert!(
        screen.contains("right"),
        "the hint got clipped off the footer:\n{}",
        screen.lines().last().unwrap_or_default(),
    );
}
