//! Terminal application state, input handling and rendering entry point.
//!
//! Mirrors [`gui::app::MyApp`] one-to-one: both wrap the same
//! [`appcore::AppState`] and offer the same five pages. The differences follow
//! from the medium — there is no texture and no native window, so an egui popup
//! becomes a centered overlay, the search field becomes an input mode, and the
//! title bar becomes a header line plus a footer of key hints.
//!
//! The mouse is the one input the window frontend gets for free. It is emulated
//! by hit testing: every widget that answers a click registers the rectangle it
//! was drawn into ([`TuiApp::hit`]), and a click is dispatched to the same
//! method its key press would have reached — see [`Click`]. Registering while
//! drawing is what keeps the two in step; a rectangle computed a second time, by
//! hand, would eventually point somewhere the glyph is not.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

use appcore::app::{AppState, Page};
use appcore::sounds;
use appcore::updater::{self, UpdateState, UpdaterCommand};
use appcore::{AppConfig, Toggle, config};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
#[cfg(feature = "self-update")]
use database::version::Frontend;
use database::version::{Changelog, NewRelease};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
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

/// Lines a page-up / page-down moves in the deleted-path overlay.
const DETAILS_PAGE: isize = 10;

/// Rows one wheel notch moves the focused row, the step a terminal wheel
/// reports on every platform.
const WHEEL_STEP: isize = 3;

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

/// The overlay listing which paths one results row deleted, and how much each of
/// them freed.
///
/// It stores the row rather than the paths themselves: the paths live in the
/// finished run, which the results page already owns, and a second copy here
/// would be a list that can go stale.
pub struct PathDetails {
    /// Row of the results table this describes.
    pub row: usize,
    /// First path line on screen.
    pub scroll: usize,
    /// Largest usable `scroll`, recomputed on every render from the number of
    /// paths and the height the overlay got.
    pub max_scroll: usize,
}

/// What a left click on a rectangle of the last frame does.
///
/// Every variant names the action, never the widget: [`TuiApp::run`] sends each
/// one to the method the matching key press calls, so the mouse cannot do
/// something the footer hints do not promise. An index is always a position in
/// the list it was drawn from — the category grid, the filtered program list or
/// the open overlay — which is also where the click target puts the cursor.
///
/// A target is a *left*-click target. [`TuiApp::open_menu`] reinterprets it for
/// the right button, which opens the menu behind it where there is one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Click {
    /// Tick or untick this category, and move the cursor onto it.
    Category(usize),
    /// Open the subcategory overlay of this category.
    CategorySubs(usize),
    /// Tick or untick the program on this row of the filtered list.
    Program(usize),
    /// Open the category overlay of the program on this row.
    ProgramCategories(usize),
    /// Tick or untick this entry of the open overlay.
    PopupItem(usize),
    /// Move the settings cursor onto this volume.
    Settings(usize),
    /// Set a volume to the level under the pointer. `left` is the column its
    /// bar starts at, so the click's own column can be turned into a level.
    Volume { slot: usize, left: u16 },
    /// Put the caret into the search field.
    Search,
    /// The pinned ` Next ` button on the category page.
    Next,
    /// The pinned ` Start Cleaning ` button on the program page.
    StartCleaning,
    /// A row of the results table: open the paths it deleted.
    ResultRow(usize),
    /// A line of the open path overlay. It does nothing by itself, but it has to
    /// exist: without it the backdrop underneath would close the overlay on a
    /// click inside it.
    DetailRow(usize),
    /// The backdrop of the path overlay: close it.
    CloseDetails,
    /// One of the update dialog's rows.
    Update(UpdateAction),
    /// The backdrop of the checkable overlay: close it.
    ClosePopup,
    /// The backdrop of the changelog overlay: close it.
    CloseChangelog,
    /// The backdrop of the update dialog: put it off, the way `Esc` does.
    CloseDialog,
}

/// The choices the update dialog offers.
///
/// Each one is a key *and* a row on screen, so both inputs reach
/// [`TuiApp::run_update`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UpdateAction {
    /// `d`: download and install.
    Install,
    /// `r`: restart into the new version, or retry a failed download.
    Confirm,
    /// `o`: open the release page in a browser.
    ReleasePage,
    /// `?`: open the changelog.
    Changelog,
    /// `Esc`: leave the dialog for now.
    Later,
}

/// The clickable rectangles of the current frame.
///
/// Pages append one per interactive widget while drawing, in paint order, and a
/// lookup walks the list backwards, so the topmost registration wins: that is
/// what makes a click land on an overlay drawn over the page rather than on the
/// page underneath it.
#[derive(Default)]
struct HitAreas(Vec<(Rect, Click)>);

impl HitAreas {
    /// Forgets the rectangles of the last frame.
    ///
    /// Without this a widget that stopped being drawn — a page change, a
    /// dismissed overlay — would keep answering clicks where it used to be.
    fn clear(&mut self) {
        self.0.clear();
    }

    fn push(&mut self, rect: Rect, click: Click) {
        self.0.push((rect, click));
    }

    /// The action a click at `(column, row)` reaches, if any.
    ///
    /// An empty rectangle contains nothing, so a target that was clipped away
    /// by a cramped terminal cannot swallow the click.
    fn at(&self, column: u16, row: u16) -> Option<Click> {
        let position = Position { x: column, y: row };
        self.0
            .iter()
            .rev()
            .find(|(rect, _)| rect.contains(position))
            .map(|(_, click)| *click)
    }
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
    /// The open "which paths did this row delete" overlay.
    pub details: Option<PathDetails>,
    pub toast: Option<Toast>,

    /// Rectangles drawn by the last frame that answer a click, rebuilt by every
    /// [`TuiApp::render`].
    hits: HitAreas,

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
    /// Set once the version check found a release this build can install and the
    /// download was started without waiting for a key press, the way the window
    /// frontend does it.
    auto_update: bool,
    /// Set once the dialog has been raised for a finished automatic update, so a
    /// user who answered it does not get it back on the next tick.
    update_announced: bool,
    /// Footer line for the automatic update while the worker runs.
    ///
    /// Its own field rather than a toast: a toast is transient and gets
    /// overwritten by the next one — a cleaner warning or "Cleaning finished." —
    /// which would make the download flicker in and out. A toast still wins the
    /// line, because it reports something that happened to the user's data.
    update_progress: Option<String>,
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
        // Only the version check below writes to `app`, so without the
        // `self-update` feature it is never mutated after construction.
        #[allow(unused_mut)]
        let mut app = Self {
            state,
            category_cursor: 0,
            program_cursor: 0,
            result_cursor: 0,
            settings_cursor: 0,
            input: InputMode::Normal,
            popup: None,
            details: None,
            toast: None,
            hits: HitAreas::default(),
            changelog_open: false,
            changelog_scroll: 0,
            changelog_max_scroll: 0,
            changelog: None,
            changelog_handle: None,
            update_release: None,
            update_receiver: None,
            update_open: false,
            auto_update: false,
            update_announced: false,
            update_progress: None,
            updater_state: updater::new_state(),
            updater_tx: None,
            should_quit: false,
            started: Instant::now(),
        };
        #[cfg(feature = "self-update")]
        app.start_update_check();
        app
    }

    /// Starts the self-update worker.
    ///
    /// The worker is what lets the terminal app replace its own executable, and
    /// it lives in the `selfupdate` crate so the window frontend shares it.
    /// Nothing to start without the `self-update` feature: `updater_tx` stays
    /// empty, and without the version check there is no update to begin with.
    pub fn start_self_update(&mut self) {
        #[cfg(feature = "self-update")]
        if self.updater_tx.is_none() {
            let (tx, rx) = std::sync::mpsc::channel();
            let state = self.updater_state.clone();
            self.updater_tx = Some(tx);
            std::thread::spawn(move || selfupdate::run(rx, state));
        }
    }

    /// Asks GitHub for the latest release.
    ///
    /// [`Frontend::Tui`] matters: the release ships one binary per frontend, and
    /// resolving the GUI asset would install the window app over the terminal one.
    ///
    /// Without the `self-update` feature the check is not made at all, which is
    /// what keeps the update notification out of a distribution package: no
    /// release is ever found, so there is nothing to announce or offer.
    #[cfg(feature = "self-update")]
    fn start_update_check(&mut self) {
        let (tx, rx) = std::sync::mpsc::channel();
        self.update_receiver = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(database::version::check_new_version_for(Frontend::Tui));
        });
    }

    /// Handles a release the background check found.
    ///
    /// A release this build can install is downloaded straight away, exactly as
    /// the window frontend does it: waiting for a key press made the terminal app
    /// look like it had no updater at all, when it only had one behind a
    /// shortcut. Anything else — no worker, or no binary for this platform —
    /// still points at the release page, which is all it can honestly offer.
    fn accept_release(&mut self, release: NewRelease) {
        let version = release.version.clone();
        self.update_release = Some(release.clone());
        if !self.can_install(&release) {
            database::diag::info(format!(
                "[updater] v{version} found, but this build cannot install it (worker={}, asset={})",
                self.updater_tx.is_some(),
                release.has_asset(),
            ));
            self.toast = Some(Toast::new(
                format!("New version v{version} available — press O for the release page"),
                Style::default().fg(Theme::GOOD),
            ));
            return;
        }
        database::diag::info(format!("[updater] v{version} found, downloading it now"));
        self.start_update();
    }

    /// True when this build can replace its own executable with `release`.
    fn can_install(&self, release: &NewRelease) -> bool {
        self.updater_tx.is_some() && release.has_asset()
    }

    /// Watches the worker while an automatic update is in flight.
    ///
    /// Two things have to happen without a key press: the footer keeps reporting
    /// the download, and the dialog appears as soon as the worker needs an
    /// answer. It waits for that moment rather than opening on the first byte —
    /// a modal that swallows the keyboard for the length of a download would
    /// lock the app out of its own cleaning run.
    fn poll_update(&mut self) {
        if !self.auto_update {
            return;
        }
        let stage = updater::current(&self.updater_state);
        if stage.is_running() {
            // Also covers a retry started from the dialog: the footer has to
            // follow it, not just the first attempt.
            self.update_progress = Some(update_progress_line(&stage));
            return;
        }
        self.update_progress = None;
        if self.update_announced {
            return;
        }
        // Done either way: installed, or failed with something to retry. Raised
        // once, so dismissing it is final.
        self.update_announced = true;
        let was_open = self.update_open;
        self.update_open = true;
        if !was_open {
            sfx("pop", sounds::pop);
        }
    }

    // --- frame loop ------------------------------------------------------

    /// Advances time-based state and drains the channels the cleaning job and
    /// the version check write to. Cheap, so it can run on every tick.
    pub fn tick(&mut self) {
        self.state.drain_progress();
        if self.state.poll_result() {
            self.result_cursor = 0;
            // A new run replaces the report, so an overlay pointing into the old
            // one has nothing left to describe.
            self.details = None;
            self.toast = Some(Toast::info("Cleaning finished."));
            // The end of a run, so the window frontend's done clip belongs here
            // too — otherwise a long clean gives no sign it ever finished.
            sfx("done", sounds::done);
        }

        if let Some(receiver) = &self.update_receiver {
            match receiver.try_recv() {
                Ok(Ok(Some(release))) => {
                    self.update_receiver = None;
                    self.accept_release(release);
                }
                // Nothing newer: the ordinary case, nothing worth a toast.
                Ok(Ok(None)) => self.update_receiver = None,
                // A failed check used to be dropped on the floor, which made
                // "the updater did nothing" indistinguishable from "there was
                // nothing to do" — the one thing a user reporting a silent
                // updater needs to be able to tell apart. GitHub also answers an
                // unauthenticated client with 403 once the hourly rate limit is
                // spent, so this is not a rare path.
                Ok(Err(error)) => {
                    self.update_receiver = None;
                    self.toast = Some(Toast::warn(format!("Update check failed: {error}")));
                }
                // Nothing to read *yet*, which is the normal state for the first
                // second or so: the request is still in flight. Keep polling.
                //
                // This must not be lumped in with `Disconnected`. The tick is 100 ms
                // and the request takes about a second and a half, so treating
                // "not ready" as "not coming" dropped the receiver before the answer
                // could ever arrive — which is why the terminal app looked like it
                // had no updater at all.
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                // The checking thread is gone and sent nothing: it panicked.
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.update_receiver = None;
                    self.toast = Some(Toast::warn("Update check did not finish."));
                }
            }
        }
        self.poll_update();

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
            MouseEventKind::ScrollUp => self.scroll(-1),
            MouseEventKind::ScrollDown => self.scroll(1),
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
    fn scroll(&mut self, notches: isize) {
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

    /// Keys of the update dialog.
    ///
    /// Only the mapping lives here: every key names an [`UpdateAction`] and
    /// hands it to [`TuiApp::run_update`], so a click on the same row cannot end
    /// up doing something else.
    fn on_key_update(&mut self, key: KeyEvent) {
        let action = match key.code {
            KeyCode::Esc => Some(UpdateAction::Later),
            KeyCode::Char('?') | KeyCode::F(1) => Some(UpdateAction::Changelog),
            KeyCode::Char('o') | KeyCode::Char('O') => Some(UpdateAction::ReleasePage),
            KeyCode::Char('d') | KeyCode::Char('D') => Some(UpdateAction::Install),
            KeyCode::Char('r') | KeyCode::Char('R') => Some(UpdateAction::Confirm),
            _ => None,
        };
        if let Some(action) = action {
            self.run_update(action);
        }
    }

    /// Carries out one of the dialog's choices, from a key or from a click.
    ///
    /// The dialog is the only place that can start an install or a relaunch, so
    /// it owns both inputs while it is up — except for the running stages,
    /// where there is nothing safe to press or click. Starting a second download
    /// on top of a running one would interleave two writes to the same
    /// executable, and closing mid-download would strand the user on the old
    /// binary. The changelog and the release page stay open: neither touches the
    /// worker.
    fn run_update(&mut self, action: UpdateAction) {
        let unsafe_while_running = matches!(
            action,
            UpdateAction::Install | UpdateAction::Confirm | UpdateAction::Later
        );
        if unsafe_while_running && updater::current(&self.updater_state).is_running() {
            return;
        }
        match action {
            UpdateAction::Install => self.start_update(),
            UpdateAction::Confirm => self.confirm_or_retry(),
            UpdateAction::ReleasePage => self.open_release_page(),
            UpdateAction::Changelog => self.open_changelog(),
            UpdateAction::Later => {
                self.update_open = false;
                self.toast = Some(Toast::info("Update postponed."));
            }
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
        // From here on this app is the one driving the update, so `tick` keeps
        // the footer on it and raises the dialog when it needs an answer. Set
        // before the send, because a full channel already ends the attempt.
        self.auto_update = true;
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
        self.state
            .cleared_data
            .as_ref()
            .and_then(|data| data.3.get(row))
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
    fn set_volume(&mut self, slot: usize, value: f32) {
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
    ///
    /// Also rebuilds the click targets: every page registers the rectangles it
    /// just drew, in paint order, so what answers a click is exactly what is on
    /// screen. The previous frame's targets go first — a widget that is no
    /// longer drawn must stop answering clicks.
    pub fn render(&mut self, frame: &mut Frame) {
        self.refresh_popup();
        self.refresh_changelog();
        self.hits.clear();

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
        // The page area the overlays are drawn into. The checkable overlay takes
        // the whole screen because it belongs to no one page; the path list takes
        // the results page's own area, because it describes one row of it and has
        // to stay inside the report it belongs to.
        match self.state.current_page {
            Page::Main => pages::main::render(self, frame, body),
            Page::ProgramSelection => pages::program_selection::render(self, frame, body),
            Page::Clearing => pages::clearing::render(self, frame, body),
            // The path overlay is drawn into the same area the report was, below
            // in [`TuiApp::render_details`]: it describes one row of it and has
            // to stay inside the frame the report drew around itself.
            Page::Results => pages::results::render(self, frame, body),
            Page::Settings => pages::settings::render(self, frame, body),
        }
        self.render_footer(frame, footer);

        // The overlay is drawn from `self.popup` while its targets go into
        // `self.hits`, so the borrow is split rather than taken twice. It comes
        // after the page, which is what puts it on top of the page's targets.
        let Self { popup, hits, .. } = self;
        if let Some(popup) = popup {
            render_popup(popup, hits, frame, area);
        }
        // Same arrangement for the path overlay, drawn into the results page's
        // area rather than the screen: its backdrop covers the report only, so a
        // click beside the box inside it closes the list, and the header and
        // footer outside it are never covered.
        if self.details.is_some() && self.state.current_page == Page::Results {
            pages::results::render_details(self, frame, body);
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
        // A toast outranks the download: it reports something that happened to
        // the user's data, while the download is ambient and still visible a
        // moment later.
        if let Some(progress) = &self.update_progress {
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(format!(" {progress} "), Style::default().fg(Theme::ACCENT)),
                    Span::styled("· U for details", Theme::dim()),
                ])),
                area,
            );
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
            // The right button opens the same overlay as `Enter`, so it shares
            // the hint rather than adding a line the footer has no room for —
            // a hint that gets clipped away teaches nothing.
            Page::Main => &[
                "↑↓ row",
                "tab column",
                "space select",
                "enter/right subs",
                "n next",
                "s settings",
            ],
            Page::ProgramSelection => &[
                "↑↓ move",
                "space select",
                "→/right cats",
                "/ search",
                "S start",
            ],
            Page::Clearing => &["cleaning, please wait"],
            Page::Results => &["↑↓ scroll", "enter paths", "esc back"],
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

        // Nothing inside the notes is clickable — they are text — so the whole
        // screen is the dismissal, exactly as `Esc` is.
        self.hits.push(area, Click::CloseChangelog);
    }
}

/// Footer text for an update the worker is still performing.
///
/// Separate from the dialog's own wording: the dialog is modal and spells out
/// the keys, while this line is a status readout that has to fit on one row
/// next to a download already in motion.
fn update_progress_line(stage: &updater::UpdateStage) -> String {
    match stage {
        updater::UpdateStage::Downloading {
            version,
            done,
            total,
        } => format!(
            "updating to v{version} — {}",
            updater::format_progress(*done, *total)
        ),
        updater::UpdateStage::Installing { version } => format!("installing v{version}"),
        // Unreachable through `poll_update`, which only calls this while the
        // stage is running. Kept exhaustive so a new stage cannot be forgotten.
        _ => updater::stage_heading(stage),
    }
}

/// Project repository, opened by the `G` binding.
const GITHUB_URL: &str = "https://github.com/Cross-Cleaner/Cross-Cleaner";

/// The volume stored in slot `slot` of the shared config.
///
/// [`SETTINGS`] and the four fields are kept in step by this and
/// [`volume_slot`]; the settings page reads the same order.
fn volume_of(cfg: &AppConfig, slot: usize) -> f32 {
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

/// Plays the sound for a selection that was just toggled.
///
/// Check and uncheck are separate clips in the window frontend, and a keyboard
/// has no checkbox to fall back on — this is the only cue that tells the two
/// apart, so the mapping is kept in one place instead of at every call site.
fn play_toggle(toggle: Toggle) {
    match toggle {
        Toggle::On => sfx("check", sounds::check),
        Toggle::Off => sfx("uncheck", sounds::uncheck),
    }
}

/// Plays one clip and names it, which is what lets the tests assert the sound
/// map.
///
/// Playing needs an audio device, so a test cannot hear anything; it can only
/// check that an action *reached* a clip. Without a name at the call site the
/// map is invisible to the suite — which is how this app came to ship without a
/// single sound while the window one had five.
fn sfx(clip: &'static str, play: impl FnOnce()) {
    #[cfg(test)]
    PLAYED.with(|played| played.borrow_mut().push(clip));
    #[cfg(not(test))]
    let _ = clip;
    play();
}

// Clips played on this thread, newest last.
//
// Thread-local rather than global: the harness runs tests in parallel and most
// of them press keys, so a shared buffer would make the assertion depend on
// which tests happened to overlap.
#[cfg(test)]
thread_local! {
    static PLAYED: std::cell::RefCell<Vec<&'static str>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Empties and returns the clips played on this thread so far.
#[cfg(test)]
fn take_played() -> Vec<&'static str> {
    PLAYED.with(|played| played.take())
}

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

/// Draws the centered checkable overlay, and registers what it answers.
///
/// An entry toggles its own checkbox; a click anywhere else closes the overlay,
/// the way `Esc` does. The backdrop goes in before the entries so the entries
/// are found first — a lookup takes the topmost target.
fn render_popup(popup: &Popup, hits: &mut HitAreas, frame: &mut Frame, area: Rect) {
    let width = (area.width * 2 / 3).clamp(30, 64);
    // `clamp` panics when its bounds are inverted, which a terminal shorter than
    // the minimum would do — the overlay then fills the screen rather than
    // taking the frame down.
    let height = (popup.items.len() as u16 + 4)
        .max(6)
        .min(area.height.saturating_sub(2).max(6));
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
    // Taken before the block is handed to the list, which consumes it.
    let inner = block.inner(rect);
    let list = ratatui::widgets::List::new(items)
        .block(block)
        .highlight_style(Theme::selected());

    let mut list_state = popup.list;
    frame.render_stateful_widget(list, rect, &mut list_state);

    // The backdrop covers the whole screen, so a click outside the box closes the
    // overlay instead of reaching the page underneath. Registered first, so the
    // entries below win where they overlap.
    hits.push(area, Click::ClosePopup);
    // The list is scrolled to the selected entry, and `List` writes the offset it
    // ended up with back into the state — so the rows on screen are only known
    // after drawing, not before.
    let offset = list_state.offset();
    for row in 0..inner.height {
        hits.push(
            pages::row_of(inner, row, inner.width),
            Click::PopupItem(offset + row as usize),
        );
    }
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

    /// Clicks at a screen position with `button` held down.
    fn click_with(app: &mut TuiApp, button: MouseButton, column: u16, row: u16) {
        app.on_event(Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(button),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }));
    }

    /// Left-clicks at a screen position.
    fn click(app: &mut TuiApp, column: u16, row: u16) {
        click_with(app, MouseButton::Left, column, row);
    }

    /// Right-clicks at a screen position: the terminal's context menu.
    fn right_click(app: &mut TuiApp, column: u16, row: u16) {
        click_with(app, MouseButton::Right, column, row);
    }

    /// Right-clicks the first `needle` found on the current frame.
    fn right_click_on(app: &mut TuiApp, width: u16, height: u16, needle: &str) {
        let screen = draw(app, width, height);
        let (column, row) = position_of(&screen, needle);
        right_click(app, column as u16, row as u16);
    }

    /// Turns the wheel at a screen position, positive for a downward notch.
    fn wheel(app: &mut TuiApp, notches: isize) {
        let kind = if notches > 0 {
            MouseEventKind::ScrollDown
        } else {
            MouseEventKind::ScrollUp
        };
        for _ in 0..notches.abs() {
            app.on_event(Event::Mouse(MouseEvent {
                kind,
                column: 0,
                row: 0,
                modifiers: KeyModifiers::NONE,
            }));
        }
    }

    /// Draws a frame and clicks the first `needle` found on it.
    ///
    /// Goes through the rendered text rather than through the layout constants,
    /// so the test fails if a target and the glyph it belongs to drift apart —
    /// which is the failure mode hit testing has, and the reason the targets are
    /// registered where the widgets are drawn.
    fn click_on(app: &mut TuiApp, width: u16, height: u16, needle: &str) {
        let screen = draw(app, width, height);
        let (column, row) = position_of(&screen, needle);
        click(app, column as u16, row as u16);
    }

    /// Screen position of the first `needle`, as `(column, row)`.
    fn position_of(screen: &str, needle: &str) -> (usize, usize) {
        screen
            .lines()
            .enumerate()
            .find_map(|(row, line)| column_of(line, needle).map(|column| (column, row)))
            .unwrap_or_else(|| panic!("no {needle:?} on screen:\n{screen}"))
    }

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

    /// One category holding a single program, so the program list has a row whose
    /// program is in exactly one category — the case the marker names instead of
    /// counting. `sample_app` cannot do this: Chrome is in two subcategories and
    /// Firefox spans two categories.
    fn single_category_program() -> TuiApp {
        let entries = vec![
            entry("Cache", "Solo", "Browser"),
            entry("Logs", "Chrome", "App"),
            entry("Logs", "Chrome", "Core"),
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
        app.state.toggle_category(0);
        assert!(
            app.state.build_program_list(),
            "the program list must have been built",
        );
        app.state.current_page = Page::ProgramSelection;
        app
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

    /// Every action the window frontend gives a sound must reach the same clip
    /// here.
    ///
    /// The terminal app shipped silent while `gui` played five clips, so the map
    /// is pinned rather than left to whoever reads the code: a binding that
    /// quietly stops making noise is a defect the user has to notice, not one a
    /// test stumbles over.
    #[test]
    fn actions_play_the_same_clips_as_the_window_frontend() {
        // This one runs a real cleaning job, so it reports through the same
        // process-global diagnostics queue the test below owns. Take turns, and
        // capture the output so it never reaches stderr either.
        let _serial = sequential();
        let _guard = DiagnosticsGuard::install();
        let mut app = sample_app();
        take_played();

        // Category checkbox: selecting and clearing are two different clips.
        press(&mut app, KeyCode::Char(' '));
        press(&mut app, KeyCode::Char(' '));
        assert_eq!(take_played(), ["check", "uncheck"], "category checkbox");

        // Both categories, so Firefox really spans two of them — that is what
        // makes the per-program popup reachable at all. The grid is two columns
        // wide, so Tab is what reaches the second category; Down would stay on
        // the only row.
        press(&mut app, KeyCode::Char(' '));
        press(&mut app, KeyCode::Tab);
        press(&mut app, KeyCode::Char(' '));
        // `n` is the window frontend's Next button.
        press(&mut app, KeyCode::Char('n'));
        assert_eq!(app.state.current_page, Page::ProgramSelection);
        assert_eq!(
            take_played(),
            ["check", "check", "click"],
            "both categories, then Next",
        );

        // Program checkbox. The list arrives pre-selected, so this clears and
        // re-selects rather than the other way round.
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Enter);
        assert_eq!(take_played(), ["uncheck", "check"], "program checkbox");

        // `→` is the per-program category menu; the sample Chrome entry is in a
        // single category, so the refusal must stay silent.
        press(&mut app, KeyCode::Right);
        assert!(app.popup.is_none(), "Chrome has one category");
        assert_eq!(take_played(), Vec::<&str>::new(), "refused popup");

        // Firefox is in two, so this one opens.
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Right);
        assert!(app.popup.is_some());
        assert_eq!(take_played(), ["pop"], "program category menu");

        // The popup starts with every category enabled, so the first space
        // excludes one rather than adding it.
        press(&mut app, KeyCode::Char(' '));
        press(&mut app, KeyCode::Char(' '));
        assert_eq!(take_played(), ["uncheck", "check"], "popup entry");
        // Esc is navigation, not a button: dismissing the overlay is silent.
        press(&mut app, KeyCode::Esc);
        assert!(app.popup.is_none());
        assert_eq!(take_played(), Vec::<&str>::new(), "escape");

        // `S` starts the run. It spawns onto a tokio runtime, and the sample
        // entries have nothing to remove, so no real file is touched.
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("runtime");
        runtime.block_on(async {
            press(&mut app, KeyCode::Char('S'));
        });
        assert_eq!(take_played(), ["click"], "Start");

        // The job runs on the runtime's own threads, so the result arrives on
        // whichever one is free: `tick` until it lands.
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
        assert_eq!(take_played(), ["done"], "the finishing clip");
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
            Arc::from(vec![database::structures::Cleared {
                program: "Chrome".to_string(),
                removed_bytes: 4096,
                removed_files: 7,
                removed_directories: 2,
                affected_categories: vec!["Cache".to_string(), "Logs".to_string()],
                paths: Vec::new(),
                paths_omitted: 0,
            }]),
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
        haystack
            .find(needle)
            .map(|at| haystack[..at].chars().count())
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
            Arc::from(vec![
                database::structures::Cleared {
                    program: "Chrome".to_string(),
                    removed_bytes: 4096,
                    removed_files: 7,
                    removed_directories: 2,
                    affected_categories: vec!["Cache".to_string()],
                    paths: Vec::new(),
                    paths_omitted: 0,
                },
                database::structures::Cleared {
                    program: "Visual Studio Code".to_string(),
                    removed_bytes: 3_221_225_472,
                    removed_files: 1234,
                    removed_directories: 567,
                    affected_categories: vec!["Cache".to_string(), "Logs".to_string()],
                    paths: Vec::new(),
                    paths_omitted: 0,
                },
            ]),
        ));
        app.state.current_page = Page::Results;
        let screen = draw(&mut app, 110, 24);
        let lines: Vec<&str> = screen.lines().collect();
        let header = header_line(&screen, &lines);

        let data: Vec<&str> = lines
            .iter()
            .filter(|line| {
                column_of(line, "Chrome").is_some() || column_of(line, "Visual").is_some()
            })
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
        let interruptions = border.chars().filter(|c| *c != '─' && *c != ' ').count();
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

    // --- deleted paths ---------------------------------------------------

    /// One row of the results table, with the paths behind its numbers.
    fn cleared(program: &str, bytes: u64, paths: &[(&str, u64)]) -> database::structures::Cleared {
        use database::structures::{Cleared, ClearedPath};
        Cleared {
            program: program.to_string(),
            removed_bytes: bytes,
            removed_files: paths.len() as u64,
            removed_directories: 0,
            affected_categories: vec!["Cache".to_string()],
            paths: paths
                .iter()
                .map(|(path, bytes)| ClearedPath {
                    path: (*path).into(),
                    removed_bytes: *bytes,
                    removed_files: 1,
                    removed_directories: 0,
                })
                .collect(),
            paths_omitted: 0,
        }
    }

    /// A finished run: Chrome deleted three paths, Firefox one, and a third
    /// program reported a total without saying which path it came from.
    fn results_app() -> TuiApp {
        let mut app = sample_app();
        app.state.cleared_data = Some((
            6144,
            4,
            0,
            Arc::from(vec![
                cleared(
                    "Chrome",
                    4096,
                    &[
                        ("C:/cache/a.tmp", 2048),
                        ("C:/cache/b.tmp", 1024),
                        ("C:/cache/c.tmp", 1024),
                    ],
                ),
                cleared("Firefox", 2048, &[("C:/ff/x.dat", 2048)]),
                cleared("ShareX", 0, &[]),
            ]),
        ));
        app.state.current_page = Page::Results;
        app
    }

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
        app.state.cleared_data =
            Some((780, 40, 0, Arc::from(vec![cleared("Chrome", 780, &paths)])));
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
        let margin = pages::results::DETAIL_MARGIN as usize;
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
            30 - 2 - pages::results::DETAIL_MARGIN_BOTTOM as usize,
            "bottom margin, inside the report:\n{with}",
        );
        assert!(
            pages::results::DETAIL_MARGIN_BOTTOM < pages::results::DETAIL_MARGIN,
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
        let _serial = config_lock();
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
        appcore::config::update(|c| *c = AppConfig::default());
    }

    /// Serialises the tests that read or write the sound levels.
    ///
    /// The volumes live in one process-global config, and the harness runs tests
    /// in parallel: without this a test that clamps `sound_volume` to zero would
    /// silently break another's "full volume" assertion, and the failure would
    /// land on whichever test happened to overlap rather than on the cause.
    static CONFIG: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Holds [`CONFIG`] and restores the default volumes on the way out.
    ///
    /// The restore is the point of returning a guard rather than a lock: the
    /// next test in line must not inherit the level this one left behind, and a
    /// panicking test has to restore it too.
    fn config_lock() -> ConfigGuard {
        // The field is never read: the lock exists for its `Drop`, and holding it
        // is what keeps the tests apart.
        let guard = CONFIG.lock().unwrap_or_else(|e| e.into_inner());
        appcore::config::update(|c| *c = AppConfig::default());
        ConfigGuard(guard)
    }

    struct ConfigGuard(#[expect(dead_code)] std::sync::MutexGuard<'static, ()>);

    impl Drop for ConfigGuard {
        fn drop(&mut self) {
            appcore::config::update(|c| *c = AppConfig::default());
        }
    }

    // --- mouse -----------------------------------------------------------

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
        let bar_width = pages::settings::BAR_WIDTH;
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

    /// Serialises the tests that drive a real cleaning run.
    ///
    /// The harness runs tests in parallel, and a cleaning run reports through the
    /// process-global diagnostic queue that the test below asserts on. Two runs at
    /// once would put one test's messages into the other's assertions, so they
    /// take turns.
    static SEQUENTIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Holds [`SEQUENTIAL`] for the duration of a test.
    ///
    /// A poisoned lock only means some earlier test panicked; the queue is drained
    /// on the way out either way, so there is nothing here worth failing over.
    fn sequential() -> std::sync::MutexGuard<'static, ()> {
        SEQUENTIAL.lock().unwrap_or_else(|e| e.into_inner())
    }

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
        let _serial = sequential();
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
            url: "https://github.com/Cross-Optimizations/Cross-Cleaner/releases/tag/v9.9.9"
                .to_string(),
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

    /// A version check that has already found `found`, so the first `tick` picks
    /// it up. The check sender is dropped on return, which is exactly what the
    /// real background thread does after reporting.
    ///
    /// The updater receiver comes back too: an automatic download has to land
    /// somewhere, and it has to be a live one or the send fails instead.
    fn app_with_check(
        found: Option<NewRelease>,
    ) -> (TuiApp, std::sync::mpsc::Receiver<UpdaterCommand>) {
        let (mut app, rx) = app_with_update();
        let (tx, check) = std::sync::mpsc::channel();
        tx.send(Ok(found)).expect("the receiver is alive");
        app.update_receiver = Some(check);
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

    /// A release this build can install must download itself, the way the window
    /// frontend does — waiting for a key press is what made the terminal app look
    /// like it had no updater at all.
    #[test]
    fn a_found_release_starts_downloading_without_a_keypress() {
        let (mut app, rx) = app_with_check(Some(release()));
        app.tick();

        match rx.try_recv().expect("the worker was asked to install") {
            UpdaterCommand::Install(release) => assert_eq!(release.version, "9.9.9"),
            UpdaterCommand::Restart => panic!("expected an install, got a restart"),
        }
        assert!(
            matches!(
                updater::current(&app.updater_state),
                UpdateStage::Downloading { .. }
            ),
            "the opening stage is published synchronously",
        );
        // The footer reports it, but the dialog stays shut: a modal that ate the
        // keyboard for the length of a download would lock the user out of the
        // app they were cleaning with.
        assert!(!app.update_open, "not modal while it downloads");
        let screen = draw(&mut app, 100, 30);
        assert!(screen.contains("updating to v9.9.9"), "{screen}");
    }

    /// The dialog waits for the one moment the user actually has to decide, then
    /// stays down once answered.
    #[test]
    fn the_dialog_opens_when_the_worker_needs_an_answer_and_not_before() {
        let (mut app, _rx) = app_with_check(Some(release()));
        app.tick();
        assert!(!app.update_open, "the first byte needs no decision");

        updater::publish(
            &app.updater_state,
            UpdateStage::Installed {
                version: "9.9.9".to_string(),
            },
        );
        app.tick();
        assert!(app.update_open, "restarting is the user's call");
        assert!(
            app.update_progress.is_none(),
            "the footer hands its line over to the dialog",
        );
        let screen = draw(&mut app, 100, 30);
        assert!(screen.contains("Restart to use it"), "{screen}");

        press(&mut app, KeyCode::Esc);
        assert!(!app.update_open);
        app.tick();
        assert!(
            !app.update_open,
            "an update the user answered must not come back",
        );
    }

    /// A failed automatic download is reported the same way: not as a toast that
    /// fades, but as the dialog offering a retry.
    #[test]
    fn a_failed_automatic_download_asks_for_a_decision_too() {
        let (mut app, _rx) = app_with_check(Some(release()));
        app.tick();
        updater::publish(
            &app.updater_state,
            UpdateStage::Failed {
                version: "9.9.9".to_string(),
                error: "connection reset".to_string(),
            },
        );
        app.tick();
        assert!(app.update_open);
        let screen = draw(&mut app, 100, 30);
        assert!(screen.contains("connection reset"), "{screen}");
        assert!(screen.contains("retry"), "{screen}");
    }

    /// Nothing to install means nothing to start: the release page is all this
    /// build can honestly offer, and it has to say so.
    /// A retry started from the dialog is still this app's own update, so the
    /// footer follows the second attempt instead of freezing on the first.
    #[test]
    fn a_retry_keeps_reporting_progress_in_the_footer() {
        let (mut app, _rx) = app_with_check(Some(release()));
        app.tick();
        updater::publish(
            &app.updater_state,
            UpdateStage::Failed {
                version: "9.9.9".to_string(),
                error: "connection reset".to_string(),
            },
        );
        app.tick();
        assert!(app.update_open);

        press(&mut app, KeyCode::Char('r'));
        assert!(
            updater::current(&app.updater_state).is_running(),
            "`r` on a failure starts a fresh download",
        );
        app.tick();
        assert!(
            app.update_progress
                .as_deref()
                .is_some_and(|line| line.contains("updating to")),
            "the footer follows the retry: {:?}",
            app.update_progress,
        );

        updater::publish(
            &app.updater_state,
            UpdateStage::Installed {
                version: "9.9.9".to_string(),
            },
        );
        app.tick();
        assert!(app.update_progress.is_none(), "the download is over");
        let screen = draw(&mut app, 100, 30);
        assert!(screen.contains("Restart to use it"), "{screen}");
    }

    /// The regression behind "the updater never did anything": the tick is far
    /// faster than the request, so the first ticks find an empty channel. Treating
    /// "not answered yet" as "will never answer" threw the receiver away ~100 ms in,
    /// long before the ~1.5 s response could arrive — and the release was then never
    /// seen at all.
    #[test]
    fn a_check_that_is_still_in_flight_keeps_being_polled() {
        let mut app = sample_app();
        // A live sender that has not reported yet, which is exactly what the first
        // ticks see.
        let (tx, check) = std::sync::mpsc::channel();
        app.update_receiver = Some(check);

        for _ in 0..5 {
            app.tick();
        }
        assert!(
            app.update_receiver.is_some(),
            "an unanswered check must still be polled",
        );
        assert!(app.toast.is_none(), "still in flight is not a failure");

        // And when it does answer, the answer is still picked up.
        tx.send(Ok(Some(release()))).expect("receiver alive");
        app.tick();
        assert!(app.update_receiver.is_none(), "polling stops once answered");
        // `sample_app` has no self-update worker, so this build takes the
        // release-page path — the point is that the release was seen at all.
        assert_eq!(
            app.update_release.as_ref().map(|r| r.version.as_str()),
            Some("9.9.9"),
            "the answer that arrived late must still be acted on",
        );
    }

    /// A check that failed must say so. GitHub rate-limits unauthenticated
    /// clients, and a silently dropped error is indistinguishable from "you are
    /// already on the newest version".
    #[test]
    fn a_failed_check_is_reported_instead_of_being_dropped() {
        let mut app = sample_app();
        let (tx, check) = std::sync::mpsc::channel();
        tx.send(Err(
            "Failed to request latest release: status 403".to_string()
        ))
        .expect("receiver alive");
        app.update_receiver = Some(check);
        app.tick();
        let screen = draw(&mut app, 100, 30);
        assert!(screen.contains("Update check failed"), "{screen}");
        assert!(screen.contains("403"), "the reason must survive: {screen}");
        assert!(!app.auto_update, "nothing was started");

        // Polling stops, so the toast is not refreshed every tick.
        app.tick();
        assert!(app.update_receiver.is_none(), "not polled again");
    }

    #[test]
    fn a_check_that_found_nothing_stays_quiet() {
        let mut app = sample_app();
        let (tx, check) = std::sync::mpsc::channel();
        tx.send(Ok(None)).expect("receiver alive");
        app.update_receiver = Some(check);
        app.tick();
        assert!(app.toast.is_none(), "already up to date is not news");
        assert!(app.update_receiver.is_none(), "not polled again");
    }

    #[test]
    fn a_release_this_build_cannot_install_only_points_at_the_page() {
        let (mut app, rx) = app_with_check(Some(NewRelease {
            asset_url: None,
            ..release()
        }));
        app.tick();
        assert!(
            rx.try_recv().is_err(),
            "a release with no binary must not be handed to the worker",
        );
        assert!(!app.auto_update, "nothing was started");
        assert!(!app.update_open);
        assert!(app.update_progress.is_none());
        let screen = draw(&mut app, 100, 30);
        assert!(screen.contains("press O for the release page"), "{screen}");
    }

    #[test]
    fn a_build_without_a_worker_only_points_at_the_page() {
        let mut app = sample_app();
        let (tx, check) = std::sync::mpsc::channel();
        tx.send(Ok(Some(release()))).expect("receiver alive");
        app.update_receiver = Some(check);
        app.tick();
        assert!(!app.auto_update, "no worker, so no download");
        assert!(!app.update_open);
        let screen = draw(&mut app, 100, 30);
        assert!(screen.contains("press O for the release page"), "{screen}");
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
        let (mut app, rx) = app_with_update();
        app.update_open = true;
        press(&mut app, KeyCode::Char('d'));
        assert!(
            rx.try_recv().is_ok(),
            "the first download is the only one the worker may get",
        );
        press(&mut app, KeyCode::Esc);
        assert!(
            app.update_open,
            "closing mid-download would strand the user on the old binary",
        );
        // ...`r` must not relaunch on top of it either, and `d` must not start a
        // second download writing to the same executable.
        press(&mut app, KeyCode::Char('r'));
        assert!(updater::current(&app.updater_state).is_running());
        press(&mut app, KeyCode::Char('d'));
        assert!(
            rx.try_recv().is_err(),
            "only the first install may reach the worker",
        );
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
