//! Terminal application state, input handling and rendering entry point.
//!
//! Mirrors [`gui::app::MyApp`] one-to-one: both wrap the same
//! [`appcore::AppState`] and offer the same five pages. The differences follow
//! from the medium — there is no hover, no texture and no native window, so an
//! egui popup becomes a centered overlay, the search field becomes an input
//! mode, and the title bar becomes a header line plus a footer of key hints.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

use appcore::app::{AppState, Page};
use appcore::updater::{self, UpdateState, UpdaterCommand};
use appcore::{Toggle, config};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use database::version::{Changelog, Frontend, NewRelease};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, ListState, Padding, Paragraph, Wrap};

use crate::keymap;
use crate::pages;
use crate::theme::Theme;

/// How long a transient toast stays on screen.
const TOAST_LIFETIME: Duration = Duration::from_secs(6);

/// The four volumes, in the order the window frontend lists them.
const SETTINGS: [&str; 4] = ["Popup sound", "Click sound", "Check sound", "Done sound"];

/// Lines a page-up / page-down moves in the changelog overlay.
const CHANGELOG_PAGE: usize = 10;

/// Diagnostics captured while this process owns the terminal.
///
/// Cleaning runs on worker threads and reports failures through
/// [`database::diag`]. Those must never reach stderr: the alternate screen would
/// be torn in the middle of the run, and with `panic = "abort"` there is no
/// redraw afterwards. They are collected here and surfaced as a toast instead.
static DIAGNOSTICS: std::sync::Mutex<VecDeque<(String, Instant)>> =
    std::sync::Mutex::new(VecDeque::new());

/// How many diagnostics to keep. Bounded because nothing drains this buffer
/// except [`TuiApp::tick`], and a long run must not grow it without limit.
const DIAGNOSTIC_HISTORY: usize = 32;

/// Routes library diagnostics into [`DIAGNOSTICS`].
///
/// Without this every `warn` from a cleaner would be written straight to the
/// terminal this app is drawing into.
pub fn install_diagnostic_sink() {
    database::diag::set_sink(Some(std::sync::Arc::new(push_diagnostic)));
}

/// Removes the sink again, restoring the stderr default.
pub fn remove_diagnostic_sink() {
    database::diag::set_sink(None);
}

/// Takes the diagnostics collected since the last call.
fn take_diagnostics() -> Vec<String> {
    DIAGNOSTICS
        .lock()
        .map(|mut queue| queue.drain(..).map(|(line, _)| line).collect())
        .unwrap_or_default()
}

/// A diagnostic arrived for the buffer, in arrival order.
fn push_diagnostic(line: &str) {
    let Ok(mut queue) = DIAGNOSTICS.lock() else {
        return;
    };
    if queue.len() >= DIAGNOSTIC_HISTORY {
        queue.pop_front();
    }
    queue.push_back((line.to_string(), Instant::now()));
}

/// Where the program search field gets its keystrokes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    /// Keystrokes drive the list.
    Normal,
    /// Keystrokes are appended to the query.
    Editing,
}

/// What the open overlay is editing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupKind {
    /// Subcategories of the category at this index.
    Category(usize),
    /// Categories of the program at this index.
    Program(usize),
}

/// A centered overlay listing checkable entries.
pub struct Popup {
    pub kind: PopupKind,
    pub title: String,
    pub list: ListState,
    /// `(label, selected)` pairs, rebuilt from `AppState` on every render so
    /// they can never go stale.
    pub items: Vec<(String, bool)>,
}

/// A transient status line shown in place of the key hints.
pub struct Toast {
    pub message: String,
    pub style: Style,
    until: Instant,
}

impl Toast {
    fn new(message: impl Into<String>, style: Style) -> Self {
        Self {
            message: message.into(),
            style,
            until: Instant::now() + TOAST_LIFETIME,
        }
    }

    fn info(message: impl Into<String>) -> Self {
        Self::new(message, Style::default().fg(Theme::TEXT))
    }

    fn warn(message: impl Into<String>) -> Self {
        Self::new(message, Style::default().fg(Theme::WARN))
    }
}

/// The terminal frontend's application object.
pub struct TuiApp {
    /// Selection, program list and cleaning job, shared with `gui`.
    pub state: AppState,

    /// Cursor on the category page.
    pub category_cursor: usize,
    /// Cursor on the program page, in *filtered* coordinates.
    pub program_cursor: usize,
    /// Cursor on the results table.
    pub result_cursor: usize,
    /// Cursor on the settings page.
    pub settings_cursor: usize,

    pub input: InputMode,
    pub popup: Option<Popup>,
    pub toast: Option<Toast>,

    /// "What's New" overlay state.
    pub changelog_open: bool,
    /// First visible line of the changelog.
    pub changelog_scroll: usize,
    /// Largest valid `changelog_scroll`, recomputed on every render from the
    /// fetched content and the overlay height.
    changelog_max_scroll: usize,
    changelog: Option<Changelog>,
    changelog_handle: Option<std::thread::JoinHandle<Changelog>>,

    /// Release offered by the background version check.
    pub update_release: Option<NewRelease>,
    update_receiver: Option<std::sync::mpsc::Receiver<Result<Option<NewRelease>, String>>>,
    /// True while the update dialog is on screen.
    pub update_open: bool,
    /// Progress of the self-update, written by the `selfupdate` worker thread.
    pub updater_state: UpdateState,
    /// Channel to the self-update worker. `None` when no worker was started,
    /// which disables the in-app update and falls back to the release page.
    pub updater_tx: Option<std::sync::mpsc::Sender<UpdaterCommand>>,

    /// Set when the user asked to quit.
    pub should_quit: bool,

    /// App start, used to advance the spinner animation and the indeterminate
    /// download bar.
    pub started: Instant,
}

impl TuiApp {
    /// Builds the app around a prepared [`AppState`].
    pub fn new(state: AppState) -> Self {
        let mut app = Self {
            state,
            category_cursor: 0,
            program_cursor: 0,
            result_cursor: 0,
            settings_cursor: 0,
            input: InputMode::Normal,
            popup: None,
            toast: None,
            changelog_open: false,
            changelog_scroll: 0,
            changelog_max_scroll: 0,
            changelog: None,
            changelog_handle: None,
            update_release: None,
            update_receiver: None,
            update_open: false,
            updater_state: updater::new_state(),
            updater_tx: None,
            should_quit: false,
            started: Instant::now(),
        };
        app.start_update_check();
        app
    }

    /// Starts the self-update worker and kicks off the background version check.
    ///
    /// The worker is what lets the terminal app replace its own executable, and
    /// it lives in the `selfupdate` crate so the window frontend shares it.
    pub fn start_self_update(&mut self) {
        if self.updater_tx.is_some() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let state = self.updater_state.clone();
        self.updater_tx = Some(tx);
        std::thread::spawn(move || selfupdate::run(rx, state));
    }

    /// Asks GitHub for the latest release.
    ///
    /// [`Frontend::Tui`] matters: the release ships one binary per frontend, and
    /// resolving the GUI asset would install the window app over the terminal one.
    fn start_update_check(&mut self) {
        let (tx, rx) = std::sync::mpsc::channel();
        self.update_receiver = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(database::version::check_new_version_for(Frontend::Tui));
        });
    }

    // --- frame loop ------------------------------------------------------

    /// Advances time-based state and drains the channels the cleaning job and
    /// the version check write to. Cheap, so it can run on every tick.
    pub fn tick(&mut self) {
        self.state.drain_progress();
        if self.state.poll_result() {
            self.result_cursor = 0;
            self.toast = Some(Toast::info("Cleaning finished."));
        }

        if let Some(receiver) = &self.update_receiver {
            match receiver.try_recv() {
                Ok(Ok(Some(release))) => {
                    self.update_receiver = None;
                    self.toast = Some(Toast::new(
                        format!(
                            "New version v{} available — press O for the release page",
                            release.version
                        ),
                        Style::default().fg(Theme::GOOD),
                    ));
                    self.update_release = Some(release);
                }
                // No newer version, or the check failed: stop polling.
                Ok(Ok(None)) | Ok(Err(_)) | Err(_) => self.update_receiver = None,
            }
        }

        if self
            .toast
            .as_ref()
            .is_some_and(|t| Instant::now() >= t.until)
        {
            self.toast = None;
        }

        // Surface whatever the cleaners reported. Only the newest is shown and
        // the rest are counted, so a run that fails on every entry cannot bury
        // the interface under a wall of toasts.
        let diagnostics = take_diagnostics();
        if let Some(last) = diagnostics.last() {
            let more = diagnostics.len().saturating_sub(1);
            let message = if more == 0 {
                last.clone()
            } else {
                format!("{last} (+{more} more)")
            };
            self.toast = Some(Toast::new(message, Style::default().fg(Theme::WARN)));
        }
    }

    /// Current frame of the spinner animation, for the pages that show one.
    pub fn spinner_frame(&self) -> &'static str {
        let step = (self.started.elapsed().as_millis() / 100) as usize;
        Theme::SPINNER[step % Theme::SPINNER.len()]
    }

    // --- input -----------------------------------------------------------

    /// Feeds one terminal event into the app.
    pub fn on_event(&mut self, event: Event) {
        match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => self.on_key(key),
            // The kitty protocol also reports key releases; the app is driven
            // by presses only, so a release must not double-fire a binding.
            _ => {}
        }
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

    /// Keys of the update dialog.
    ///
    /// The dialog is the only place that can start an install or a relaunch, so
    /// it owns the keyboard while it is up — except for the running stages,
    /// where there is nothing safe to press.
    fn on_key_update(&mut self, key: KeyEvent) {
        let stage = updater::current(&self.updater_state);
        match key.code {
            KeyCode::Esc => {
                if stage.is_running() {
                    return;
                }
                self.update_open = false;
                self.toast = Some(Toast::info("Update postponed."));
            }
            KeyCode::Char('?') | KeyCode::F(1) => self.open_changelog(),
            KeyCode::Char('o') | KeyCode::Char('O') => self.open_release_page(),
            KeyCode::Char('d') | KeyCode::Char('D') => self.start_update(),
            KeyCode::Char('r') | KeyCode::Char('R') => self.confirm_or_retry(),
            _ => {}
        }
    }

    /// Opens the release page of the pending update.
    fn open_release_page(&mut self) {
        match &self.update_release {
            Some(release) => {
                appcore::browser::open_in_browser(&release.url);
                self.toast = Some(Toast::info("Opened the release page."));
            }
            None => self.toast = Some(Toast::warn("No pending update.")),
        }
    }

    /// Downloads the pending release and replaces the running executable.
    fn start_update(&mut self) {
        if self.updater_tx.is_none() {
            self.toast = Some(Toast::warn("Self-update is not available in this build."));
            return;
        }
        let Some(release) = self.update_release.clone() else {
            return;
        };
        if !release.has_asset() {
            self.toast = Some(Toast::warn(
                "This release has no terminal binary for your platform.",
            ));
            return;
        }
        let version = release.version.clone();
        let total = release.asset_size;
        // Publish the opening stage right away so the dialog never shows an idle
        // bar while the worker picks the command up.
        updater::publish(
            &self.updater_state,
            updater::UpdateStage::Downloading {
                version,
                done: 0,
                total,
            },
        );
        if let Err(e) = self
            .updater_tx
            .as_ref()
            .expect("checked above")
            .send(UpdaterCommand::Install(release))
        {
            updater::publish(
                &self.updater_state,
                updater::UpdateStage::Failed {
                    version: "?".to_string(),
                    error: format!("The updater stopped responding: {e}"),
                },
            );
        }
    }

    /// `r` either restarts into the freshly installed version, or retries a
    /// failed download.
    fn confirm_or_retry(&mut self) {
        let stage = updater::current(&self.updater_state);
        if stage.is_running() {
            return;
        }
        let send = |command: UpdaterCommand| -> Result<(), String> {
            let Some(tx) = &self.updater_tx else {
                return Err("Self-update is not available in this build.".to_string());
            };
            tx.send(command)
                .map_err(|e| format!("The updater stopped responding: {e}"))
        };
        let outcome = if stage.is_installed() {
            // Never restart mid-download: the worker owns the executable then.
            send(UpdaterCommand::Restart)
        } else {
            self.start_update();
            Ok(())
        };
        if let Err(error) = outcome {
            self.toast = Some(Toast::warn(error));
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
            KeyCode::Char(' ') => {
                if len == 0 {
                    return;
                }
                let cursor = self.category_cursor;
                self.toast = Some(Toast::info(match self.state.toggle_category(cursor) {
                    Toggle::On => "Category selected.",
                    Toggle::Off => "Category cleared.",
                }));
            }
            // Enter and `→` / `l` open the subcategory overlay — the terminal
            // stand-in for the window frontend's per-category menu button.
            KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => self.open_category_popup(),
            // The settings gear in the window title bar.
            KeyCode::Char('s') | KeyCode::Char('S') => {
                self.state.current_page = Page::Settings;
            }
            KeyCode::Char('n') | KeyCode::Char('N') => self.advance_to_programs(),
            _ => {}
        }
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

    fn open_category_popup(&mut self) {
        let Some(index) = self.state.categories.get(self.category_cursor) else {
            return;
        };
        if index.subs.is_empty() {
            self.toast = Some(Toast::warn("This category has no subcategories."));
            return;
        }
        let popup = Popup {
            kind: PopupKind::Category(self.category_cursor),
            title: format!(" {} subcategories ", index.name),
            list: ListState::default().with_selected(Some(0)),
            items: self.category_sub_items(self.category_cursor),
        };
        self.popup = Some(popup);
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
            KeyCode::Enter | KeyCode::Char(' ') => {
                let Some(index) = self
                    .state
                    .filtered_programs
                    .get(self.program_cursor)
                    .copied()
                else {
                    return;
                };
                self.toast = Some(Toast::info(match self.state.toggle_program(index) {
                    Toggle::On => "Program selected.",
                    Toggle::Off => "Program excluded.",
                }));
            }
            KeyCode::Right | KeyCode::Char('l') => self.open_program_popup(),
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

    fn open_program_popup(&mut self) {
        let Some(index) = self
            .state
            .filtered_programs
            .get(self.program_cursor)
            .copied()
        else {
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
    }

    fn start_cleaning(&mut self) {
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

    // --- settings page ----------------------------------------------------

    fn on_key_settings(&mut self, key: KeyEvent) {
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
        let mut cfg = config::get();
        let target = match slot {
            0 => &mut cfg.sound_volume,
            1 => &mut cfg.click_volume,
            2 => &mut cfg.check_volume,
            _ => &mut cfg.done_volume,
        };
        *target = if delta == 0.0 {
            1.0
        } else {
            (*target + delta).clamp(0.0, 1.0)
        };
        let value = *target;
        config::update(|c| *c = cfg);
        self.toast = Some(Toast::info(format!(
            "{}: {:.0}%",
            SETTINGS[slot],
            value * 100.0
        )));
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
            KeyCode::Char(' ') => self.toggle_popup_entry(),
            _ => {}
        }
    }

    fn toggle_popup_entry(&mut self) {
        let Some(popup) = &self.popup else {
            return;
        };
        let selected = popup.list.selected().unwrap_or(0);
        match popup.kind {
            PopupKind::Category(index) => {
                // Real subcategories first, then "Uncategorized".
                let subs = self.state.categories[index].subs.len();
                let sub = if selected < subs {
                    Arc::clone(&self.state.categories[index].subs[selected])
                } else {
                    Arc::from("")
                };
                self.state.toggle_category_sub(index, &sub);
            }
            PopupKind::Program(index) => {
                let category = Arc::clone(&self.state.program_categories[index][selected]);
                self.state.toggle_program_category(index, &category);
            }
        }
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
    fn refresh_popup(&mut self) {
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
    fn refresh_changelog(&mut self) {
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

    // --- cursors ----------------------------------------------------------

    /// Clamps every cursor after a page change, so a stale index can never
    /// highlight a row that no longer exists.
    fn sync_cursors(&mut self) {
        self.category_cursor = self
            .category_cursor
            .min(self.state.categories.len().saturating_sub(1));
        self.program_cursor = self
            .program_cursor
            .min(self.state.filtered_programs.len().saturating_sub(1));
        self.result_cursor = self
            .result_cursor
            .min(self.result_count().saturating_sub(1));
    }

    // --- rendering --------------------------------------------------------

    /// Draws one frame.
    pub fn render(&mut self, frame: &mut Frame) {
        self.refresh_popup();
        self.refresh_changelog();

        let area = frame.area();
        frame.render_widget(
            Paragraph::new("").style(Style::default().bg(Theme::BG)),
            area,
        );

        // The toast takes the footer's line, so the list never gets pushed
        // under the bottom edge.
        let [header, body, footer] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(if self.toast.is_some() { 2 } else { 1 }),
        ])
        .areas(area);

        self.render_header(frame, header);
        match self.state.current_page {
            Page::Main => pages::main::render(self, frame, body),
            Page::ProgramSelection => pages::program_selection::render(self, frame, body),
            Page::Clearing => pages::clearing::render(self, frame, body),
            Page::Results => pages::results::render(self, frame, body),
            Page::Settings => pages::settings::render(self, frame, body),
        }
        self.render_footer(frame, footer);

        if let Some(popup) = &self.popup {
            render_popup(frame, area, popup);
        }
        pages::update::render(self, frame, area);
        if self.changelog_open {
            self.render_changelog(frame, area);
        }
    }

    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let line = Line::from(vec![
            Span::styled(" Cross Cleaner ", Theme::heading()),
            Span::styled(format!("· {}", self.state.window_title), Theme::dim()),
            Span::raw("  "),
            Span::styled(
                format!("[{}]", self.state.current_page.title()),
                Style::default().fg(Theme::ACCENT),
            ),
        ]);
        frame.render_widget(Paragraph::new(line), area);
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        if let Some(toast) = &self.toast {
            let remaining = toast.until.saturating_duration_since(Instant::now());
            let line = Line::from(vec![
                Span::styled(format!(" {} ", toast.message), toast.style),
                Span::styled(format!("· {}s", remaining.as_secs() + 1), Theme::dim()),
            ]);
            frame.render_widget(Paragraph::new(line), area);
            return;
        }
        frame.render_widget(
            Paragraph::new(Line::from(self.footer_hints())).style(Theme::dim()),
            area,
        );
    }

    /// Context-sensitive key hints, the terminal stand-in for tooltips.
    fn footer_hints(&self) -> Vec<Span<'static>> {
        let page: &[&str] = match self.state.current_page {
            Page::Main => &[
                "↑↓ row",
                "tab column",
                "space select",
                "enter subs",
                "n next",
                "s settings",
            ],
            Page::ProgramSelection => &["↑↓ move", "space select", "→ cats", "/ search", "S start"],
            Page::Clearing => &["cleaning, please wait"],
            Page::Results => &["↑↓ scroll", "esc back"],
            Page::Settings => &["↑↓ pick", "←→ volume", "r reset", "esc back"],
        };
        let mut global: Vec<&str> = vec!["? changelog", "G repo"];
        if self.update_release.is_some() {
            global.push("O release page");
            global.push("U update");
        }
        global.push("q quit");
        let global = global.as_slice();

        let mut spans = Vec::new();
        for group in [page, global] {
            for (index, hint) in group.iter().enumerate() {
                if index > 0 || !spans.is_empty() {
                    spans.push(Span::styled(" · ", Theme::dim()));
                }
                spans.push(Span::styled(*hint, Style::default().fg(Theme::TEXT_DIM)));
            }
        }
        spans
    }

    /// Draws the "What's New" changelog overlay.
    ///
    /// A release note is far longer than the overlay, so the content is scrolled
    /// with an explicit offset and the position is shown in the title; a
    /// `Paragraph` on its own would just clip whatever does not fit.
    fn render_changelog(&mut self, frame: &mut Frame, area: Rect) {
        let width = (area.width * 3 / 4).clamp(40, 84);
        let height = (area.height * 3 / 4).max(10).min(area.height);
        let rect = centered(area, width, height);
        frame.render_widget(Clear, rect);

        let lines = changelog_lines(self.changelog.as_ref());

        let block = Theme::block(" What's New ", true).padding(Padding::uniform(1));
        // Borders and padding eat two rows and one column on each side; a very
        // short terminal can leave nothing at all, hence the saturating math.
        let viewport_height = block.inner(rect).height as usize;

        let max_scroll = lines.len().saturating_sub(viewport_height);
        // Clamp here rather than on the key press: the length is only known once
        // the fetch lands, and it can grow while the overlay is open.
        self.changelog_scroll = self.changelog_scroll.min(max_scroll);
        self.changelog_max_scroll = max_scroll;

        let position = format!(
            " {}/{} · {}esc close ",
            self.changelog_scroll + 1,
            lines.len().max(1),
            if max_scroll > 0 {
                "↑↓ scroll · "
            } else {
                ""
            },
        );
        let block = block.title_bottom(Line::from(position).right_aligned());

        frame.render_widget(
            Paragraph::new(lines)
                .block(block)
                .wrap(Wrap { trim: false })
                .scroll((self.changelog_scroll as u16, 0)),
            rect,
        );
    }
}

/// Project repository, opened by the `G` binding.
const GITHUB_URL: &str = "https://github.com/WinBooster/Cross-Cleaner";

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

/// Draws the centered checkable overlay.
fn render_popup(frame: &mut Frame, area: Rect, popup: &Popup) {
    let width = (area.width * 2 / 3).clamp(30, 64);
    let height = (popup.items.len() as u16 + 4).clamp(6, area.height.saturating_sub(2));
    let rect = centered(area, width, height);
    frame.render_widget(Clear, rect);

    let items: Vec<ratatui::widgets::ListItem> = popup
        .items
        .iter()
        .map(|(label, selected)| {
            let (mark, mark_style) = Theme::checkbox(*selected, false);
            ratatui::widgets::ListItem::new(Line::from(vec![
                Span::styled(format!("{mark} "), mark_style),
                Span::styled(label.clone(), Theme::text()),
            ]))
        })
        .collect();

    let block = Theme::block(&popup.title, true)
        .title_bottom(Line::from(" ↑↓ move · space toggle · esc close ").right_aligned());
    let list = ratatui::widgets::List::new(items)
        .block(block)
        .highlight_style(Theme::selected());

    let mut list_state = popup.list.clone();
    frame.render_stateful_widget(list, rect, &mut list_state);
}

/// Flattens a changelog into display lines.
fn changelog_lines(changelog: Option<&Changelog>) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let bullet = |item: &str| {
        Line::from(vec![
            Span::styled(format!(" {} ", Theme::BULLET), Theme::dim()),
            Span::styled(item.to_string(), Theme::text()),
        ])
    };

    let Some(changelog) = changelog else {
        lines.push(Line::styled("Loading changelog...", Theme::dim()));
        return lines;
    };
    if changelog.groups.is_empty() && changelog.contributors.is_empty() {
        lines.push(Line::styled("No changes found.", Theme::dim()));
        return lines;
    }
    for group in &changelog.groups {
        lines.push(Line::default());
        lines.push(Line::styled(group.title.clone(), Theme::heading()));
        lines.extend(group.items.iter().map(|item| bullet(item)));
    }
    if !changelog.contributors.is_empty() {
        lines.push(Line::default());
        lines.push(Line::styled("Contributors", Theme::heading()));
        lines.extend(changelog.contributors.iter().map(|c| bullet(c)));
    }
    lines
}

/// A `width` x `height` rectangle centered inside `area`.
pub(crate) fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use appcore::updater::UpdateStage;
    use database::cleaner_database::CleanerDatabase;
    #[cfg(windows)]
    use database::registry_database::RegistryDatabase;
    use database::structures::{CleanerData, CleanerFlags};
    use database::version::ChangelogGroup;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn entry(category: &str, program: &str, sub: &str) -> CleanerData {
        CleanerData {
            path: format!("{category}/{program}").into(),
            category: Arc::from(category),
            program: Arc::from(program),
            class: Arc::from("Application"),
            sub_category: Arc::from(sub),
            files_to_remove: vec![],
            directories_to_remove: vec![],
            flags: CleanerFlags::empty(),
        }
    }

    /// Two categories, three programs, one of them in two categories.
    fn sample_app() -> TuiApp {
        let entries = vec![
            entry("Cache", "Chrome", "Browser"),
            entry("Cache", "Chrome", "Code"),
            entry("Cache", "Firefox", "Browser"),
            entry("Logs", "Firefox", "App"),
        ];
        let database = CleanerDatabase::from_vec(entries);
        let custom = Arc::from(Vec::new());
        let state = {
            #[cfg(windows)]
            {
                AppState::from_database(database, RegistryDatabase::from_vec(Vec::new()), custom)
            }
            #[cfg(not(windows))]
            {
                AppState::from_database(database, custom)
            }
        };
        // The version check hits the network; tests must not wait for it.
        let mut app = TuiApp::new(state);
        app.update_receiver = None;
        app
    }

    /// Renders one frame into an offscreen backend and returns it as text.
    fn draw(app: &mut TuiApp, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("test backend");
        terminal.draw(|frame| app.render(frame)).expect("draw");
        let buffer = terminal.backend().buffer();
        (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn press(app: &mut TuiApp, code: KeyCode) {
        app.on_event(Event::Key(KeyEvent::new(code, KeyModifiers::NONE)));
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

    /// Every checkbox glyph a category cell can show.
    const CHECKS: [&str; 3] = ["[x]", "[ ]", "[-]"];

    /// True when the text between the box borders is a category grid row.
    fn is_grid_row(inner: &str) -> bool {
        !checkbox_columns(inner).is_empty()
    }

    /// Columns at which a grid row shows checkboxes, left to right.
    ///
    /// Byte offsets are converted to character columns: the focus marker is a
    /// three-byte glyph, so raw `match_indices` would report it as a shift.
    fn checkbox_columns(inner: &str) -> Vec<usize> {
        CHECKS
            .iter()
            .copied()
            .flat_map(|mark| inner.match_indices(mark))
            .map(|(at, _)| inner[..at].chars().count())
            .collect()
    }

    /// The category grid rows of a rendered frame, borders stripped.
    fn grid_rows(screen: &str) -> Vec<String> {
        screen
            .lines()
            .filter_map(|line| {
                let inner = line.strip_prefix('│')?.strip_suffix('│')?;
                is_grid_row(inner).then(|| inner.to_string())
            })
            .collect()
    }

    /// Rows whose cells differ in shape: one has subcategories and a marker, the
    /// next has none, and the labels have different lengths. That is exactly what
    /// used to push the second column out of alignment.
    fn uneven_grid_app() -> TuiApp {
        let entries = vec![
            // Row 0, left: label plus a `→ N` marker.
            entry("Cache", "App0", "Browser"),
            // Row 0, right: no subcategories at all, so no marker.
            entry("Logs", "App1", ""),
            // Row 1, left: no marker, long label.
            entry("Documentation and manuals", "App2", ""),
            // Row 1, right: marker plus a two-digit count.
            entry("Backups", "App3", "Alpha"),
            entry("Backups", "App3", "Beta"),
            entry("Backups", "App3", "Gamma"),
            entry("Backups", "App3", "Delta"),
            entry("Backups", "App3", "Epsilon"),
            entry("Backups", "App3", "Zeta"),
            entry("Backups", "App3", "Eta"),
            entry("Backups", "App3", "Theta"),
            entry("Backups", "App3", "Iota"),
            entry("Backups", "App3", "Kappa"),
        ];
        let database = CleanerDatabase::from_vec(entries);
        let custom = Arc::from(Vec::new());
        let state = {
            #[cfg(windows)]
            {
                AppState::from_database(database, RegistryDatabase::from_vec(Vec::new()), custom)
            }
            #[cfg(not(windows))]
            {
                AppState::from_database(database, custom)
            }
        };
        let mut app = TuiApp::new(state);
        app.update_receiver = None;
        // Tick two subcategories so the `→ N` marker needs two digits.
        app.state.categories[3].selected.insert(Arc::from("Alpha"));
        app.state.categories[3].selected.insert(Arc::from("Beta"));
        app
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

    /// Two categories that both have subcategories, so both cells show an arrow.
    fn mirrored_grid_app() -> TuiApp {
        let entries = vec![
            entry("Cache", "App0", "Browser"),
            entry("Logs", "App1", "App"),
        ];
        let database = CleanerDatabase::from_vec(entries);
        let custom = Arc::from(Vec::new());
        let state = {
            #[cfg(windows)]
            {
                AppState::from_database(database, RegistryDatabase::from_vec(Vec::new()), custom)
            }
            #[cfg(not(windows))]
            {
                AppState::from_database(database, custom)
            }
        };
        let mut app = TuiApp::new(state);
        app.update_receiver = None;
        app
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

    /// `count` categories, all with two subcategories, so the grid is a real
    /// multi-row rectangle.
    fn many_categories(count: usize) -> TuiApp {
        let entries: Vec<_> = (0..count)
            .flat_map(|i| {
                [
                    entry(&format!("Cat{i}"), &format!("App{i}"), "Alpha"),
                    entry(&format!("Cat{i}"), &format!("App{i}"), "Beta"),
                ]
            })
            .collect();
        let database = CleanerDatabase::from_vec(entries);
        let custom = Arc::from(Vec::new());
        let state = {
            #[cfg(windows)]
            {
                AppState::from_database(database, RegistryDatabase::from_vec(Vec::new()), custom)
            }
            #[cfg(not(windows))]
            {
                AppState::from_database(database, custom)
            }
        };
        let mut app = TuiApp::new(state);
        app.update_receiver = None;
        app
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

    #[test]
    fn search_narrows_the_program_list() {
        let mut app = sample_app();
        press(&mut app, KeyCode::Char(' '));
        press(&mut app, KeyCode::Char('n'));
        press(&mut app, KeyCode::Char('/'));
        assert_eq!(app.input, InputMode::Editing);
        for c in "fox".chars() {
            press(&mut app, KeyCode::Char(c));
        }
        assert_eq!(app.state.search_query_visible, "fox");
        assert_eq!(app.state.filtered_programs.len(), 1);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.input, InputMode::Normal);
        press(&mut app, KeyCode::Char('u'));
        assert_eq!(app.state.filtered_programs.len(), 2);
    }

    #[test]
    fn backspace_rewrites_the_query() {
        let mut app = sample_app();
        press(&mut app, KeyCode::Char(' '));
        press(&mut app, KeyCode::Char('n'));
        press(&mut app, KeyCode::Char('/'));
        press(&mut app, KeyCode::Char('a'));
        press(&mut app, KeyCode::Char('b'));
        press(&mut app, KeyCode::Backspace);
        assert_eq!(app.state.search_query_visible, "a");
    }

    #[test]
    fn clearing_page_renders_a_spinner_before_the_first_tick() {
        let mut app = sample_app();
        app.state.current_page = Page::Clearing;
        let screen = draw(&mut app, 100, 30);
        assert!(screen.contains("preparing"), "{screen}");
        // Once a tick arrives the bar replaces the spinner.
        app.state.total_tasks = 4;
        app.state.current_task = 2;
        app.state.cleaned_bytes = 2048;
        app.state.progress_start = Some(Instant::now());
        let screen = draw(&mut app, 100, 30);
        assert!(screen.contains("2/4"), "{screen}");
        assert!(screen.contains("2.0 KB"), "{screen}");
    }

    #[test]
    fn results_page_renders_the_summary_and_table() {
        let mut app = sample_app();
        app.state.cleared_data = Some((
            4096,
            7,
            2,
            vec![database::structures::Cleared {
                program: "Chrome".to_string(),
                removed_bytes: 4096,
                removed_files: 7,
                removed_directories: 2,
                affected_categories: vec!["Cache".to_string(), "Logs".to_string()],
            }],
        ));
        app.state.current_page = Page::Results;
        let screen = draw(&mut app, 100, 30);
        assert!(screen.contains("Cleaning Results"), "{screen}");
        assert!(screen.contains("4.0 KB"), "{screen}");
        assert!(screen.contains("Chrome"), "{screen}");
        assert!(screen.contains("Cache, Logs"), "{screen}");
    }

    /// Character column at which `needle` starts in `haystack`.
    fn column_of(haystack: &str, needle: &str) -> Option<usize> {
        haystack.find(needle).map(|at| haystack[..at].chars().count())
    }

    /// The column header: it sits inside the table's top border, so the border line
    /// itself carries the labels.
    fn header_line<'a>(screen: &'a str, lines: &[&'a str]) -> &'a str {
        lines
            .iter()
            .find(|line| line.starts_with('╭') && line.contains("Program"))
            .copied()
            .unwrap_or_else(|| panic!("no header row:\n{screen}"))
    }

    #[test]
    fn the_results_header_lines_up_with_its_columns() {
        // Values chosen so each column has a recognisable, differently sized
        // entry: if the header and the rows used different widths, the two
        // would not line up.
        let mut app = sample_app();
        app.state.cleared_data = Some((
            4096,
            7,
            2,
            vec![
                database::structures::Cleared {
                    program: "Chrome".to_string(),
                    removed_bytes: 4096,
                    removed_files: 7,
                    removed_directories: 2,
                    affected_categories: vec!["Cache".to_string()],
                },
                database::structures::Cleared {
                    program: "Visual Studio Code".to_string(),
                    removed_bytes: 3_221_225_472,
                    removed_files: 1234,
                    removed_directories: 567,
                    affected_categories: vec!["Cache".to_string(), "Logs".to_string()],
                },
            ],
        ));
        app.state.current_page = Page::Results;
        let screen = draw(&mut app, 110, 24);
        let lines: Vec<&str> = screen.lines().collect();
        let header = header_line(&screen, &lines);

        let data: Vec<&str> = lines
            .iter()
            .filter(|line| column_of(line, "Chrome").is_some() || column_of(line, "Visual").is_some())
            .copied()
            .collect();
        assert_eq!(data.len(), 2, "expected two rows:\n{screen}");

        // The labels sit inside the top border, and the rule must survive
        // *between* them: a block title is drawn over the border, so any blank
        // inside it erases the line and leaves the labels floating.
        assert!(
            header.starts_with('╭') && header.trim_end().ends_with('╮'),
            "the top border must be drawn around the labels:\n{screen}",
        );
        let border = header.trim_start_matches('╭').trim_end_matches('╮');
        // Only the labels and the single space after each may interrupt the rule.
        let interruptions = border
            .chars()
            .filter(|c| *c != '─' && *c != ' ')
            .count();
        assert!(
            interruptions > 0,
            "the labels must be inside the border:\n{screen}",
        );
        // Between "Dirs" and "Categories" there is a rule, not a blank gap.
        let dirs = header.find("Dirs").expect("Dirs label");
        let categories = header.find("Categories").expect("Categories label");
        let between = &header[dirs + "Dirs".len()..categories];
        assert!(
            between.contains('─'),
            "the rule must show between the labels, found {between:?}:\n{screen}",
        );

        // Every column is left-aligned, so the header and the value must start
        // at the very same column.
        for ((label, short), (_, long)) in [
            (("Program", "Chrome"), ("Program", "Visual Studio Code")),
            (("Size", "4.0 KB"), ("Size", "3.0 GB")),
            (("Files", "7"), ("Files", "1234")),
            (("Dirs", "2"), ("Dirs", "567")),
            (("Categories", "Cache"), ("Categories", "Cache, Logs")),
        ] {
            let header_at = column_of(header, label)
                .unwrap_or_else(|| panic!("header has no {label}:\n{screen}"));
            for (row, needle) in data.iter().zip([short, long]) {
                let row_at = column_of(row, needle)
                    .unwrap_or_else(|| panic!("row has no {needle}:\n{screen}"));
                assert_eq!(
                    header_at, row_at,
                    "{label}: header starts at {header_at}, {needle} at {row_at}\n{screen}",
                );
            }
        }
    }

    #[test]
    fn results_page_without_data_falls_back_gracefully() {
        let mut app = sample_app();
        app.state.current_page = Page::Results;
        let screen = draw(&mut app, 100, 30);
        assert!(screen.contains("No results yet"), "{screen}");
    }

    #[test]
    fn settings_page_renders_all_four_volumes() {
        let mut app = sample_app();
        app.state.current_page = Page::Settings;
        let screen = draw(&mut app, 100, 30);
        for label in SETTINGS {
            assert!(screen.contains(label), "missing {label}:\n{screen}");
        }
    }

    #[test]
    fn settings_volume_steps_and_clamps() {
        let mut app = sample_app();
        app.state.current_page = Page::Settings;
        for _ in 0..20 {
            press(&mut app, KeyCode::Left);
        }
        let cfg = appcore::config::get();
        assert_eq!(cfg.sound_volume, 0.0, "clamps at silence");
        press(&mut app, KeyCode::Right);
        assert!((appcore::config::get().sound_volume - 0.1).abs() < 1e-6);
        press(&mut app, KeyCode::Char('r'));
        assert_eq!(appcore::config::get().sound_volume, 1.0, "resets to full");
        // Put the shared config back so the test does not leak state.
        appcore::config::update(|c| *c = appcore::AppConfig::default());
    }

    #[test]
    fn escape_goes_back_then_quits_from_the_main_page() {
        let mut app = sample_app();
        app.state.current_page = Page::Settings;
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.state.current_page, Page::Main);
        assert!(!app.should_quit, "still on the main page");
        press(&mut app, KeyCode::Esc);
        assert!(app.should_quit);
    }

    #[test]
    fn ctrl_c_always_quits() {
        let mut app = sample_app();
        app.on_event(Event::Key(KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
        )));
        assert!(app.should_quit);
    }

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
    fn toast_replaces_the_key_hints() {
        let mut app = sample_app();
        press(&mut app, KeyCode::Char(' '));
        let screen = draw(&mut app, 100, 30);
        assert!(screen.contains("Category selected"), "{screen}");
        assert!(
            !screen.contains("changelog"),
            "hints are replaced: {screen}"
        );
    }

    /// A changelog long enough to overflow any overlay.
    fn long_changelog() -> Changelog {
        Changelog {
            groups: (0..12)
                .map(|group| ChangelogGroup {
                    title: format!("Group {group}"),
                    items: (0..4)
                        .map(|item| format!("Change {group}.{item} with some detail"))
                        .collect(),
                })
                .collect(),
            contributors: (0..20).map(|c| format!("Contributor {c}")).collect(),
        }
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

    // --- keyboard layouts ------------------------------------------------

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

    // --- diagnostics ----------------------------------------------------

    /// Installs the capture sink for one test and removes it again on drop.
    ///
    /// The sink and the queue behind it are process-global, so this also keeps a
    /// panicking test from leaking the sink into the rest of the suite — which is
    /// why the diagnostics behaviour is covered by a single test rather than
    /// several that would race for the same global.
    struct DiagnosticsGuard;

    impl DiagnosticsGuard {
        fn install() -> Self {
            install_diagnostic_sink();
            Self
        }
    }

    impl Drop for DiagnosticsGuard {
        fn drop(&mut self) {
            remove_diagnostic_sink();
            let _ = take_diagnostics();
        }
    }

    #[test]
    fn diagnostics_are_captured_and_surfaced_as_a_toast() {
        let _guard = DiagnosticsGuard::install();
        let mut app = sample_app();

        // The regression: a cleaner reporting a failure used to `eprintln!`
        // straight onto the alternate screen, tearing the frame apart in the
        // middle of a run.
        database::diag::warn("cleaner: remove_file C:\\cache\\a.tmp: access denied");
        assert!(
            !draw(&mut app, 100, 30).contains("access denied"),
            "a diagnostic must never be part of a frame",
        );

        // The next tick turns it into a toast instead.
        app.tick();
        let screen = draw(&mut app, 100, 30);
        assert!(screen.contains("access denied"), "{screen}");

        // Drained, so an expired toast does not bring it back.
        app.toast = None;
        app.tick();
        assert!(!draw(&mut app, 100, 30).contains("access denied"));

        // A run that fails on every entry must not bury the interface either. The
        // queue is capped, so only `DIAGNOSTIC_HISTORY` messages survive and the
        // toast counts the rest it dropped.
        for i in 0..40 {
            database::diag::warn(format!("cleaner: failure {i}"));
        }
        app.tick();
        let screen = draw(&mut app, 100, 30);
        assert!(screen.contains("failure 39"), "{screen}");
        assert!(
            screen.contains(&format!("(+{} more)", DIAGNOSTIC_HISTORY - 1)),
            "{screen}",
        );

        // `info` is surfaced too — image optimization reports on every file.
        app.toast = None;
        database::diag::info("[image_optimize] WRITTEN saved=4096");
        app.tick();
        let screen = draw(&mut app, 100, 30);
        assert!(screen.contains("[info]"), "{screen}");
        assert!(screen.contains("saved=4096"), "{screen}");
    }

    // --- update dialog ---------------------------------------------------

    fn release() -> NewRelease {
        NewRelease {
            version: "9.9.9".to_string(),
            url: "https://github.com/WinBooster/Cross-Cleaner/releases/tag/v9.9.9".to_string(),
            asset_url: Some("https://example.invalid/tui".to_string()),
            asset_size: Some(4096),
        }
    }

    /// A pending update plus a *live* updater channel.
    ///
    /// The receiver is returned so the caller can keep it alive: a dropped receiver
    /// would make every `send` fail, which is a different code path than the real
    /// one and would hide bugs in the dialog.
    fn app_with_update() -> (TuiApp, std::sync::mpsc::Receiver<UpdaterCommand>) {
        let mut app = sample_app();
        let (tx, rx) = std::sync::mpsc::channel();
        app.updater_tx = Some(tx);
        app.update_release = Some(release());
        (app, rx)
    }

    #[test]
    fn no_update_dialog_without_a_pending_release() {
        let mut app = sample_app();
        assert!(matches!(
            pages::update::prompt(&app),
            pages::update::Prompt::Hidden
        ));
        let screen = draw(&mut app, 100, 30);
        assert!(!screen.contains("download and install"), "{screen}");
    }

    #[test]
    fn the_dialog_is_hidden_until_the_user_asks_for_it() {
        let (mut app, _rx) = app_with_update();
        assert!(matches!(
            pages::update::prompt(&app),
            pages::update::Prompt::Hidden
        ));
        press(&mut app, KeyCode::Char('U'));
        assert!(app.update_open);
        assert!(matches!(
            pages::update::prompt(&app),
            pages::update::Prompt::Offer
        ));
        let screen = draw(&mut app, 100, 30);
        assert!(screen.contains("download and install"), "{screen}");
        assert!(screen.contains("v9.9.9 is available"), "{screen}");
    }

    #[test]
    fn a_release_without_a_binary_offers_only_the_release_page() {
        let (mut app, _rx) = app_with_update();
        app.update_release = Some(NewRelease {
            asset_url: None,
            ..release()
        });
        app.update_open = true;
        let screen = draw(&mut app, 100, 30);
        assert!(
            screen.contains("No terminal binary"),
            "the warning must explain why:\n{screen}",
        );
        assert!(
            !screen.contains("download and install"),
            "a download would fail:\n{screen}",
        );
        assert!(screen.contains("release page"), "{screen}");
    }

    #[test]
    fn d_starts_the_download_and_the_bar_shows_progress() {
        let (mut app, _rx) = app_with_update();
        app.update_open = true;
        press(&mut app, KeyCode::Char('d'));
        // The opening stage is published synchronously, so the dialog never
        // shows an idle bar while the worker picks the command up.
        let stage = updater::current(&app.updater_state);
        assert!(
            matches!(stage, UpdateStage::Downloading { .. }),
            "{stage:?}"
        );
        let screen = draw(&mut app, 100, 30);
        assert!(screen.contains("downloading v9.9.9"), "{screen}");
        assert!(!screen.contains("download and install"), "{screen}");
    }

    #[test]
    fn d_hands_the_release_to_the_worker() {
        let (mut app, rx) = app_with_update();
        app.update_open = true;
        press(&mut app, KeyCode::Char('d'));
        let command = rx.try_recv().expect("the worker was asked to install");
        match command {
            UpdaterCommand::Install(release) => {
                assert_eq!(release.version, "9.9.9");
                assert!(release.has_asset());
            }
            UpdaterCommand::Restart => panic!("expected an install, got a restart"),
        }
    }

    #[test]
    fn a_running_download_cannot_be_dismissed() {
        let (mut app, _rx) = app_with_update();
        app.update_open = true;
        press(&mut app, KeyCode::Char('d'));
        press(&mut app, KeyCode::Esc);
        assert!(
            app.update_open,
            "closing mid-download would strand the user on the old binary",
        );
        // ...and `r` must not relaunch on top of it either.
        press(&mut app, KeyCode::Char('r'));
        assert!(updater::current(&app.updater_state).is_running());
    }

    #[test]
    fn an_installed_update_offers_a_restart_and_later() {
        let (mut app, _rx) = app_with_update();
        app.update_open = true;
        updater::publish(
            &app.updater_state,
            UpdateStage::Installed {
                version: "9.9.9".to_string(),
            },
        );
        let screen = draw(&mut app, 100, 30);
        assert!(screen.contains("Restart to use it?"), "{screen}");
        assert!(screen.contains("restart now"), "{screen}");
        // `Later` dismisses it.
        press(&mut app, KeyCode::Esc);
        assert!(!app.update_open);
    }

    #[test]
    fn r_after_installing_sends_the_restart_command() {
        let (mut app, rx) = app_with_update();
        app.update_open = true;
        updater::publish(
            &app.updater_state,
            UpdateStage::Installed {
                version: "9.9.9".to_string(),
            },
        );
        press(&mut app, KeyCode::Char('r'));
        assert!(
            matches!(rx.try_recv(), Ok(UpdaterCommand::Restart)),
            "expected a restart command",
        );
    }

    #[test]
    fn a_failed_update_offers_a_retry() {
        let (mut app, _rx) = app_with_update();
        app.update_open = true;
        updater::publish(
            &app.updater_state,
            UpdateStage::Failed {
                version: "9.9.9".to_string(),
                error: "connection reset".to_string(),
            },
        );
        let screen = draw(&mut app, 100, 30);
        assert!(screen.contains("connection reset"), "{screen}");
        assert!(screen.contains("retry"), "{screen}");
    }

    #[test]
    fn retry_after_a_failure_starts_the_download_again() {
        let (mut app, _rx) = app_with_update();
        app.update_open = true;
        updater::publish(
            &app.updater_state,
            UpdateStage::Failed {
                version: "9.9.9".to_string(),
                error: "connection reset".to_string(),
            },
        );
        press(&mut app, KeyCode::Char('r'));
        assert!(updater::current(&app.updater_state).is_running());
    }

    #[test]
    fn a_build_without_a_worker_cannot_start_an_install() {
        let mut app = sample_app();
        app.update_release = Some(release());
        app.update_open = true;
        press(&mut app, KeyCode::Char('d'));
        // No worker, so nothing may claim to be downloading.
        assert!(!updater::current(&app.updater_state).is_running());
        let screen = draw(&mut app, 100, 30);
        assert!(screen.contains("Self-update is not available"), "{screen}");
    }

    #[test]
    fn the_update_dialog_renders_on_every_page() {
        let (mut app, _rx) = app_with_update();
        app.update_open = true;
        for page in [
            Page::Main,
            Page::ProgramSelection,
            Page::Clearing,
            Page::Results,
            Page::Settings,
        ] {
            app.state.current_page = page;
            let screen = draw(&mut app, 100, 30);
            assert!(
                screen.contains("download and install"),
                "{page:?}\n{screen}"
            );
        }
    }

    #[test]
    fn the_update_dialog_survives_a_tiny_terminal() {
        let (mut app, _rx) = app_with_update();
        app.update_open = true;
        let _ = draw(&mut app, 20, 8);
        let _ = draw(&mut app, 10, 4);
    }

    #[test]
    fn global_bindings_do_not_steal_page_keys() {
        let mut app = many_categories(6);
        // `j` / `k` move the cursor; only `G`, `O` and `?` are global.
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.category_cursor, 2);
        press(&mut app, KeyCode::Char('k'));
        assert_eq!(app.category_cursor, 0);
        assert!(app.popup.is_none());
        assert!(!app.changelog_open);
    }

    #[test]
    fn every_page_survives_a_tiny_terminal() {
        let mut app = sample_app();
        for page in [
            Page::Main,
            Page::ProgramSelection,
            Page::Clearing,
            Page::Results,
            Page::Settings,
        ] {
            app.state.current_page = page;
            // Must not panic on zero-sized or cramped layouts.
            let _ = draw(&mut app, 20, 6);
            let _ = draw(&mut app, 8, 3);
        }
    }

    #[test]
    fn cursors_stay_inside_the_list_after_navigation() {
        let mut app = sample_app();
        press(&mut app, KeyCode::Char(' '));
        press(&mut app, KeyCode::Char('n'));
        press(&mut app, KeyCode::End);
        assert_eq!(app.program_cursor, 1);
        // A search that matches nothing empties the list and resets the cursor.
        press(&mut app, KeyCode::Char('/'));
        for c in "zzz".chars() {
            press(&mut app, KeyCode::Char(c));
        }
        assert!(app.state.filtered_programs.is_empty());
        assert_eq!(app.program_cursor, 0);
        // Esc cancels the search, so the list comes back.
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.state.search_query_visible, "");
        assert_eq!(app.state.filtered_programs.len(), 2);
        // Leaving the page must not leave a cursor pointing at a missing row.
        press(&mut app, KeyCode::Esc);
        press(&mut app, KeyCode::Char('n'));
        assert!(app.program_cursor < app.state.filtered_programs.len());
    }
}
