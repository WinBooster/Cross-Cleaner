//! Every key press and every mouse event, and what they do.
//!
//! The keyboard and the mouse are not two front ends here: a click resolves to
//! the same [`Click`] its key press would have reached, and [`TuiApp::run`]
//! sends each of those to one method. That is what keeps the footer hints honest
//! — anything they promise is one method, reached two ways.

use std::sync::Arc;

use appcore::Toggle;
use appcore::app::Page;
use appcore::sounds;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::{Position, Rect};
use ratatui::widgets::ListState;

use crate::keymap;
use crate::pages;
use crate::pages::results::{ResultSort, SortColumn, sorted_rows};

use super::TuiApp;
use super::click::{Click, UpdateAction};
use super::overlay::{InputMode, PathDetails, Popup, PopupKind, Toast};
use super::sound::{play_toggle, sfx};

/// Lines a page-up / page-down moves in the changelog overlay.
const CHANGELOG_PAGE: usize = 10;

/// Lines a page-up / page-down moves in the deleted-path overlay.
const DETAILS_PAGE: isize = 10;

/// Rows one wheel notch moves the focused row, the step a terminal wheel
/// reports on every platform.
const WHEEL_STEP: isize = 3;

impl TuiApp {
    /// Feeds one terminal event into the app.
    pub fn on_event(&mut self, event: Event) {
        match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => self.on_key(key),
            Event::Mouse(mouse) => self.on_mouse(mouse),
            // The kitty protocol also reports key releases; the app is driven
            // by presses only, so a release must not double-fire a binding.
            _ => {}
        }
    }

    /// Routes a mouse event.
    ///
    /// The app owns the whole alternate screen, so a column and a row are the
    /// buffer coordinates [`TuiApp::render`] drew into and the rectangles
    /// registered last frame line up with it.
    fn on_mouse(&mut self, mouse: MouseEvent) {
        let at = Position::new(mouse.column, mouse.row);
        match mouse.kind {
            // Only the press, so dragging the button across the screen cannot
            // fire an action on every widget it passes over.
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(click) = self.hits.at(mouse.column, mouse.row) {
                    self.run(click, mouse.column);
                }
            }
            MouseEventKind::Down(MouseButton::Right) => {
                if let Some(click) = self.hits.at(mouse.column, mouse.row) {
                    self.open_menu(click);
                }
            }
            MouseEventKind::ScrollUp => self.scroll(-1, at),
            MouseEventKind::ScrollDown => self.scroll(1, at),
            // Motion, drags and the other buttons belong to terminal text
            // selection; the app has nothing to say about them.
            _ => {}
        }
    }

    /// What the right button does with whatever the click resolved to.
    ///
    /// The right button is the terminal's context menu, and the one menu this
    /// app has is a category's subcategory overlay — the very overlay its `→ N`
    /// marker opens. So a right click on a checkbox opens the subcategories and
    /// leaves the tick alone, which is what the right button means everywhere
    /// else.
    ///
    /// Everything else is deliberately inert. A target with no menu behind it —
    /// a pinned button, a volume bar, a dialog row — does nothing, because a
    /// right click that silently started a cleaning run would be far worse than
    /// one that does nothing at all.
    fn open_menu(&mut self, click: Click) {
        match click {
            // A cell and its own marker open the same overlay, so where in the
            // cell the right button landed makes no difference here.
            Click::Category(index) | Click::CategorySubs(index) => self.open_category_subs(index),
            Click::Program(row) | Click::ProgramCategories(row) => {
                self.open_program_categories(row)
            }
            // An overlay entry is already inside the menu, and a dismissal has
            // nothing behind it: there is no deeper menu to open.
            _ => {}
        }
    }

    /// Performs the action a click resolved to.
    ///
    /// Every branch calls the method the matching key press calls, so the two
    /// ways in cannot drift apart: same action, same sound, same toast. `column`
    /// is the click's own column, which is what lets a click on a volume bar
    /// set it to the level under the pointer instead of stepping it.
    fn run(&mut self, click: Click, column: u16) {
        match click {
            Click::Category(index) => self.toggle_category(index),
            Click::CategorySubs(index) => self.open_category_subs(index),
            Click::Program(row) => self.toggle_program(row),
            Click::ProgramCategories(row) => self.open_program_categories(row),
            Click::PopupItem(row) => self.toggle_popup_entry(row),
            Click::Settings(slot) => self.settings_cursor = slot,
            Click::Volume { slot, left } => {
                self.set_volume(slot, pages::settings::level_at(left, column));
            }
            Click::Search => self.input = InputMode::Editing,
            Click::Next => self.advance_to_programs(),
            Click::StartCleaning => self.start_cleaning(),
            Click::ResultRow(row) => self.toggle_details(row),
            // One label, three answers, and the button alone says which one: the
            // chain is built by clicking, never by holding a modifier down —
            // terminals do not all report one, and a control that works only on
            // the ones that do is a control that is not there.
            Click::SortColumn(column) => self.cycle_result_sort(column),
            // A line of the overlay is text, like a changelog line: the click is
            // swallowed rather than answered, which is also what keeps it from
            // dismissing the overlay the user is reading.
            Click::DetailRow(_) => {}
            Click::CloseDetails => self.details = None,
            Click::Update(action) => self.run_update(action),
            Click::ClosePopup => self.popup = None,
            Click::CloseChangelog => self.changelog_open = false,
            // Dismissing the dialog is `Esc`, and a running download refuses it
            // — the rule lives in `run_update`, which is why the click goes
            // through there too.
            Click::CloseDialog => self.run_update(UpdateAction::Later),
        }
    }

    /// Moves whatever is under the wheel by `notches`.
    ///
    /// `at` is where the wheel is: the report's labels are a target for it, and
    /// that is the one place the wheel clears the sort instead of scrolling —
    /// the sort is what is drawn there, so it is what a wheel over it should
    /// undo. Everywhere else the wheel keeps scrolling, as it always did.
    fn scroll(&mut self, notches: isize, at: Position) {
        if self.changelog_open {
            // A release note is far longer than its overlay, so here the wheel
            // has to scroll the content itself rather than move a cursor.
            self.changelog_scroll = self
                .changelog_scroll
                .saturating_add_signed(notches * WHEEL_STEP);
            return;
        }
        if self.details.is_some() {
            // Same shape: a program can delete thousands of paths, so the wheel
            // scrolls the list instead of moving the table behind it.
            self.scroll_details(notches * WHEEL_STEP);
            return;
        }
        if let Some(len) = self.popup.as_ref().map(|popup| popup.items.len()) {
            // The overlay is one long list of checkboxes, and the selection is
            // what scrolls it.
            self.move_popup(notches, len);
            return;
        }
        // After the overlays, never before: one of them owns the screen, and a
        // scroll over it belongs to it rather than to the report underneath.
        if self.over_result_header(at) {
            self.reset_result_sort();
            return;
        }
        let step = notches * WHEEL_STEP;
        match self.state.current_page {
            // The grid is two categories wide, so a notch is a row, not a cell.
            Page::Main => self.move_category_row(notches),
            Page::ProgramSelection => {
                move_index(
                    &mut self.program_cursor,
                    step,
                    self.state.filtered_programs.len(),
                );
            }
            Page::Results => {
                let len = self.result_count();
                move_index(&mut self.result_cursor, step, len);
            }
            // Cleaning owns the page, and the settings list is four rows tall.
            Page::Clearing | Page::Settings => {}
        }
    }

    /// Records that `rect` answers a left click with `click`.
    ///
    /// Pages call this while drawing, which is the only moment the geometry is
    /// known: the same values that put a widget on screen say where it is, so a
    /// click target cannot drift away from the glyph it belongs to. Order is
    /// paint order — see [`HitAreas`].
    pub(crate) fn hit(&mut self, rect: Rect, click: Click) {
        self.hits.push(rect, click);
    }

    /// Routes a key press to whichever overlay currently owns the keyboard.
    fn on_key(&mut self, key: KeyEvent) {
        // Ctrl+C quits from anywhere and must survive a non-English layout:
        // on a Russian keyboard that key reports `с`, not `c`.
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && keymap::normalize(key.code) == KeyCode::Char('c')
        {
            self.should_quit = true;
            return;
        }
        if self.changelog_open {
            self.on_key_changelog(key);
        } else if self.update_open {
            self.on_key_update(key);
        } else if self.details.is_some() {
            self.on_key_details(key);
        } else if self.popup.is_some() {
            self.on_key_popup(key);
        } else if self.input == InputMode::Editing {
            // No normalization here: the user is typing a query, so their
            // characters must survive verbatim or Cyrillic program names would
            // be unsearchable.
            self.on_key_search(key);
        } else {
            self.on_key_normal(Self::physical(key));
        }
    }

    /// A copy of `key` whose character comes from an English layout.
    ///
    /// Terminals report the character the active layout produces, so without
    /// this every letter binding would stop working under, say, a Russian
    /// keyboard. See [`crate::keymap`].
    fn physical(key: KeyEvent) -> KeyEvent {
        let code = keymap::normalize(key.code);
        if code == key.code {
            key
        } else {
            KeyEvent { code, ..key }
        }
    }

    fn on_key_normal(&mut self, key: KeyEvent) {
        if self.on_global(key) {
            return;
        }
        match key.code {
            KeyCode::Esc => {
                // Back navigation first; quit only from the main page.
                if !self.state.go_back() {
                    self.should_quit = true;
                }
                // Leaving the page drops the report the overlay was describing.
                self.details = None;
                self.sync_cursors();
            }
            _ => match self.state.current_page {
                Page::Main => self.on_key_main(key),
                Page::ProgramSelection => self.on_key_programs(key),
                // Cleaning owns the keyboard until it finishes.
                Page::Clearing => {}
                Page::Results => self.on_key_results(key),
                Page::Settings => self.on_key_settings(key),
            },
        }
    }

    /// Bindings that work on every page.
    fn on_global(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('?') | KeyCode::F(1) => {
                self.open_changelog();
                true
            }
            KeyCode::Char('G') => {
                appcore::browser::open_in_browser(GITHUB_URL);
                self.toast = Some(Toast::info("Opened the repository in a browser."));
                true
            }
            KeyCode::Char('O') => {
                self.open_release_page();
                true
            }
            // `u` clears the search on the program page, so the dialog uses the
            // shifted key.
            KeyCode::Char('U') if self.update_release.is_some() => {
                self.update_open = !self.update_open;
                // Only on the way in: the dialog closing is a dismissal, and the
                // window frontend does not click on a dismissed notification
                // either.
                if self.update_open {
                    sfx("pop", sounds::pop);
                }
                true
            }
            _ => false,
        }
    }

    // --- main page -------------------------------------------------------

    fn on_key_main(&mut self, key: KeyEvent) {
        let len = self.state.categories.len();
        match key.code {
            // The grid is two columns wide, so a row is `CATEGORY_COLUMNS`
            // categories. Moving by row keeps the focused column, which is what
            // the highlight on the rendered grid shows.
            KeyCode::Up | KeyCode::Char('k') => self.move_category_row(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_category_row(1),
            KeyCode::PageUp => self.move_category_row(-5),
            KeyCode::PageDown => self.move_category_row(5),
            // Tab changes column; there is no wrapping, so the cursor cannot
            // drift into a row that is too short.
            KeyCode::Tab => self.move_category_column(1),
            KeyCode::BackTab => self.move_category_column(-1),
            KeyCode::Home => self.category_cursor = 0,
            KeyCode::End => self.category_cursor = len.saturating_sub(1),
            KeyCode::Char(' ') => self.toggle_category(self.category_cursor),
            // Enter and `→` / `l` open the subcategory overlay — the terminal
            // stand-in for the window frontend's per-category menu button, which
            // is what the `→ N` marker opens for a mouse.
            KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => {
                self.open_category_subs(self.category_cursor)
            }
            // The settings gear in the window title bar.
            KeyCode::Char('s') | KeyCode::Char('S') => {
                sfx("click", sounds::click);
                self.state.current_page = Page::Settings;
            }
            KeyCode::Char('n') | KeyCode::Char('N') => self.advance_to_programs(),
            _ => {}
        }
    }

    /// Ticks or unticks the category at `index` and moves the cursor onto it.
    ///
    /// Taking the index rather than reading the cursor is what lets a click
    /// land on a cell that is not the focused one and still leave the keyboard
    /// pointing at the category it just changed.
    fn toggle_category(&mut self, index: usize) {
        if index >= self.state.categories.len() {
            return;
        }
        self.category_cursor = index;
        let toggle = self.state.toggle_category(index);
        play_toggle(toggle);
        self.toast = Some(Toast::info(match toggle {
            Toggle::On => "Category selected.",
            Toggle::Off => "Category cleared.",
        }));
    }

    /// Moves the focused category by whole rows, preserving the column.
    fn move_category_row(&mut self, delta: isize) {
        let len = self.state.categories.len();
        if len == 0 {
            return;
        }
        let columns = appcore::CATEGORY_COLUMNS;
        let row = (self.category_cursor / columns) as isize;
        let column = self.category_cursor % columns;
        let rows = len.div_ceil(columns) as isize;
        let next_row = (row + delta).clamp(0, rows - 1);
        self.category_cursor = ((next_row as usize) * columns + column).min(len - 1);
    }

    /// Moves the focused category by one cell along the row.
    fn move_category_column(&mut self, delta: isize) {
        move_index(
            &mut self.category_cursor,
            delta,
            self.state.categories.len(),
        );
    }

    /// Opens the subcategory overlay of the category at `index`.
    fn open_category_subs(&mut self, index: usize) {
        let Some(category) = self.state.categories.get(index) else {
            return;
        };
        if category.subs.is_empty() {
            self.toast = Some(Toast::warn("This category has no subcategories."));
            return;
        }
        let popup = Popup {
            kind: PopupKind::Category(index),
            title: format!(" {} subcategories ", category.name),
            list: ListState::default().with_selected(Some(0)),
            items: self.category_sub_items(index),
        };
        self.popup = Some(popup);
        // Opened, not merely refused: a category without subcategories answers
        // with a toast, and that must stay silent.
        sfx("pop", sounds::pop);
    }

    /// `(label, selected)` for every subcategory of the category at `index`,
    /// with the "Uncategorized" pseudo-entry last.
    fn category_sub_items(&self, index: usize) -> Vec<(String, bool)> {
        let Some(category) = self.state.categories.get(index) else {
            return Vec::new();
        };
        let mut items: Vec<(String, bool)> = category
            .subs
            .iter()
            .map(|sub| {
                (
                    self.state.sub_label(&category.name, sub),
                    category.selected.contains(sub),
                )
            })
            .collect();
        if category.has_empty {
            let empty: Arc<str> = Arc::from("");
            items.push((
                self.state.sub_label(&category.name, &empty),
                category.selected.contains(""),
            ));
        }
        items
    }

    fn advance_to_programs(&mut self) {
        if self.state.build_program_list() {
            self.state.current_page = Page::ProgramSelection;
            self.program_cursor = 0;
            sfx("click", sounds::click);
        } else {
            self.toast = Some(Toast::warn("Select at least one category first."));
        }
    }

    // --- program selection page -------------------------------------------

    fn on_key_programs(&mut self, key: KeyEvent) {
        let len = self.state.filtered_programs.len();
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => move_index(&mut self.program_cursor, -1, len),
            KeyCode::Down | KeyCode::Char('j') => move_index(&mut self.program_cursor, 1, len),
            KeyCode::PageUp => move_index(&mut self.program_cursor, -10, len),
            KeyCode::PageDown => move_index(&mut self.program_cursor, 10, len),
            KeyCode::Home => self.program_cursor = 0,
            KeyCode::End => self.program_cursor = len.saturating_sub(1),
            KeyCode::Enter | KeyCode::Char(' ') => self.toggle_program(self.program_cursor),
            // `→` is the per-program category menu; the `→ N` marker on the row
            // is what a mouse uses for the same thing.
            KeyCode::Right | KeyCode::Char('l') => {
                self.open_program_categories(self.program_cursor)
            }
            // `.` is accepted as well: the key labelled `/` on an English layout
            // produces `.` on a Russian one, and a layout table cannot tell the
            // two apart without shadowing the English `.`.
            KeyCode::Char('/') | KeyCode::Char('.') => self.input = InputMode::Editing,
            KeyCode::Char('u') => {
                self.state.set_search("");
                self.program_cursor = 0;
                self.toast = Some(Toast::info("Search cleared."));
            }
            KeyCode::Char('S') => self.start_cleaning(),
            _ => {}
        }
    }

    /// Ticks or unticks the program on `row` of the filtered list.
    ///
    /// The row is in filtered coordinates, which is also what the cursor counts
    /// in, so a click can pass it straight through without looking the program
    /// up again.
    fn toggle_program(&mut self, row: usize) {
        let Some(index) = self.state.filtered_programs.get(row).copied() else {
            return;
        };
        self.program_cursor = row;
        let toggle = self.state.toggle_program(index);
        play_toggle(toggle);
        self.toast = Some(Toast::info(match toggle {
            Toggle::On => "Program selected.",
            Toggle::Off => "Program excluded.",
        }));
    }

    /// Opens the category overlay of the program on `row` of the filtered list.
    fn open_program_categories(&mut self, row: usize) {
        let Some(index) = self.state.filtered_programs.get(row).copied() else {
            return;
        };
        let Some(categories) = self.state.program_categories.get(index) else {
            return;
        };
        if categories.len() <= 1 {
            self.toast = Some(Toast::warn("This program has a single category."));
            return;
        }
        let disabled = &self.state.program_disabled[index];
        let items = categories
            .iter()
            .map(|cat| (cat.to_string(), !disabled.contains(cat)))
            .collect();
        let name = self.state.program_checkboxes[index].1.to_string();
        self.popup = Some(Popup {
            kind: PopupKind::Program(index),
            title: format!(" {name} categories "),
            list: ListState::default().with_selected(Some(0)),
            items,
        });
        sfx("pop", sounds::pop);
    }

    fn start_cleaning(&mut self) {
        sfx("click", sounds::click);
        self.state.start_cleaning();
        self.sync_cursors();
    }

    // --- results page -----------------------------------------------------

    fn on_key_results(&mut self, key: KeyEvent) {
        let len = self.result_count();
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => move_index(&mut self.result_cursor, -1, len),
            KeyCode::Down | KeyCode::Char('j') => move_index(&mut self.result_cursor, 1, len),
            KeyCode::PageUp => move_index(&mut self.result_cursor, -10, len),
            KeyCode::PageDown => move_index(&mut self.result_cursor, 10, len),
            KeyCode::Home => self.result_cursor = 0,
            KeyCode::End => self.result_cursor = len.saturating_sub(1),
            // The table has nothing to change — it is a report, not a list to
            // edit — so `Enter` spends itself on the detail behind the row.
            KeyCode::Enter | KeyCode::Char(' ') => self.toggle_details(self.result_cursor),
            _ => {}
        }
    }

    /// Number of rows in the results table.
    pub fn result_count(&self) -> usize {
        self.state
            .cleared_data
            .as_ref()
            .map_or(0, |data| data.3.len())
    }

    /// The results rows in the order the table draws them: indices into the
    /// run's own list.
    ///
    /// Every row the page counts goes through here — the drawn rows, the row
    /// targets, [`Self::path_count`] and the overlay behind `Enter` — because a
    /// sort is one decision, and re-deriving the order at each of those places
    /// is how one of them ends up a row out and opens another program's paths.
    pub fn result_order(&self) -> Vec<usize> {
        self.state
            .cleared_data
            .as_ref()
            .map_or(Vec::new(), |data| sorted_rows(&data.3, &self.result_sort))
    }

    /// Puts `column` into the sort, or walks it out of it again.
    ///
    /// Clicking a label cycles it through the three states a column of the sort
    /// can be in — out of it, in it as it reads best, in it the other way round —
    /// so a chain of two or three columns is built and taken apart with the same
    /// button, in the same place, and a secondary column can be reversed without
    /// costing the reader the chain around it.
    fn cycle_result_sort(&mut self, column: SortColumn) {
        self.result_sort = self.result_sort.cycled(column);
        sfx("click", sounds::click);
    }

    /// Puts the report back in the order the run produced it.
    ///
    /// Only ever called with something to clear: the wheel's rectangle is not
    /// recorded while the chain is empty, so this cannot swallow a scroll that
    /// was meant for the table.
    fn reset_result_sort(&mut self) {
        self.result_sort = ResultSort::default();
        sfx("click", sounds::click);
    }

    /// True while the pointer is over the report's column labels.
    ///
    /// Kept out of [`HitAreas`] on purpose: the labels' own targets cover the
    /// same row and are registered later, so a lookup would find them first and
    /// the wheel would scroll the table instead of clearing the sort.
    fn over_result_header(&self, at: Position) -> bool {
        self.result_header.is_some_and(|area| area.contains(at))
    }

    /// Opens the deleted-path list of the results row at `row`, or closes it when
    /// that row is already open.
    ///
    /// Both ways in reach the same method — the table's own rows and its mouse
    /// target both go through [`TuiApp::run`] — so a click and `Enter` cannot
    /// end up showing different things. Toggling rather than only opening keeps
    /// the same key working both ways for a user who does not know it is there.
    fn toggle_details(&mut self, row: usize) {
        if self
            .details
            .as_ref()
            .is_some_and(|details| details.row == row)
        {
            self.details = None;
            return;
        }
        // Opening an empty list would be a dead end: the row stays put, and the
        // user is left looking at a box with nothing in it. Counted for `row`,
        // not for the open overlay — which does not exist yet on the way in.
        if self.path_count(row) == 0 {
            self.toast = Some(Toast::warn("This entry reported no deleted paths."));
            return;
        }
        self.details = Some(PathDetails {
            row,
            scroll: 0,
            max_scroll: 0,
        });
        sfx("pop", sounds::pop);
    }

    /// Keys of the deleted-path overlay.
    ///
    /// It is modal over the results table the way the checkable overlay is modal
    /// over a page: `Esc` closes it instead of leaving the page, or the first
    /// dismissal would look like nothing happened.
    fn on_key_details(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => self.details = None,
            KeyCode::Up | KeyCode::Char('k') => self.scroll_details(-1),
            KeyCode::Down | KeyCode::Char('j') => self.scroll_details(1),
            KeyCode::PageUp => self.scroll_details(-DETAILS_PAGE),
            KeyCode::PageDown => self.scroll_details(DETAILS_PAGE),
            KeyCode::Home => self.set_details_scroll(0),
            // Clamped by the render, which is the only place that knows how many
            // lines actually fit.
            KeyCode::End => self.set_details_scroll(usize::MAX),
            _ => {}
        }
    }

    /// Deleted paths of one results row — the lines the overlay lists.
    pub fn path_count(&self, row: usize) -> usize {
        // Through the drawn order, like every other row: `row` is a position in
        // the table, not in the run's list.
        let Some(&index) = self.result_order().get(row) else {
            return 0;
        };
        self.state
            .cleared_data
            .as_ref()
            .and_then(|data| data.3.get(index))
            .map_or(0, |cleared| cleared.paths.len())
    }

    /// Moves the overlay's content by `delta` paths.
    fn scroll_details(&mut self, delta: isize) {
        let Some(row) = self.details.as_ref().map(|details| details.row) else {
            return;
        };
        let len = self.path_count(row);
        if len == 0 {
            return;
        }
        if let Some(details) = &mut self.details {
            details.scroll = (details.scroll as isize + delta).clamp(0, len as isize - 1) as usize;
        }
    }

    fn set_details_scroll(&mut self, scroll: usize) {
        if let Some(details) = &mut self.details {
            details.scroll = scroll;
        }
    }

    // --- search input -----------------------------------------------------

    fn on_key_search(&mut self, key: KeyEvent) {
        match key.code {
            // Enter keeps the query; Esc cancels it. Without the clear, leaving
            // the page and coming back would show an empty list whose only
            // explanation is a stale query in the search field.
            KeyCode::Enter => self.input = InputMode::Normal,
            KeyCode::Esc => {
                self.apply_search(String::new());
                self.input = InputMode::Normal;
            }
            KeyCode::Backspace => {
                let mut query = self.state.search_query_visible.clone();
                query.pop();
                self.apply_search(query);
            }
            KeyCode::Char(c) => {
                let mut query = self.state.search_query_visible.clone();
                query.push(c);
                self.apply_search(query);
            }
            _ => {}
        }
    }

    fn apply_search(&mut self, query: String) {
        self.state.set_search(&query);
        self.program_cursor = 0;
    }

    // --- popup ------------------------------------------------------------

    fn on_key_popup(&mut self, key: KeyEvent) {
        let len = self.popup.as_ref().map_or(0, |popup| popup.items.len());
        match key.code {
            KeyCode::Esc | KeyCode::Enter => self.popup = None,
            KeyCode::Up | KeyCode::Char('k') => self.move_popup(-1, len),
            KeyCode::Down | KeyCode::Char('j') => self.move_popup(1, len),
            KeyCode::Home => {
                if let Some(popup) = &mut self.popup {
                    popup.list.select(Some(0));
                }
            }
            KeyCode::End => {
                if let Some(popup) = &mut self.popup {
                    popup.list.select(Some(len.saturating_sub(1)));
                }
            }
            KeyCode::Char(' ') => {
                let selected = self
                    .popup
                    .as_ref()
                    .and_then(|popup| popup.list.selected())
                    .unwrap_or(0);
                self.toggle_popup_entry(selected);
            }
            _ => {}
        }
    }

    /// Ticks or unticks the entry at `row` of the open overlay.
    ///
    /// The row is what `Space` toggles at the highlighted position, so a click
    /// on an entry and a click on the same entry's checkbox do the same thing.
    fn toggle_popup_entry(&mut self, row: usize) {
        let Some(popup) = &self.popup else {
            return;
        };
        let toggle = match popup.kind {
            PopupKind::Category(index) => {
                // Real subcategories first, then "Uncategorized".
                let subs = self.state.categories[index].subs.len();
                let sub = if row < subs {
                    Arc::clone(&self.state.categories[index].subs[row])
                } else {
                    Arc::from("")
                };
                self.state.toggle_category_sub(index, &sub)
            }
            PopupKind::Program(index) => {
                let category = Arc::clone(&self.state.program_categories[index][row]);
                self.state.toggle_program_category(index, &category)
            }
        };
        play_toggle(toggle);
        // Refresh right away so the new state is visible without waiting for
        // the next frame, and so a reader of `popup` never sees stale entries.
        self.refresh_popup();
    }

    fn move_popup(&mut self, delta: isize, len: usize) {
        if let Some(popup) = &mut self.popup {
            move_list(&mut popup.list, delta, len);
        }
    }

    /// Re-reads the popup entries from the state, so a toggle shows up on the
    /// very next frame.
    pub(super) fn refresh_popup(&mut self) {
        let Some(popup) = &self.popup else {
            return;
        };
        let items = match popup.kind {
            PopupKind::Category(index) => self.category_sub_items(index),
            PopupKind::Program(index) => {
                let (Some(categories), Some(disabled)) = (
                    self.state.program_categories.get(index),
                    self.state.program_disabled.get(index),
                ) else {
                    self.popup = None;
                    return;
                };
                categories
                    .iter()
                    .map(|cat| (cat.to_string(), !disabled.contains(cat)))
                    .collect()
            }
        };
        if items.is_empty() {
            // The underlying entry disappeared (a search narrowed the list).
            self.popup = None;
            return;
        }
        if let Some(popup) = &mut self.popup {
            popup.items = items;
        }
    }

    // --- changelog --------------------------------------------------------

    /// Opens the "What's New" overlay and starts the background fetch.
    pub fn open_changelog(&mut self) {
        if self.changelog_handle.is_none() {
            let current_version = database::get_version().to_string();
            self.changelog_handle = Some(std::thread::spawn(move || {
                database::version::fetch_changelogs(&current_version).unwrap_or_default()
            }));
        }
        // Always start at the top: reopening the overlay should show the newest
        // release, not wherever the user stopped reading last time.
        self.changelog_scroll = 0;
        self.changelog_open = true;
        sfx("pop", sounds::pop);
    }

    fn on_key_changelog(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') | KeyCode::F(1) => {
                self.changelog_open = false;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.changelog_scroll = self.changelog_scroll.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.changelog_scroll += 1;
            }
            KeyCode::PageUp => {
                self.changelog_scroll = self.changelog_scroll.saturating_sub(CHANGELOG_PAGE);
            }
            KeyCode::PageDown => {
                self.changelog_scroll += CHANGELOG_PAGE;
            }
            KeyCode::Home => self.changelog_scroll = 0,
            KeyCode::End => self.changelog_scroll = self.changelog_max_scroll,
            _ => {}
        }
    }

    /// Picks up the changelog once the fetch thread finished.
    pub(super) fn refresh_changelog(&mut self) {
        if !self
            .changelog_handle
            .as_ref()
            .is_some_and(std::thread::JoinHandle::is_finished)
        {
            return;
        }
        if let Some(handle) = self.changelog_handle.take() {
            // The thread already finished, so joining cannot block.
            self.changelog = handle.join().ok();
        }
    }
}

/// Project repository, opened by the `G` binding.
const GITHUB_URL: &str = "https://github.com/Cross-Cleaner/Cross-Cleaner";

/// Moves a plain index cursor by `delta`, wrapping around `len` entries.
fn move_index(cursor: &mut usize, delta: isize, len: usize) {
    if len == 0 {
        return;
    }
    *cursor = ((*cursor as isize + delta).rem_euclid(len as isize)) as usize;
}

/// [`move_index`] for a ratatui `ListState`.
fn move_list(state: &mut ListState, delta: isize, len: usize) {
    if len == 0 {
        return;
    }
    let current = state.selected().unwrap_or(0) as isize;
    state.select(Some((current + delta).rem_euclid(len as isize) as usize));
}
