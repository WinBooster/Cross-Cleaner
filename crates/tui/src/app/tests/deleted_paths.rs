use super::*;

/// The point of the whole feature: a row is an aggregate, and the aggregate
/// has to be openable into the paths it is made of.
#[test]
fn enter_on_a_result_row_lists_the_paths_it_deleted() {
    let mut app = results_app();
    press(&mut app, KeyCode::Enter);
    let screen = draw(&mut app, 100, 30);
    assert!(screen.contains("C:/cache/a.tmp"), "{screen}");
    assert!(screen.contains("2.0 KB"), "the freed size: {screen}");
    assert!(screen.contains("3 paths"), "the count of paths: {screen}");
    // Only this row's paths — the other programs belong to other rows.
    assert!(!screen.contains("C:/ff/x.dat"), "{screen}");
}

/// The mouse must reach the same list `Enter` does, and on the row that was
/// clicked rather than on the one the keyboard was pointing at.
#[test]
fn clicking_a_result_row_lists_that_rows_paths() {
    let mut app = results_app();
    press(&mut app, KeyCode::Down);
    click_on(&mut app, 100, 30, "Firefox");

    let details = app.details.as_ref().expect("the row opened its paths");
    assert_eq!(details.row, 1, "the clicked row, not the focused one");
    let screen = draw(&mut app, 100, 30);
    assert!(screen.contains("C:/ff/x.dat"), "{screen}");
    assert!(!screen.contains("C:/cache/a.tmp"), "{screen}");
}

/// The overlay is modal over the table: the first `Esc` closes it, and only
/// the second leaves the page. One `Esc` that did both would make the
/// dismissal invisible.
#[test]
fn escape_closes_the_paths_and_only_then_the_page() {
    let mut app = results_app();
    press(&mut app, KeyCode::Enter);
    assert!(app.details.is_some());
    press(&mut app, KeyCode::Esc);
    assert!(app.details.is_none(), "the overlay closes first");
    assert_eq!(app.state.current_page, Page::Results, "and stays put");
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.state.current_page, Page::Main);
}

/// A click beside the box dismisses it, exactly as `Esc` does.
#[test]
fn clicking_beside_the_path_list_closes_it() {
    let mut app = results_app();
    press(&mut app, KeyCode::Enter);
    assert!(app.details.is_some());
    click_on(&mut app, 100, 30, "Categories");
    assert!(app.details.is_none(), "the backdrop dismisses");
    assert_eq!(app.state.current_page, Page::Results, "the page stays");
}

/// A click inside the box must not close it: the lines are text, and the
/// backdrop underneath them would answer for them.
#[test]
fn clicking_inside_the_path_list_keeps_it_open() {
    let mut app = results_app();
    press(&mut app, KeyCode::Enter);
    click_on(&mut app, 100, 30, "C:/cache/a.tmp");
    assert!(app.details.is_some(), "the list stays open");
}

/// One program can delete thousands of paths, so the list scrolls — and says
/// where in it the reader is.
#[test]
fn the_path_list_scrolls_and_reports_its_position() {
    let mut app = sample_app();
    let paths: Vec<(String, u64)> = (0..40)
        .map(|n| (format!("C:/cache/file{n}"), n as u64))
        .collect();
    let paths: Vec<(&str, u64)> = paths
        .iter()
        .map(|(path, bytes)| (path.as_str(), *bytes))
        .collect();
    app.state.cleared_data = Some((780, 40, 0, Arc::from(vec![cleared("Chrome", 780, &paths)])));
    app.state.current_page = Page::Results;

    press(&mut app, KeyCode::Enter);
    let top = draw(&mut app, 100, 30);
    assert!(top.contains("C:/cache/file0"), "{top}");
    assert!(top.contains("40 paths"), "{top}");
    assert!(
        top.contains("scroll"),
        "a list this long has to hint: {top}"
    );

    press(&mut app, KeyCode::Down);
    let moved = draw(&mut app, 100, 30);
    assert!(!moved.contains("C:/cache/file0"), "{moved}");
    assert!(moved.contains("C:/cache/file1"), "{moved}");

    // The wheel scrolls the list, not the table behind it.
    wheel(&mut app, 1);
    assert!(
        app.details.as_ref().is_some_and(|d| d.scroll > 1),
        "the wheel moved the list",
    );

    // And the render clamps it to the end instead of scrolling into nothing:
    // the last path is the last line on screen, whatever the box is tall.
    press(&mut app, KeyCode::End);
    let end = draw(&mut app, 100, 30);
    assert!(end.contains("C:/cache/file39"), "{end}");
    let details = app.details.as_ref().expect("still open");
    assert_eq!(details.scroll, details.max_scroll);
}

/// A row that reported a total without saying which path produced it has
/// nothing to list, and must say so instead of opening an empty box.
#[test]
fn a_row_without_deleted_paths_says_so() {
    let mut app = results_app();
    press(&mut app, KeyCode::End);
    press(&mut app, KeyCode::Enter);
    assert!(app.details.is_none(), "nothing to open");
    assert!(
        draw(&mut app, 100, 30).contains("no deleted paths"),
        "and it says why",
    );
}

/// Past the cap the list is incomplete, and the overlay has to admit it: a
/// truncated list that reads as the whole thing is worse than a short one.
#[test]
fn paths_left_out_by_the_cap_are_reported() {
    let mut app = results_app();
    app.state.cleared_data = Some((
        2048,
        1,
        0,
        Arc::from(vec![database::structures::Cleared {
            paths_omitted: 1200,
            ..cleared("Chrome", 2048, &[("C:/cache/a.tmp", 2048)])
        }]),
    ));
    press(&mut app, KeyCode::Enter);
    let screen = draw(&mut app, 100, 30);
    assert!(screen.contains("1200 more"), "{screen}");
    assert!(screen.contains("C:/cache/a.tmp"), "{screen}");
}

/// `Enter` on the same row twice is a toggle, so the key that opens the list
/// also closes it — a user who does not know it is there still gets out.
#[test]
fn enter_toggles_the_path_list_of_the_same_row() {
    let mut app = results_app();
    press(&mut app, KeyCode::Enter);
    assert!(app.details.is_some());
    press(&mut app, KeyCode::Enter);
    assert!(app.details.is_none());
}

/// The hint in the table's own border is the only place a user learns the
/// rows open anything at all.
#[test]
fn the_results_table_advertises_the_path_list() {
    let mut app = results_app();
    let screen = draw(&mut app, 110, 30);
    assert!(screen.contains("enter paths"), "{screen}");
}

/// A row means one program, and a sort moves the rows around — so `Enter` has to
/// keep following the row it was aimed at. Counting in the run's own list would
/// open the paths of whichever program landed on that position, which is a
/// different answer every time the table was re-ordered.
#[test]
fn enter_on_a_sorted_row_opens_that_rows_paths() {
    let mut app = sortable_results_app();
    // Biggest first, which is not the order the run reported.
    click_header(&mut app, 100, 30, "Size");
    press(&mut app, KeyCode::Enter);

    let screen = draw(&mut app, 100, 30);
    assert!(screen.contains("C:/vscode/a"), "{screen}");
    assert!(!screen.contains("C:/sharex/a"), "{screen}");
}

/// The same for the row's own target: after a sort the click targets have to be
/// counted in the order the rows are drawn in, not in the run's list.
#[test]
fn clicking_a_sorted_row_opens_that_rows_paths() {
    let mut app = sortable_results_app();
    // Most files first, so Adobe moves up to the second row from the bottom.
    click_header(&mut app, 100, 30, "Files");
    let screen = draw(&mut app, 100, 30);
    assert_row_order(&screen, &["ShareX", "Adobe", "Visual Studio Code"]);

    click_on(&mut app, 100, 30, "Adobe");
    let screen = draw(&mut app, 100, 30);
    assert!(screen.contains("C:/adobe/a"), "{screen}");
    assert!(!screen.contains("C:/vscode/a"), "{screen}");
}

/// The cursor counts rows the same way the targets do, so moving down and
/// pressing `Enter` lands on the row below the one that is highlighted.
#[test]
fn the_cursor_counts_rows_in_the_drawn_order() {
    let mut app = sortable_results_app();
    click_header(&mut app, 100, 30, "Dirs");
    // Dirs descending: VS Code, Adobe, ShareX.
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Enter);
    let screen = draw(&mut app, 100, 30);
    assert!(screen.contains("C:/adobe/a"), "{screen}");
}

/// A new run is a new report: it arrives in the cleaner's own order, and the
/// order the reader chose for the last one is an answer about *those* numbers,
/// not a setting to carry onto a table whose values are all different now.
#[test]
fn a_finished_run_reports_itself_in_its_own_order() {
    // A real run, so the report really arrives through the channel the app polls.
    let _serial = sequential();
    let _guard = DiagnosticsGuard::install();
    let mut app = sortable_results_app();
    click_header(&mut app, 100, 30, "Size");
    let (header, _) = report_header(&mut app, 100, 30);
    assert!(
        header.contains("Size ↓"),
        "the sort is in force: {header:?}"
    );

    // The sample entries have nothing to remove, so no file is touched and the
    // report that comes back is empty — the header is all there is to read.
    app.state.current_page = Page::ProgramSelection;
    app.state.cleared_data = None;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("runtime");
    runtime.block_on(async {
        app.state.start_cleaning();
    });
    for _ in 0..500 {
        app.tick();
        if app.state.current_page == Page::Results {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        app.state.current_page,
        Page::Results,
        "the run never finished",
    );

    let (header, _) = report_header(&mut app, 100, 30);
    assert!(
        !header.contains('↓') && !header.contains('↑'),
        "a new report is ordered by nobody: {header:?}",
    );
}

/// A path is the one string on this page that cannot be shortened, so the
/// overlay takes the width it can. It used to take three quarters of the
/// screen and cap out at 110 columns: at 120 columns that left the box 90
/// wide, and this path — the length a real Chrome cache entry runs to — no
/// longer fitted on one line beside its size.
#[test]
fn the_path_list_uses_the_width_it_can() {
    let mut app = sample_app();
    let long = "C:\\Users\\WindowsUser\\AppData\\Local\\Google\\Chrome\\User Data\\Default\\Cache\\Cache_Data\\f_000002";
    // The premise, stated as the geometry that used to break it: three
    // quarters of 120, minus the borders, was 88 columns for a 92-char path
    // plus its size.
    let old_inner = 120 * 3 / 4 - 2;
    assert!(
        long.chars().count() + " 2.0 KB".len() > old_inner as usize,
        "the premise: a path this long has to be what is at stake",
    );
    app.state.cleared_data = Some((
        2048,
        1,
        0,
        Arc::from(vec![cleared("Chrome", 2048, &[(long, 2048)])]),
    ));
    app.state.current_page = Page::Results;

    press(&mut app, KeyCode::Enter);
    let screen = draw(&mut app, 120, 30);
    let row = screen
        .lines()
        .find(|line| line.contains("Cache_Data"))
        .unwrap_or_else(|| panic!("the path is not listed:\n{screen}"));
    assert!(row.contains(long), "the path must fit on one line: {row:?}");
    assert!(row.contains("2.0 KB"), "and keep its size: {row:?}");
}

/// The list belongs to the report, so it is drawn *inside* it: the summary
/// above and the footer below stay where they were, and the box is inset
/// from the page rather than covering it.
#[test]
fn the_path_list_sits_inside_the_report() {
    let mut app = results_app();
    let without = draw(&mut app, 100, 30);
    press(&mut app, KeyCode::Enter);
    let with = draw(&mut app, 100, 30);

    // The header and the footer are the page's, not the overlay's: an overlay
    // that covered them would be a screen, not a detail of the report.
    let header = without.lines().next().expect("a header row");
    let footer = without.lines().last().expect("a footer row");
    assert_eq!(
        with.lines().next(),
        Some(header),
        "the header must survive the overlay:\n{with}",
    );
    assert_eq!(
        with.lines().last(),
        Some(footer),
        "and so must the footer:\n{with}",
    );
    assert!(
        with.contains("Cleaning Results"),
        "and the summary:\n{with}"
    );

    // The box is inset from the page by the same margin on every side. Read
    // off the borders: they are the only rows guaranteed to be intact, since a
    // long title is what wraps when the box gets narrow.
    let margin = crate::pages::results::DETAIL_MARGIN as usize;
    let width = 100usize;
    let rows: Vec<&str> = with.lines().collect();
    // The report draws its own frame, so there are two boxes on screen. The
    // overlay's corners are the ones inset from the edge; the report's sit
    // on it, which is what tells the two apart.
    // The report draws its own frame too, so two boxes are on screen. The overlay's
    // corners are the ones inset from the edge; the report's sit on it, which
    // is what tells the two apart.
    let inset = |line: &str, open: char| {
        line.chars()
            .position(|c| c == open)
            .filter(|column| *column >= margin && *column < width - margin)
    };
    let (top_row, left) = rows
        .iter()
        .enumerate()
        .find_map(|(row, line)| Some((row, inset(line, '╭')?)))
        .expect("the overlay's top-left corner");
    let (bottom_row, right) = rows
        .iter()
        .enumerate()
        .rev()
        .find_map(|(row, line)| Some((row, inset(line, '╰')?)))
        .map(|(row, _)| (row, inset(rows[row], '╯').expect("its right corner")))
        .expect("the overlay's bottom-left corner");
    assert!(
        bottom_row > top_row,
        "the two corners must be the overlay's, not the report's:\n{with}",
    );
    assert_eq!(left, margin, "left margin:\n{with}");
    assert_eq!(right, width - 1 - margin, "right margin:\n{with}",);
    // The header row is the page's first row and the footer its last, so the
    // report runs between them. The box clears the top by the full margin and
    // the foot by the smaller one, so the report is still visible under it.
    assert_eq!(
        top_row,
        1 + margin,
        "top margin, inside the report:\n{with}",
    );
    assert_eq!(
        bottom_row,
        30 - 2 - crate::pages::results::DETAIL_MARGIN_BOTTOM as usize,
        "bottom margin, inside the report:\n{with}",
    );
    assert!(
        crate::pages::results::DETAIL_MARGIN_BOTTOM < crate::pages::results::DETAIL_MARGIN,
        "the foot margin is deliberately the smaller of the two",
    );
}

/// Narrow terminals have to keep working: the box shrinks with them rather
/// than overflowing or panicking, and the path wraps instead of vanishing.
#[test]
fn the_path_list_survives_a_narrow_terminal() {
    let mut app = results_app();
    press(&mut app, KeyCode::Enter);
    for (width, height) in [(100, 30), (40, 12), (20, 6), (8, 3), (1, 1)] {
        let screen = draw(&mut app, width, height);
        for line in screen.lines() {
            assert!(
                line.chars().count() <= width as usize,
                "{width}x{height}: row overflows: {line:?}",
            );
        }
    }
}
