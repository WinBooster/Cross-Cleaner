use super::*;

/// The pinned action button must be exactly as tall as its label.
///
/// The button row used to be three rows high while the label is one line, so
/// two rows of bare background hung under `Next` and read as dead space — the
/// one thing a pinned primary action should not have below it.
#[test]
fn the_pinned_button_has_nothing_blank_below_it() {
    for (page, label) in [
        (Page::Main, "Next"),
        (Page::ProgramSelection, "Start Cleaning"),
    ] {
        let mut app = sample_app();
        // Selected through the state rather than with a key press: a press
        // raises a toast, which takes two footer rows and lifts the button.
        app.state.toggle_category(0);
        assert!(
            app.state.has_selection(),
            "the list must have something to show"
        );
        app.state.current_page = page;

        let screen = draw(&mut app, 80, 20);
        assert!(app.toast.is_none(), "no toast: this is about the layout");
        let lines: Vec<&str> = screen.lines().collect();
        let button = lines
            .iter()
            .position(|line| line.contains(label))
            .unwrap_or_else(|| panic!("no `{label}` on {page:?}:\n{screen}"));
        // The footer is the last row of the frame, so the button belongs on
        // the one right above it.
        assert_eq!(
            button + 1,
            lines.len() - 1,
            "`{label}` must touch the footer, not float above a blank strip:\n{screen}",
        );
    }
}

#[test]
fn main_page_lists_every_category() {
    let mut app = sample_app();
    let screen = draw(&mut app, 100, 30);
    assert!(screen.contains("Cache"), "{screen}");
    assert!(screen.contains("Logs"), "{screen}");
    // The pinned button is the window frontend's Next.
    assert!(screen.contains("Next"), "{screen}");
}

#[test]
fn space_toggles_the_highlighted_category() {
    let mut app = sample_app();
    assert!(!app.state.has_selection());
    press(&mut app, KeyCode::Char(' '));
    assert!(app.state.has_selection());
    assert!(draw(&mut app, 100, 30).contains("[x]"));
    press(&mut app, KeyCode::Char(' '));
    assert!(!app.state.has_selection());
}

#[test]
fn next_requires_a_selection_and_then_opens_the_program_list() {
    let mut app = sample_app();
    press(&mut app, KeyCode::Char('n'));
    assert_eq!(app.state.current_page, Page::Main, "nothing selected yet");
    press(&mut app, KeyCode::Char(' '));
    press(&mut app, KeyCode::Char('n'));
    assert_eq!(app.state.current_page, Page::ProgramSelection);
    // Two distinct programs across the two categories.
    assert_eq!(app.state.filtered_programs.len(), 2);
    let screen = draw(&mut app, 100, 30);
    assert!(screen.contains("Chrome"), "{screen}");
    assert!(screen.contains("Firefox"), "{screen}");
}

#[test]
fn arrows_move_by_row_and_tab_by_column() {
    let mut app = many_categories(6);
    // Six categories are three rows of two.
    assert_eq!(app.category_cursor, 0);
    press(&mut app, KeyCode::Down);
    assert_eq!(app.category_cursor, 2, "down moves a whole row");
    press(&mut app, KeyCode::Tab);
    assert_eq!(app.category_cursor, 3, "tab steps to the next column");
    press(&mut app, KeyCode::Down);
    assert_eq!(app.category_cursor, 5, "and keeps the column");
    press(&mut app, KeyCode::Down);
    assert_eq!(app.category_cursor, 5, "clamped at the last row");
    press(&mut app, KeyCode::Up);
    assert_eq!(app.category_cursor, 3);
    press(&mut app, KeyCode::BackTab);
    assert_eq!(app.category_cursor, 2);
}

#[test]
fn down_never_moves_the_toggle_into_another_column() {
    // The bug this guards: the highlight moved a row down while `Space`
    // still toggled the checkbox of the previous row's second column.
    let mut app = many_categories(6);
    for expected in [0, 2, 4] {
        assert_eq!(app.category_cursor, expected);
        press(&mut app, KeyCode::Char(' '));
        // The category that actually got toggled is the highlighted one.
        assert!(
            !app.state.categories[expected].is_unchecked(),
            "category {expected} was not the toggled one",
        );
        // And nothing in the same row's other column changed.
        if expected + 1 < app.state.categories.len() {
            assert!(
                app.state.categories[expected + 1].is_unchecked(),
                "column {} must stay untouched",
                expected + 1
            );
        }
        press(&mut app, KeyCode::Down);
    }
}

#[test]
fn the_highlight_sits_on_the_cell_space_would_toggle() {
    let mut app = many_categories(6);
    press(&mut app, KeyCode::Tab);
    assert_eq!(app.category_cursor, 1);
    let screen = draw(&mut app, 100, 30);
    // Every cell emits a marker slot, but only the focused one is filled,
    // so the whole grid must contain exactly one.
    assert_eq!(screen.matches('▸').count(), 1, "{screen}");
}

#[test]
fn both_grid_columns_line_up_on_every_row() {
    // A uniform grid, so the two cells of a row are identical in shape and
    // every checkbox must land on the same column. This is the regression
    // test for cells that were not padded to a common width.
    let mut app = many_categories(6);
    let screen = draw(&mut app, 100, 30);
    let rows = grid_rows(&screen);
    assert!(rows.len() >= 3, "expected three grid rows:\n{screen}");

    let columns: Vec<Vec<usize>> = rows.iter().map(|row| checkbox_columns(row)).collect();
    assert!(
        columns.iter().all(|row| row.len() == 2),
        "a grid row does not hold two cells: {columns:?}\n{screen}",
    );
    assert!(
        columns.windows(2).all(|pair| pair[0] == pair[1]),
        "columns drifted between rows: {columns:?}\n{screen}",
    );
}

#[test]
fn uneven_cells_still_share_the_same_grid_edges() {
    let mut app = uneven_grid_app();
    let screen = draw(&mut app, 100, 30);
    let rows = grid_rows(&screen);
    assert!(rows.len() >= 2, "expected two grid rows:\n{screen}");

    // The left cell starts at the same column in every row: the focus marker
    // is one column wide whether or not it is filled.
    let starts: Vec<usize> = rows
        .iter()
        .map(|row| checkbox_columns(row).first().copied().unwrap_or(usize::MAX))
        .collect();
    assert!(
        starts.windows(2).all(|pair| pair[0] == pair[1]),
        "left edge drifted: {starts:?}\n{screen}",
    );
    // The mirrored cell puts its checkbox last, so the distance from the
    // right edge depends only on that layout — not on the label length or on
    // whether the category carries a subcategory marker at all.
    let mirrored: Vec<usize> = rows
        .iter()
        .map(|row| {
            let width = row.chars().count();
            checkbox_columns(row)
                .get(1)
                .map(|at| width - at)
                .unwrap_or_default()
        })
        .collect();
    assert!(
        mirrored.iter().all(|d| *d == mirrored[0]),
        "mirrored cells drift from the edge: {mirrored:?}\n{screen}",
    );
}

#[test]
fn the_second_column_mirrors_the_first() {
    let mut app = mirrored_grid_app();
    let screen = draw(&mut app, 60, 20);
    let row = grid_rows(&screen).remove(0);

    let boxes = checkbox_columns(&row);
    assert_eq!(boxes.len(), 2, "{row:?}");
    let (first_box, second_box) = (boxes[0], boxes[1]);

    // Exactly one arrow per column, pointing opposite ways: the left column
    // keeps `→`, the mirrored right column uses `←`.
    assert_eq!(row.matches('→').count(), 1, "{row:?}");
    assert_eq!(row.matches('←').count(), 1, "{row:?}");
    let right_arrow = row.find('→').expect("checked above");
    let left_arrow = row.find('←').expect("checked above");

    // Left cell reads: marker, checkbox, label, arrow.
    assert!(
        first_box < right_arrow,
        "the checkbox leads the left cell: {row:?}",
    );
    // Right cell is the mirror: the count and its arrow come first, then the
    // label, and the checkbox trails at the window edge.
    assert!(
        left_arrow < second_box,
        "the mirrored checkbox must trail its arrow: {row:?}",
    );
    assert!(
        second_box > right_arrow,
        "the mirrored cell must follow the first: {row:?}",
    );
}

#[test]
fn mirroring_survives_a_narrow_terminal() {
    let mut app = mirrored_grid_app();
    // Below the width where two cells fit at both edges the grid packs them
    // left to right, and the mirror has to hold there too.
    let screen = draw(&mut app, 34, 20);
    let row = grid_rows(&screen).remove(0);
    let boxes = checkbox_columns(&row);
    assert_eq!(boxes.len(), 2, "{row:?}");
    let right_arrow = row.find('→').expect("left cell keeps its arrow");
    let left_arrow = row.find('←').expect("mirrored cell keeps its arrow");
    assert!(boxes[0] < right_arrow, "{row:?}");
    assert!(left_arrow < boxes[1], "{row:?}");
}

#[test]
fn a_category_without_subcategories_has_no_arrow_in_either_column() {
    let mut app = uneven_grid_app();
    let screen = draw(&mut app, 100, 30);

    // In this grid every right-hand category (`Logs`,
    // `Documentation and manuals`) has no subcategories, so neither column
    // may draw a mirrored arrow — not even an empty one.
    for row in grid_rows(&screen) {
        assert_eq!(row.matches('←').count(), 0, "{row:?}");
    }

    // The positive case: the same layout with a right-hand category that does
    // have subcategories must produce exactly one mirrored arrow.
    let mut app = mirrored_grid_app();
    let screen = draw(&mut app, 60, 20);
    let row = grid_rows(&screen).remove(0);
    assert_eq!(row.matches('←').count(), 1, "{row:?}");
}

#[test]
fn a_long_name_never_pushes_the_mirrored_cell_off_screen() {
    let mut app = uneven_grid_app();
    // Row 1 left cell has the longest label in the grid; the mirrored cell
    // must still be fully visible, checkbox included.
    let screen = draw(&mut app, 40, 20);
    for row in grid_rows(&screen) {
        assert_eq!(
            checkbox_columns(&row).len(),
            2,
            "a cell was clipped away: {row:?}",
        );
    }
}

#[test]
fn an_uneven_last_row_keeps_its_cell_on_the_left() {
    let mut app = uneven_grid_app();
    // Drop one category so the grid ends with a short row.
    app.state.categories.pop();
    let screen = draw(&mut app, 100, 30);
    let last = grid_rows(&screen).pop().expect("a grid row");
    assert_eq!(
        checkbox_columns(&last).len(),
        1,
        "the short row should hold a single cell: {last:?}",
    );
    assert!(
        last.starts_with(char::is_whitespace),
        "single cell should hug the left edge: {last:?}",
    );
}

#[test]
fn a_narrow_terminal_packs_the_grid_instead_of_overlapping() {
    let mut app = many_categories(4);
    // Too narrow for one cell at each edge, so they are packed left to right
    // instead of overlapping.
    let screen = draw(&mut app, 34, 20);
    for row in grid_rows(&screen) {
        assert_eq!(
            checkbox_columns(&row).len(),
            2,
            "cells overlapped or were dropped: {row:?}",
        );
        assert!(row.chars().count() <= 34, "row overflows: {row:?}");
    }
}

#[test]
fn right_opens_the_subcategory_overlay_and_space_toggles_a_sub() {
    let mut app = sample_app();
    press(&mut app, KeyCode::Right);
    let popup = app.popup.as_ref().expect("overlay is open");
    assert_eq!(popup.items.len(), 2, "Browser and Code");
    assert!(popup.items.iter().all(|(_, on)| !on));
    let screen = draw(&mut app, 100, 30);
    assert!(screen.contains("Browser"), "{screen}");

    press(&mut app, KeyCode::Char(' '));
    let popup = app.popup.as_ref().expect("still open");
    assert!(popup.items[0].1, "first subcategory is now selected");
    assert!(app.state.categories[0].selected.contains("Browser"));

    press(&mut app, KeyCode::Esc);
    assert!(app.popup.is_none());
}

#[test]
fn overlay_closes_on_escape_without_changing_anything() {
    let mut app = sample_app();
    press(&mut app, KeyCode::Right);
    press(&mut app, KeyCode::Esc);
    assert!(app.popup.is_none());
    assert!(!app.state.has_selection());
}
