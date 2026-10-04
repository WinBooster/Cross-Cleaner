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
use database::structures::{Cleared, CustomCleaner};
use database::utils::get_file_size_string;
use futures::stream::{FuturesUnordered, StreamExt};
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

/// Everything the cleaning job reports back.
pub type CleanResult = (u64, u64, u64, Vec<Cleared>);

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
    let mut futures: FuturesUnordered<
        Pin<Box<dyn Future<Output = database::structures::CleanerResult> + Send>>,
    > = FuturesUnordered::new();

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

        if result.working {
            bytes_cleared += result.bytes;
            removed_files += result.files;
            removed_directories += result.folders;

            if let Some(cleared) = cleared_programs
                .iter_mut()
                .find(|c| c.program == result.program.as_ref())
            {
                cleared.removed_bytes += result.bytes;
                cleared.removed_files += result.files;
                cleared.removed_directories += result.folders;
                if !cleared
                    .affected_categories
                    .contains(&result.category.to_string())
                {
                    cleared
                        .affected_categories
                        .push(result.category.to_string());
                }
            } else {
                cleared_programs.push(Cleared {
                    program: result.program.to_string(),
                    removed_bytes: result.bytes,
                    removed_files: result.files,
                    removed_directories: result.folders,
                    affected_categories: vec![result.category.to_string()],
                });
            }
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

        if result.working {
            bytes_cleared += result.bytes;
            removed_files += result.files;
            removed_directories += result.folders;

            if let Some(cleared) = cleared_programs
                .iter_mut()
                .find(|c| c.program == result.program.as_ref())
            {
                cleared.removed_bytes += result.bytes;
                cleared.removed_files += result.files;
                cleared.removed_directories += result.folders;
                if !cleared
                    .affected_categories
                    .contains(&result.category.to_string())
                {
                    cleared
                        .affected_categories
                        .push(result.category.to_string());
                }
            } else {
                cleared_programs.push(Cleared {
                    program: result.program.to_string(),
                    removed_bytes: result.bytes,
                    removed_files: result.files,
                    removed_directories: result.folders,
                    affected_categories: vec![result.category.to_string()],
                });
            }
        }

        let _ = progress_sender
            .send(format!(
                "{PROGRESS_PREFIX}{current_task}:{total_tasks}:{bytes_cleared}"
            ))
            .await;
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
        cleared_programs,
    )
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
}
