use super::*;

#[test]
fn changelog_overlay_opens_and_closes() {
    let mut app = sample_app();
    press(&mut app, KeyCode::Char('?'));
    assert!(app.changelog_open);
    // The fetch runs in the background; the overlay must render either way.
    let screen = draw(&mut app, 100, 30);
    assert!(screen.contains("What's New"), "{screen}");
    press(&mut app, KeyCode::Esc);
    assert!(!app.changelog_open);
}

#[test]
fn the_changelog_scrolls_and_shows_its_position() {
    let mut app = sample_app();
    // Stand in for a finished fetch: the overlay is under test, not the
    // network round trip.
    app.changelog = Some(long_changelog());
    press(&mut app, KeyCode::Char('?'));

    let top = draw(&mut app, 60, 24);
    assert!(top.contains("Change 0.0"), "{top}");
    assert!(top.contains("1/"), "position belongs in the title:\n{top}");
    assert!(top.contains("scroll"), "long content must hint it:\n{top}");

    press(&mut app, KeyCode::Down);
    let one = draw(&mut app, 60, 24);
    assert_ne!(top, one, "Down must move the content");

    press(&mut app, KeyCode::PageDown);
    let paged = draw(&mut app, 60, 24);
    assert_ne!(one, paged, "PageDown must move further");

    // End jumps to the very bottom.
    press(&mut app, KeyCode::End);
    let end = draw(&mut app, 60, 24);
    assert!(end.contains("Contributor 19"), "{end}");

    // Home comes back to the top.
    press(&mut app, KeyCode::Home);
    assert_eq!(app.changelog_scroll, 0);
    assert_eq!(draw(&mut app, 60, 24), top);
}

#[test]
fn the_changelog_scroll_is_clamped_to_the_content() {
    let mut app = sample_app();
    app.changelog = Some(long_changelog());
    press(&mut app, KeyCode::Char('?'));
    for _ in 0..500 {
        press(&mut app, KeyCode::Down);
    }
    // The render clamps to the last usable offset instead of scrolling into
    // empty space.
    let screen = draw(&mut app, 60, 24);
    assert!(
        screen.contains("Contributor 19"),
        "should stop at the bottom:\n{screen}",
    );
    assert!(app.changelog_scroll <= app.changelog_max_scroll);
}

#[test]
fn a_short_changelog_needs_no_scrolling() {
    let mut app = sample_app();
    app.changelog = Some(Changelog {
        groups: vec![ChangelogGroup {
            title: "Only group".to_string(),
            items: vec!["Only item".to_string()],
        }],
        contributors: vec![],
    });
    press(&mut app, KeyCode::Char('?'));
    let screen = draw(&mut app, 60, 24);
    assert!(screen.contains("Only item"), "{screen}");
    assert!(!screen.contains("scroll"), "nothing to scroll:\n{screen}");
    assert_eq!(app.changelog_max_scroll, 0);
}

#[test]
fn reopening_the_changelog_starts_at_the_top() {
    let mut app = sample_app();
    app.changelog = Some(long_changelog());
    press(&mut app, KeyCode::Char('?'));
    press(&mut app, KeyCode::PageDown);
    press(&mut app, KeyCode::PageDown);
    assert!(app.changelog_scroll > 0);
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Char('?'));
    assert_eq!(app.changelog_scroll, 0);
}

#[test]
fn the_changelog_scrolls_while_the_fetch_is_still_running() {
    let mut app = sample_app();
    press(&mut app, KeyCode::Char('?'));
    // Nothing loaded yet: scrolling must stay put rather than drift.
    for _ in 0..20 {
        press(&mut app, KeyCode::Down);
    }
    let screen = draw(&mut app, 60, 24);
    assert!(screen.contains("Loading changelog"), "{screen}");
}
