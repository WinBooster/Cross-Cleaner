//! Terminal reporting for the command line frontend.
//!
//! Splits by what a reader needs next: the categories that exist, the programs
//! a selection resolves to, and the plan a `clean` would execute. All three come
//! out of `appcore`'s state, so a name printed here is a name the selection
//! accepts.

use appcore::AppState;

use crate::args::OutputArgs;
use crate::plan;
use crate::term::{Ui, count};

/// Prints the categories with their subcategories and entry counts.
pub fn categories(state: &AppState, ui: &Ui, output: &OutputArgs) {
    let views = plan::categories(&state.database, &state.custom_database);

    if output.json {
        ui.print_json(
            &serde_json::json!({
                "categories": views.iter().map(|view| serde_json::json!({
                    "category": view.name,
                    "entries": view.entries,
                    "subcategories": view.subs.iter().map(|sub| serde_json::json!({
                        "name": sub.name,
                        "entries": sub.entries,
                    })).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
            })
            .to_string(),
        );
        return;
    }
    if output.quiet {
        return;
    }

    ui.line(&ui.bold(&format!("{} categories", views.len())));
    for view in &views {
        // The count is the number of database paths, which is what a category
        // "has". How much space that is on disk is only known after cleaning it,
        // so a count is the most a listing can honestly promise.
        ui.line(&format!(
            "  {} {}",
            ui.bold(&view.name),
            ui.dim(&format!("({})", view.entries))
        ));
        for sub in &view.subs {
            ui.line(&format!(
                "    {} {}",
                sub.name,
                ui.dim(&format!("({})", sub.entries))
            ));
        }
    }
}

/// Prints the programs a selection resolves to, with the categories left
/// enabled for each.
///
/// The search filter is the same one the terminal app's search field applies, so
/// what a script sees and what a user sees are the same list.
pub fn programs(state: &mut AppState, ui: &Ui, output: &OutputArgs, search: Option<&str>) {
    // `set_search` has to run before the views are read: it recomputes
    // `filtered_programs`, which is the order this lists them in. The state is
    // taken by reference for exactly that one call, not owned, so the databases
    // inside it are not cloned for the sake of a filter.
    if let Some(query) = search {
        state.set_search(query);
    }

    // One pass over the enabled programs, then the search filter applied on top
    // of it, so a program's categories are not recomputed per row.
    let enabled = plan::program_views(state);
    let views: Vec<(String, Vec<String>)> = state
        .filtered_programs
        .iter()
        .filter_map(|&index| enabled.get(index).cloned())
        .collect();

    if output.json {
        ui.print_json(
            &serde_json::json!({
                "programs": views.iter().map(|(name, categories)| serde_json::json!({
                    "program": name,
                    "categories": categories,
                })).collect::<Vec<_>>(),
            })
            .to_string(),
        );
        return;
    }
    if output.quiet {
        return;
    }

    if views.is_empty() {
        ui.line(&ui.dim(if search.is_some() {
            "No program matches the search."
        } else {
            "No programs for the selected categories."
        }));
        return;
    }

    ui.line(&ui.bold(&format!("{} programs", views.len())));
    for (name, categories) in &views {
        ui.line(&format!(
            "  {} {}",
            ui.bold(name),
            ui.dim(&format!("→ {}", categories.join(", ")))
        ));
    }
}

/// Prints what a `clean` would do, without doing it.
///
/// The plan answers "which of these is actually on my disk", which the category
/// and program names alone cannot: the database holds glob patterns, and
/// `--paths` prints them so a suspicious one can be checked before the real run.
pub fn dry_run(state: &AppState, ui: &Ui, output: &OutputArgs, paths: bool) {
    let entries = plan::entries(state);
    let programs = plan::program_views(state);

    if output.json {
        ui.print_json(
            &serde_json::json!({
                "programs": programs.len(),
                "paths": entries.len(),
                "entries": programs.iter().map(|(name, categories)| serde_json::json!({
                    "program": name,
                    "categories": categories,
                })).collect::<Vec<_>>(),
                "database_paths": entries.iter().map(|entry| serde_json::json!({
                    "program": entry.program,
                    "category": entry.category,
                    "subcategory": entry.sub_category,
                    "path": entry.path,
                    "registry": entry.registry,
                })).collect::<Vec<_>>(),
            })
            .to_string(),
        );
        return;
    }
    if output.quiet {
        return;
    }

    if programs.is_empty() {
        ui.line(&ui.warn("Nothing selected."));
        return;
    }

    ui.line(&ui.heading(&format!(
        "{} and {} would be looked at.",
        count(programs.len() as u64, "program"),
        count(entries.len() as u64, "pattern")
    )));
    for (name, categories) in &programs {
        ui.line(&format!(
            "  {} {}",
            ui.bold(name),
            ui.dim(&categories.join(", "))
        ));
    }

    if paths {
        ui.line("");
        ui.line(&ui.bold("Paths"));
        for entry in &entries {
            let source = if entry.registry { " [registry]" } else { "" };
            ui.line(&format!(
                "  {} {}{source}",
                entry.program,
                ui.dim(&format!("{}/{}", entry.category, entry.sub_category))
            ));
            ui.line(&format!("    {}", entry.path));
        }
    }
}
