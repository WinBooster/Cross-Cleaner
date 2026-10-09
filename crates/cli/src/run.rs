//! Running a selection and reporting what came of it.
//!
//! The job itself is [`appcore::cleaning::work`] — this module only drives
//! [`AppState`], exactly the way `tui` and `gui` do: start the run, drain the
//! progress channel, poll for the result. Everything a run reports is already
//! computed by `appcore`; the work here is turning it into lines.
//!
//! Two things the window frontends do not need and this one does:
//!
//! * **Confirmation.** A window app has a button, and clicking it is the
//!   consent. A command line may be run from a script, so `--yes` exists and
//!   the prompt is skipped when stdin is not a terminal: a pipe cannot answer a
//!   question, and blocking on a read that will never come is worse than
//!   refusing to start.
//! * **Interrupt.** Ctrl-C has to stop the run and say what was already gone,
//!   rather than leaving the process killed mid-delete with no report at all.

use std::io::{IsTerminal, Read, Write};

use appcore::AppState;

use crate::args::{CleanArgs, OutputArgs};
use crate::select::Selection;
use crate::term::{Ui, plural};

/// What a finished run reported, so the caller can render it either way.
pub struct RunReport {
    pub bytes: u64,
    pub files: u64,
    pub directories: u64,
    pub cleared: Vec<database::structures::Cleared>,
    /// False when the run was cut short by Ctrl-C, so the exit code can say so.
    pub complete: bool,
}

/// Starts the run, waits for it, and prints the result.
///
/// `state` is expected to have the selection already applied — see
/// [`crate::select::apply`]. The category selection is consumed by the run, so
/// this is a one-way door for a given state, which is why the caller builds a
/// fresh one per invocation anyway.
pub async fn run(
    state: &mut AppState,
    ui: &Ui,
    selection: &Selection,
    args: &CleanArgs,
) -> RunReport {
    if !args.yes && !confirm(ui, selection) {
        return RunReport {
            bytes: 0,
            files: 0,
            directories: 0,
            cleared: Vec::new(),
            complete: false,
        };
    }

    state.start_cleaning();

    // INFO: `AppState` reports progress through a channel rather than a
    // callback, so the loop has to keep draining it: the channel is bounded at
    // 32 messages, and a cleaner that filled it would block until this reads.
    //
    // The Ctrl-C future is created once and polled every iteration: a fresh one
    // per tick would re-register the handler each time, and the one that has
    // already fired would never fire again.
    let interrupt = tokio::signal::ctrl_c();
    tokio::pin!(interrupt);
    let mut interrupted = false;
    // Whether the run reported a result. Tracked here rather than read off
    // `poll_result` afterwards, because that call consumes the result: a
    // second one always answers "nothing new", which would look exactly like a
    // job that died without reporting.
    let mut reported = false;

    while state.is_cleaning() {
        tokio::select! {
            _ = &mut interrupt => {
                // First Ctrl-C stops the run. The handle is aborted rather than
                // dropped, so a cleaner that is halfway through removing a
                // directory is not left with a half-removed one: `abort` cancels
                // at the next await point, and every removal path in `cleaner`
                // reaches one.
                interrupted = true;
                ui.line(&ui.warn("Stopping — the entries in flight finish first."));
                if let Some(handle) = state.task_handle.take() {
                    handle.abort();
                }
                break;
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {}
        }

        // Drain before polling: the result is only sent once the last cleaner is
        // done, and the progress messages queued before it still have to reach
        // the counters, or the final frame would show the second-to-last one.
        state.drain_progress();
        draw(ui, state);
        if state.poll_result() {
            reported = true;
            break;
        }
    }

    // Anything still queued after the loop, whether the run finished between
    // the sleep and the poll or was cut short mid-report.
    state.drain_progress();
    if !reported {
        reported = state.poll_result();
    }
    let complete = reported && !interrupted;

    ui.clear_progress();

    let (bytes, files, directories, cleared) = match state.cleared_data.clone() {
        Some(data) => data,
        None => {
            // The run was aborted, so no result will ever arrive. Report what
            // the progress messages did say rather than nothing at all.
            if interrupted {
                ui.line(
                    &ui.warn("Interrupted. The numbers above are what had been removed so far."),
                );
            } else {
                ui.line(&ui.bad("The cleaning job ended without reporting a result."));
            }
            return RunReport {
                bytes: 0,
                files: 0,
                directories: 0,
                cleared: Vec::new(),
                complete: false,
            };
        }
    };

    let cleared: Vec<database::structures::Cleared> = cleared.iter().cloned().collect();
    let report = RunReport {
        bytes,
        files,
        directories,
        cleared: cleared.clone(),
        complete,
    };
    render(ui, &report, &args.output);
    report
}

/// Draws one progress line from the state, the same numbers the window app's
/// progress bar shows.
fn draw(ui: &Ui, state: &AppState) {
    let fraction = state.progress_fraction();
    let program = state.current_program();
    let mut detail = String::new();
    if state.total_tasks > 0 {
        detail.push_str(&format!("{}/{}", state.current_task, state.total_tasks));
    }
    if state.cleaned_bytes > 0 {
        detail.push_str(&format!(
            " · {} freed",
            database::utils::get_file_size_string(state.cleaned_bytes)
        ));
    }
    if let Some(eta) = state.eta() {
        detail.push_str(&format!(" · {eta} left"));
    }
    let label = if program.is_empty() {
        "Cleaning".to_string()
    } else {
        program.to_string()
    };
    ui.progress(fraction, &label, &detail);
}

/// Prints the result: the table, then the paths if `--verbose` asked for them.
pub fn render(ui: &Ui, report: &RunReport, output: &OutputArgs) {
    if output.json {
        ui.print_json(&json(report));
        return;
    }

    ui.result_table(
        report.bytes,
        report.files,
        report.directories,
        &report.cleared,
    );

    if !output.verbose || ui.quiet() {
        return;
    }
    // Largest first, the same order the window app's detail overlay lists them
    // in, so the paths that freed the most are at the top of the terminal too.
    let mut cleared: Vec<&database::structures::Cleared> = report.cleared.iter().collect();
    cleared.sort_by_key(|row| std::cmp::Reverse(row.removed_bytes));
    for entry in cleared {
        ui.path_details(entry);
    }
}

/// The result as one JSON object, for a script that reads this instead of a
/// human.
fn json(report: &RunReport) -> String {
    let programs: Vec<serde_json::Value> = report
        .cleared
        .iter()
        .map(|entry| {
            serde_json::json!({
                "program": entry.program,
                "bytes": entry.removed_bytes,
                "files": entry.removed_files,
                "directories": entry.removed_directories,
                "categories": entry.affected_categories,
                "paths_omitted": entry.paths_omitted,
                "paths": entry.paths.iter().map(|detail| serde_json::json!({
                    "path": detail.path.as_string(),
                    "bytes": detail.removed_bytes,
                    "files": detail.removed_files,
                    "directories": detail.removed_directories,
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    serde_json::json!({
        "complete": report.complete,
        "bytes": report.bytes,
        "files": report.files,
        "directories": report.directories,
        "programs": programs,
    })
    .to_string()
}

/// Asks before deleting, unless `--yes` already answered.
///
/// Skipped entirely when stdin is not a terminal: a script or a pipeline cannot
/// answer, and a read that waits forever on a closed stdin is a hang, not a
/// safeguard. `--yes` is the documented way to say yes in that case.
fn confirm(ui: &Ui, selection: &Selection) -> bool {
    if !std::io::stdin().is_terminal() {
        return true;
    }
    ui.line(&format!(
        "About to clean {} and {}.",
        plural(selection.programs.len(), "program", "programs"),
        plural(
            selection.programs.iter().map(|(_, cats)| cats.len()).sum(),
            "category",
            "categories"
        )
    ));
    print!("Continue? [y/N] ");
    let _ = std::io::stdout().flush();

    let mut answer = [0u8; 8];
    let read = std::io::stdin().read(&mut answer);
    let answer = match read {
        Ok(n) => String::from_utf8_lossy(&answer[..n]).trim().to_lowercase(),
        Err(error) => {
            ui.warning(&format!("Could not read the answer: {error}"));
            return false;
        }
    };
    answer == "y" || answer == "yes"
}

#[cfg(test)]
mod tests {
    use super::*;
    use appcore::AppState;
    use database::cleaner_database::CleanerDatabase;
    use database::structures::{CleanerData, CleanerFlags};
    use std::sync::Arc;

    fn entry(category: &str, program: &str, sub: &str) -> CleanerData {
        CleanerData {
            path: format!("{program}/{sub}").into(),
            category: Arc::from(category),
            program: Arc::from(program),
            class: Arc::from("Application"),
            sub_category: Arc::from(sub),
            files_to_remove: vec![],
            directories_to_remove: vec![],
            flags: CleanerFlags::empty(),
        }
    }

    fn state(entries: Vec<CleanerData>) -> AppState {
        let database = CleanerDatabase::from_vec(entries);
        let custom = Arc::from(Vec::new());
        #[cfg(windows)]
        {
            AppState::from_database(
                database,
                database::registry_database::RegistryDatabase::from_vec(Vec::new()),
                custom,
            )
        }
        #[cfg(not(windows))]
        {
            AppState::from_database(database, custom)
        }
    }

    #[tokio::test]
    async fn a_run_with_nothing_to_delete_reports_zeroes() {
        // A category whose entries point at paths that do not exist: the job
        // runs, matches nothing, and has to say so rather than hang or fail.
        let mut app = state(vec![entry("Cache", "Chrome", "Browser")]);
        app.toggle_category(0);
        app.build_program_list();
        let ui = Ui::new(true, true, false);
        let selection = crate::select::Selection {
            programs: vec![("Chrome".to_string(), vec!["Cache".to_string()])],
        };
        let args = CleanArgs {
            selection: Default::default(),
            output: OutputArgs {
                json: true,
                ..Default::default()
            },
            dry_run: false,
            yes: true,
        };
        let report = run(&mut app, &ui, &selection, &args).await;
        assert!(report.complete);
        assert_eq!(report.bytes, 0);
        assert_eq!(report.cleared.len(), 0);
    }

    #[tokio::test]
    async fn a_json_report_carries_the_totals() {
        let report = RunReport {
            bytes: 100,
            files: 2,
            directories: 1,
            cleared: vec![database::structures::Cleared {
                program: "Chrome".to_string(),
                removed_bytes: 100,
                removed_files: 2,
                removed_directories: 1,
                affected_categories: vec!["Cache".to_string()],
                paths: Vec::new(),
                paths_omitted: 0,
            }],
            complete: true,
        };
        let value: serde_json::Value = serde_json::from_str(&json(&report)).expect("valid JSON");
        assert_eq!(value["bytes"], 100);
        assert_eq!(value["programs"][0]["program"], "Chrome");
        assert_eq!(value["complete"], true);
    }
}
