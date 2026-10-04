//! Category selection model: the tristate rules and the toggles a frontend
//! performs when the user activates a category or one of its subcategories.

use std::collections::HashSet;
use std::sync::Arc;

/// What a toggle did. Frontends use it to pick the matching click sound and to
/// skip repaints when nothing actually changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toggle {
    /// The item is now selected.
    On,
    /// The item is now (at least partially) deselected.
    Off,
}

impl Toggle {
    /// True when the toggle selected the item.
    pub fn is_on(self) -> bool {
        matches!(self, Toggle::On)
    }
}

/// One category of the cleaner database plus the subcategories the user picked.
#[derive(Clone, Debug)]
pub struct CategoryState {
    /// Category name, e.g. `Cache`.
    pub name: Arc<str>,
    /// Real subcategories, sorted. Empty for leaf categories.
    pub subs: Vec<Arc<str>>,
    /// The category also has entries without a `sub_category` ("Uncategorized").
    pub has_empty: bool,
    /// Currently selected subcategories. The empty string stands for
    /// "Uncategorized".
    pub selected: HashSet<Arc<str>>,
}

impl CategoryState {
    /// True when nothing at all is selected.
    pub fn is_unchecked(&self) -> bool {
        self.selected.is_empty()
    }

    /// True when every subcategory (and "Uncategorized", when present) is
    /// selected.
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

    /// True when something is selected but not everything: the third state a
    /// frontend has to render.
    pub fn is_indeterminate(&self) -> bool {
        if self.subs.is_empty() && !self.has_empty {
            false
        } else {
            !self.selected.is_empty() && !self.is_checked()
        }
    }

    /// Selects every subcategory at once, or clears the whole selection.
    ///
    /// This is what activating the category checkbox itself does.
    pub fn toggle(&mut self) -> Toggle {
        if self.is_checked() || self.is_indeterminate() {
            self.selected.clear();
            Toggle::Off
        } else {
            self.selected = self.subs.iter().cloned().collect();
            if self.has_empty {
                self.selected.insert(Arc::from(""));
            }
            // A category with no subcategories at all is a single unit, so it
            // still needs the empty pseudo-subcategory to mark it as picked.
            if self.subs.is_empty() && !self.has_empty {
                self.selected.insert(Arc::from(""));
            }
            Toggle::On
        }
    }

    /// Selects or clears a single subcategory. `""` addresses the
    /// "Uncategorized" pseudo-subcategory.
    pub fn toggle_sub(&mut self, sub: &Arc<str>) -> Toggle {
        if self.selected.contains(sub) {
            self.selected.remove(sub);
            Toggle::Off
        } else {
            self.selected.insert(Arc::clone(sub));
            Toggle::On
        }
    }
}

/// Normalizes a raw `sub_category` into the key used for selection and
/// grouping. `_class` is kept for signature compatibility with older call
/// sites.
pub fn effective_sub(_class: &str, sub_category: &str) -> Arc<str> {
    database::structures::intern_arc(sub_category)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn category(subs: &[&str], has_empty: bool) -> CategoryState {
        CategoryState {
            name: Arc::from("Cache"),
            subs: subs.iter().copied().map(Arc::from).collect(),
            has_empty,
            selected: HashSet::new(),
        }
    }

    #[test]
    fn tristate_logic() {
        let mut cat = category(&["A", "B", "C"], false);
        assert!(cat.is_unchecked());
        assert!(!cat.is_checked());
        assert!(!cat.is_indeterminate());
        cat.selected.insert(Arc::from("A"));
        assert!(cat.is_indeterminate());
        assert!(!cat.is_checked());
        cat.selected.insert(Arc::from("B"));
        cat.selected.insert(Arc::from("C"));
        assert!(cat.is_checked());
        assert!(!cat.is_indeterminate());
        cat.selected.clear();
        assert!(cat.is_unchecked());
    }

    #[test]
    fn leaf_category_is_checked_when_selected() {
        let mut cat = category(&[], false);
        assert!(cat.toggle().is_on());
        assert!(cat.is_checked());
        assert!(!cat.is_indeterminate());
        assert!(cat.toggle() == Toggle::Off);
        assert!(cat.is_unchecked());
    }

    #[test]
    fn toggle_selects_all_subs_including_uncategorized() {
        let mut cat = category(&["A", "B"], true);
        assert_eq!(cat.toggle(), Toggle::On);
        assert_eq!(cat.selected.len(), 3);
        assert!(cat.is_checked());
        // Fully checked -> a second toggle clears everything.
        assert_eq!(cat.toggle(), Toggle::Off);
        assert!(cat.is_unchecked());
    }

    #[test]
    fn toggle_on_partially_selected_clears() {
        let mut cat = category(&["A", "B"], false);
        cat.selected.insert(Arc::from("A"));
        assert!(cat.is_indeterminate());
        assert_eq!(cat.toggle(), Toggle::Off);
        assert!(cat.is_unchecked());
    }

    #[test]
    fn toggle_sub_flips_single_entry() {
        let mut cat = category(&["A", "B"], true);
        assert_eq!(cat.toggle_sub(&Arc::from("A")), Toggle::On);
        assert!(cat.selected.contains("A"));
        assert_eq!(cat.toggle_sub(&Arc::from("A")), Toggle::Off);
        assert!(!cat.selected.contains("A"));
        // The empty pseudo-subcategory behaves like any other entry.
        assert_eq!(cat.toggle_sub(&Arc::from("")), Toggle::On);
        assert!(cat.is_indeterminate());
        assert_eq!(cat.toggle_sub(&Arc::from("")), Toggle::Off);
    }

    #[test]
    fn effective_sub_interns_empty_string() {
        assert_eq!(&*effective_sub("", ""), "");
        let a = effective_sub("Application", "Browser");
        let b = effective_sub("Application", "Browser");
        assert!(Arc::ptr_eq(&a, &b), "subcategories must be interned");
    }
}
