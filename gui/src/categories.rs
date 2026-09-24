//! Category selection model and the tristate checkbox widget.

use std::sync::Arc;

use eframe::egui;

#[derive(Clone, Debug)]
pub struct CategoryState {
    pub name: std::sync::Arc<str>,
    pub subs: Vec<std::sync::Arc<str>>,
    pub has_empty: bool,
    pub selected: std::collections::HashSet<std::sync::Arc<str>>,
}

impl CategoryState {
    #[allow(dead_code)]
    pub fn is_unchecked(&self) -> bool {
        self.selected.is_empty()
    }

    pub fn is_checked(&self) -> bool {
        if self.subs.is_empty() && !self.has_empty {
            !self.selected.is_empty()
        } else if self.has_empty {
            // fully checked = all subs + empty selected
            self.selected.len() == self.subs.len() + 1 && self.selected.contains("")
        } else {
            !self.subs.is_empty() && self.selected.len() == self.subs.len()
        }
    }
    pub fn is_indeterminate(&self) -> bool {
        if self.subs.is_empty() && !self.has_empty {
            false
        } else {
            !self.selected.is_empty() && !self.is_checked()
        }
    }
}

pub fn effective_sub(_class: &str, sub_category: &str) -> Arc<str> {
    database::structures::intern_arc(sub_category)
}

/// Tristate checkbox with square (filled rect) for indeterminate state.
/// Returns response and whether state changed via click.
pub fn tristate_checkbox(
    ui: &mut egui::Ui,
    checked: bool,
    indeterminate: bool,
    text: &str,
) -> (egui::Response, bool) {
    // Use a mutable dummy bool for Checkbox widget (it will toggle on click)
    let mut dummy = checked;
    // We don't rely on Checkbox's indeterminate painting (hline); we will paint square ourselves.
    // So pass false to avoid double paint, and we handle visual manually.
    let mut response = ui.add(egui::Checkbox::new(&mut dummy, text));
    // If indeterminate, we need to paint square overlay manually
    if indeterminate && ui.is_rect_visible(response.rect) {
        // Calculate icon rect similar to Checkbox impl
        let icon_width = ui.spacing().icon_width;
        let rect = response.rect;
        // icon is at left side, centered vertically
        let icon_rect = egui::Rect::from_min_size(
            egui::pos2(rect.min.x, rect.center().y - icon_width / 2.0),
            egui::vec2(icon_width, icon_width),
        );
        // small inner square (shrink)
        let small_rect = icon_rect.shrink(4.0);
        let visuals = ui.style().interact(&response);
        // Use bg_fill for outer, but for indeterminate we fill inner square with fg color
        // Mimic checkbox bg
        ui.painter()
            .rect_filled(small_rect, 1.0, visuals.fg_stroke.color);
        // Also need to erase the hline that Checkbox didn't draw (we passed false, so no hline)
        // So nothing else
    } else if indeterminate {
        // still need to ensure checkbox appears indeterminate visually even if not visible yet
        // nothing
    }
    let clicked = response.clicked();
    // When clicked, dummy has been toggled (!checked) but for indeterminate we want custom toggle handling outside
    // Return whether clicked
    response.mark_changed();
    (response, clicked)
}
