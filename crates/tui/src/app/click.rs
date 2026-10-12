//! What a click on a rectangle of the last frame does.
//!
//! Every page registers the rectangles it just drew, and a lookup walks them
//! backwards so the topmost registration wins — that is what makes a click land
//! on an overlay drawn over a page rather than on the page underneath it. The
//! vocabulary both halves agree on is here: [`Click`] names the action, never the
//! widget, and [`HitAreas`] holds the rectangles in paint order.

use ratatui::layout::{Position, Rect};

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
pub(crate) struct HitAreas(Vec<(Rect, Click)>);

impl HitAreas {
    /// Forgets the rectangles of the last frame.
    ///
    /// Without this a widget that stopped being drawn — a page change, a
    /// dismissed overlay — would keep answering clicks where it used to be.
    pub(crate) fn clear(&mut self) {
        self.0.clear();
    }

    pub(crate) fn push(&mut self, rect: Rect, click: Click) {
        self.0.push((rect, click));
    }

    /// The action a click at `(column, row)` reaches, if any.
    ///
    /// An empty rectangle contains nothing, so a target that was clipped away
    /// by a cramped terminal cannot swallow the click.
    pub(crate) fn at(&self, column: u16, row: u16) -> Option<Click> {
        let position = Position { x: column, y: row };
        self.0
            .iter()
            .rev()
            .find(|(rect, _)| rect.contains(position))
            .map(|(_, click)| *click)
    }
}
