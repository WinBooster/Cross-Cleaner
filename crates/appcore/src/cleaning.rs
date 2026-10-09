//! Background cleaning job: runs the selected cleaners concurrently and
//! reports progress through an mpsc channel.
//!
//! Two message shapes are sent:
//!
//! * `"Cleaning: <program>"` — the name of the entry that just started, used
//!   by the frontend as the "currently working on…" line.
//! * `"PROGRESS:<done>:<total>:<bytes>"` — the counters for the progress bar.

use cleaner::clear_data;
use database::cleaner_database::CleanerDatabase;
#[cfg(windows)]
use database::registry_database::{RegistryDatabase, clear_registry};
use database::structures::{CleanerResult, Cleared, ClearedPath, CustomCleaner};
use database::utils::get_file_size_string;
use futures::stream::{FuturesUnordered, StreamExt};
use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::io::Write;
use std::pin::Pin;
use std::sync::Arc;
use tempfile::NamedTempFile;
use tokio::sync::mpsc;

use crate::categories::effective_sub;

/// Prefix of the progress counter messages.
const PROGRESS_PREFIX: &str = "PROGRESS:";
/// Prefix of the "currently cleaning" messages.
const CLEANING_PREFIX: &str = "Cleaning: ";

/// How many deleted paths one program keeps for the results page.
///
/// A `…\Cache_Data\*` glob can match tens of thousands of files, and the detail
/// list is there to be scrolled through, not read to the end: past this point
/// the extra entries cost memory in every frame after the run without adding
/// anything a user can reach. The ones kept are the largest, and the page says
/// how many it left out rather than passing the list off as complete.
const MAX_PATH_DETAILS: usize = 1000;

/// Everything the cleaning job reports back.
///
/// The program list is shared rather than copied: it is read by every frame of
/// the results page and holds a detail entry per deleted path, so cloning it
/// once per repaint would cost more than drawing the page does.
pub type CleanResult = (u64, u64, u64, Arc<[Cleared]>);

pub async fn work(
    selected_map: HashMap<Arc<str>, HashSet<Arc<str>>>,
    progress_sender: mpsc::Sender<String>,
    database: &CleanerDatabase,
    custom_database: &[CustomCleaner],
    #[cfg(windows)] registry_database: &RegistryDatabase,
    excluded_programs: HashSet<Arc<str>>,
    excluded_program_categories: HashSet<(Arc<str>, Arc<str>)>,
) -> CleanResult {
    let mut current_task = 0;

    // ASYNC without threads: pure FuturesUnordered
    let mut bytes_cleared: u64 = 0;
    let mut removed_files: u64 = 0;
    let mut removed_directories: u64 = 0;
    let mut cleared_programs = Vec::<Cleared>::new();

    // C: limit to 8 concurrent cleaners
    let sem = Arc::new(tokio::sync::Semaphore::new(8));
    let mut futures: FuturesUnordered<Pin<Box<dyn Future<Output = CleanerResult> + Send>>> =
        FuturesUnordered::new();

    // INFO: Clear LastActivity from Registry
    // WARN: Windows only - show what is being cleaned right now
    // Streams directly into FuturesUnordered to avoid buffering all matches in RAM.
    #[cfg(windows)]
    {
        let _ = registry_database.for_each(|data| {
            let eff = effective_sub(&data.class, &data.sub_category);
            if let Some(subs) = selected_map.get(data.category.as_ref())
                && subs.contains(&eff)
                && !excluded_programs.contains(data.program.as_ref())
                && !excluded_program_categories
                    .contains(&(Arc::clone(&data.program), Arc::clone(&data.category)))
            {
                let sender = progress_sender.clone();
                let name_msg = data.program.clone();
                let sem = sem.clone();
                futures.push(Box::pin(async move {
                    let _p = sem.acquire_owned().await.unwrap();
                    let _ = sender.send(format!("{CLEANING_PREFIX}{name_msg}")).await;
                    clear_registry(&data)
                }));
            }
        });
    }

    // INFO: Run built-in custom cleanings (functions defined in cleaner::custom_cleaners)
    let mut sequential_cleaners: Vec<CustomCleaner> = Vec::new();
    for data in custom_database.iter() {
        let eff = effective_sub("", &data.sub_category);
        if let Some(subs) = selected_map.get(data.category.as_ref())
            && subs.contains(&eff)
            && !excluded_programs.contains(data.program.as_ref())
            && !excluded_program_categories
                .contains(&(Arc::clone(&data.program), Arc::clone(&data.category)))
        {
            if data.sequential {
                sequential_cleaners.push(data.clone());
            } else {
                let data = data.clone();
                let sender = progress_sender.clone();
                let name_msg = data.id.clone();
                let sem = sem.clone();
                futures.push(Box::pin(async move {
                    let _p = sem.acquire_owned().await.unwrap();
                    let _ = sender.send(format!("{CLEANING_PREFIX}{name_msg}")).await;
                    let progress_for_cleaner = sender.clone();
                    database::custom_cleaners::run_custom_cleaner(&data, Some(progress_for_cleaner))
                        .await
                }));
            }
        }
    }

    // INFO: Stream the database and keep only the selected entries directly into
    // FuturesUnordered to avoid buffering Vec<Arc<CleanerData>> in RAM.
    let _ = database.for_each(|data| {
        let eff = effective_sub(&data.class, &data.sub_category);
        if let Some(subs) = selected_map.get(data.category.as_ref())
            && subs.contains(&eff)
            && !excluded_programs.contains(data.program.as_ref())
            && !excluded_program_categories
                .contains(&(Arc::clone(&data.program), Arc::clone(&data.category)))
        {
            let data = Arc::new(data);
            let sender = progress_sender.clone();
            let path_msg = data.program.clone();
            let sem = sem.clone();
            futures.push(Box::pin(async move {
                let _p = sem.acquire_owned().await.unwrap();
                let _ = sender.send(format!("{CLEANING_PREFIX}{path_msg}")).await;
                clear_data(&data).await
            }));
        }
    });

    let total_tasks = futures.len() + sequential_cleaners.len();
    let _ = progress_sender
        .send(format!("{PROGRESS_PREFIX}0:{total_tasks}:0"))
        .await;

    while let Some(result) = futures.next().await {
        current_task += 1;

        // Read before the move: `fold` owns the result, and the run's own totals
        // are keyed on whether it had anything to count at all.
        let (bytes, files, folders) = (result.bytes, result.files, result.folders);
        if fold(&mut cleared_programs, result) {
            bytes_cleared += bytes;
            removed_files += files;
            removed_directories += folders;
        }

        // Send only progress and cleared bytes; the program name was already sent before cleaning
        let _ = progress_sender
            .send(format!(
                "{PROGRESS_PREFIX}{current_task}:{total_tasks}:{bytes_cleared}"
            ))
            .await;
    }

    // Run sequential cleaners one at a time (image optimizers, etc.)
    for data in sequential_cleaners {
        current_task += 1;
        let _ = progress_sender
            .send(format!("{CLEANING_PREFIX}{}", data.id))
            .await;
        let result =
            database::custom_cleaners::run_custom_cleaner(&data, Some(progress_sender.clone()))
                .await;

        let (bytes, files, folders) = (result.bytes, result.files, result.folders);
        if fold(&mut cleared_programs, result) {
            bytes_cleared += bytes;
            removed_files += files;
            removed_directories += folders;
        }

        let _ = progress_sender
            .send(format!(
                "{PROGRESS_PREFIX}{current_task}:{total_tasks}:{bytes_cleared}"
            ))
            .await;
    }

    // Largest first: the detail overlay is a scrolling list, so the entries that
    // freed the most space have to be at the top of it.
    for cleared in &mut cleared_programs {
        cleared
            .paths
            .sort_by_key(|detail| Reverse(detail.removed_bytes));
    }

    let bytes_cleared_val = bytes_cleared;
    let removed_files_val = removed_files;
    let removed_directories_val = removed_directories;

    notify_result(
        bytes_cleared_val,
        removed_files_val,
        removed_directories_val,
    );

    (
        bytes_cleared_val,
        removed_files_val,
        removed_directories_val,
        cleared_programs.into(),
    )
}

/// Folds one finished cleaner into the per-program table.
///
/// The concurrent and the sequential pass both come through here, so they cannot
/// aggregate differently: one program ends up as one row with one list of
/// deleted paths however it was cleaned.
///
/// Returns `true` when the cleaner removed anything — a cleaner that matched
/// nothing contributes no row and nothing to the run's totals.
fn fold(cleared_programs: &mut Vec<Cleared>, result: CleanerResult) -> bool {
    if !result.working {
        return false;
    }

    let index = match cleared_programs
        .iter()
        .position(|cleared| cleared.program == result.program.as_ref())
    {
        Some(index) => index,
        None => {
            cleared_programs.push(Cleared {
                program: result.program.to_string(),
                removed_bytes: 0,
                removed_files: 0,
                removed_directories: 0,
                affected_categories: Vec::new(),
                paths: Vec::new(),
                paths_omitted: 0,
            });
            cleared_programs.len() - 1
        }
    };

    let cleared = &mut cleared_programs[index];
    cleared.removed_bytes += result.bytes;
    cleared.removed_files += result.files;
    cleared.removed_directories += result.folders;
    let category = result.category.to_string();
    if !cleared.affected_categories.contains(&category) {
        cleared.affected_categories.push(category);
    }
    // Counted even before the cap: a run that deleted more than the cap holds has
    // to say so, and the count is only true if it includes what was dropped here.
    cleared.paths_omitted += result.paths_omitted;
    for detail in result.paths {
        keep_detail(cleared, detail);
    }
    true
}

/// Keeps one deleted path, evicting the smallest kept entry once the cap bites.
///
/// Evicting rather than refusing is what makes the cap keep the useful half: a
/// truncated list that drops the paths which freed the most space would be the
/// wrong list to show.
fn keep_detail(cleared: &mut Cleared, detail: ClearedPath) {
    if cleared.paths.len() < MAX_PATH_DETAILS {
        cleared.paths.push(detail);
        return;
    }
    cleared.paths_omitted += 1;
    let Some((index, smallest)) = cleared
        .paths
        .iter()
        .enumerate()
        .min_by_key(|(_, kept)| kept.removed_bytes)
    else {
        return;
    };
    if smallest.removed_bytes < detail.removed_bytes {
        cleared.paths[index] = detail;
    }
}

/// Shows the desktop notification with the final numbers. Kept out of line
/// because it owns a temporary icon file for its whole body.
fn notify_result(bytes_cleared: u64, removed_files: u64, removed_directories: u64) {
    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(database::ICON_BYTES).unwrap();
    let icon_path = temp_file.path().to_str().unwrap();

    let notification_body = format!(
        "Removed: {}\nFiles: {}\nDirs: {}",
        get_file_size_string(bytes_cleared),
        removed_files,
        removed_directories
    );

    let notification_result = notify_rust::Notification::new()
        .summary("Cross Cleaner")
        .body(&notification_body)
        .icon(icon_path)
        .show();

    temp_file.close().unwrap();
    if let Err(e) = notification_result {
        database::diag::warn(format!("failed to show notification: {e:?}"));
    }
}

/// Strips the `"Cleaning: "` prefix from a progress message.
pub fn program_name(message: &str) -> &str {
    message.strip_prefix(CLEANING_PREFIX).unwrap_or(message)
}

/// Parses a `"PROGRESS:<done>:<total>:<bytes>"` message. Returns `None` for
/// every other message so the caller can treat it as a plain status line.
pub fn parse_progress(message: &str) -> Option<(usize, usize, u64)> {
    let rest = message.strip_prefix(PROGRESS_PREFIX)?;
    let mut parts = rest.split(':');
    let done = parts.next()?.parse().ok()?;
    let total = parts.next()?.parse().ok()?;
    let bytes = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((done, total, bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_progress_messages() {
        assert_eq!(parse_progress("PROGRESS:3:10:2048"), Some((3, 10, 2048u64)));
        assert_eq!(parse_progress("PROGRESS:0:0:0"), Some((0, 0, 0)));
    }

    #[test]
    fn rejects_non_progress_messages() {
        assert_eq!(parse_progress("Cleaning: Chrome"), None);
        assert_eq!(parse_progress("PROGRESS:3:10"), None);
        assert_eq!(parse_progress("PROGRESS:a:b:c"), None);
        assert_eq!(parse_progress("PROGRESS:1:2:3:4"), None);
        assert_eq!(parse_progress(""), None);
    }

    #[test]
    fn strips_program_prefix() {
        assert_eq!(program_name("Cleaning: Chrome"), "Chrome");
        assert_eq!(program_name("Chrome"), "Chrome");
    }

    /// A cleaner result as the job gets it back.
    fn result(program: &str, category: &str, bytes: u64, paths: &[(&str, u64)]) -> CleanerResult {
        CleanerResult {
            files: 0,
            folders: 0,
            bytes,
            working: true,
            path: "C:\\cache\\*".into(),
            paths_omitted: 0,
            paths: paths
                .iter()
                .map(|(path, bytes)| ClearedPath {
                    path: (*path).into(),
                    removed_bytes: *bytes,
                    removed_files: 1,
                    removed_directories: 0,
                })
                .collect(),
            program: Arc::from(program),
            category: Arc::from(category),
            sub_category: Arc::from(""),
        }
    }

    /// What the results page opens the detail overlay with: the deleted paths of
    /// one program, and how much each of them freed.
    #[test]
    fn deleted_paths_are_kept_per_program() {
        let mut cleared: Vec<Cleared> = Vec::new();
        assert!(fold(
            &mut cleared,
            result(
                "Chrome",
                "Cache",
                300,
                &[("C:\\cache\\a.tmp", 200), ("C:\\cache\\b.tmp", 100)],
            )
        ));
        // A second entry of the same program joins the row instead of adding one.
        assert!(fold(
            &mut cleared,
            result("Chrome", "Logs", 50, &[("C:\\logs\\c.log", 50)]),
        ));
        // And a different program gets a row of its own.
        fold(
            &mut cleared,
            result("Firefox", "Cache", 10, &[("C:\\ff\\x", 10)]),
        );

        assert_eq!(cleared.len(), 2);
        let chrome = &cleared[0];
        assert_eq!(chrome.removed_bytes, 350);
        assert_eq!(
            chrome.affected_categories,
            vec!["Cache".to_string(), "Logs".to_string()],
            "both categories belong to the one row",
        );
        let paths: Vec<(String, u64)> = chrome
            .paths
            .iter()
            .map(|detail| (detail.path.as_string(), detail.removed_bytes))
            .collect();
        assert_eq!(
            paths,
            vec![
                ("C:\\cache\\a.tmp".to_string(), 200),
                ("C:\\cache\\b.tmp".to_string(), 100),
                ("C:\\logs\\c.log".to_string(), 50),
            ],
        );
    }

    /// A cleaner that matched nothing must not leave a row behind, and must not
    /// be counted by the caller either.
    #[test]
    fn a_cleaner_that_removed_nothing_is_not_folded_in() {
        let mut idle = result("Chrome", "Cache", 0, &[]);
        idle.working = false;
        let mut cleared: Vec<Cleared> = Vec::new();
        assert!(!fold(&mut cleared, idle));
        assert!(cleared.is_empty());
    }

    /// A cleaner's own cap and this one both say what they left out, and the two
    /// counts add up: a row that silently lost items in two places is a row whose
    /// list cannot be trusted to be the whole story.
    #[test]
    fn omitted_items_from_the_cleaner_reach_the_row() {
        let mut cleared: Vec<Cleared> = Vec::new();
        let mut capped = result("Chrome", "Cache", 10, &[("C:\\cache\\a.tmp", 10)]);
        capped.paths_omitted = 900;
        fold(&mut cleared, capped);

        assert_eq!(cleared[0].paths.len(), 1, "the one item it did list");
        assert_eq!(cleared[0].paths_omitted, 900);
    }

    /// Past the cap the list keeps the largest paths rather than the first ones:
    /// a truncated list that dropped the biggest deletions would be the wrong
    /// list to show.
    #[test]
    fn the_detail_list_keeps_the_largest_paths_when_it_is_capped() {
        let mut cleared: Vec<Cleared> = Vec::new();
        let paths: Vec<(String, u64)> = (0..=MAX_PATH_DETAILS as u64)
            .map(|bytes| (format!("C:\\cache\\{bytes}"), bytes))
            .collect();
        let paths: Vec<(&str, u64)> = paths
            .iter()
            .map(|(path, bytes)| (path.as_str(), *bytes))
            .collect();
        fold(&mut cleared, result("Chrome", "Cache", 0, &paths));

        assert_eq!(cleared[0].paths.len(), MAX_PATH_DETAILS, "the cap holds");
        assert_eq!(
            cleared[0].paths_omitted, 1,
            "and the one it left out is counted, not hidden",
        );
        let smallest = cleared[0].paths.iter().map(|d| d.removed_bytes).min();
        assert_eq!(
            smallest,
            Some(1),
            "the smallest entry was evicted for the larger one",
        );
        let largest = cleared[0].paths.iter().map(|d| d.removed_bytes).max();
        assert_eq!(
            largest,
            Some(MAX_PATH_DETAILS as u64),
            "the biggest path is never the one dropped",
        );
    }
}
