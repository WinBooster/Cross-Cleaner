//! Background cleaning job: runs the selected cleaners concurrently and
//! reports progress through an mpsc channel.

use cleaner::clear_data;
use database::cleaner_database::CleanerDatabase;
#[cfg(windows)]
use database::registry_database::{RegistryDatabase, clear_registry};
#[cfg(windows)]
use database::structures::CleanerDataRegistry;
use database::structures::{CleanerData, CleanerResult, Cleared, CustomCleaner};
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

pub async fn work(
    selected_map: HashMap<String, HashSet<String>>,
    progress_sender: mpsc::Sender<String>,
    database: &CleanerDatabase,
    custom_database: &[CustomCleaner],
    #[cfg(windows)] registry_database: &RegistryDatabase,
    excluded_programs: HashSet<String>,
    excluded_program_categories: HashSet<(String, String)>,
) -> (u64, u64, u64, Vec<Cleared>) {
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
    #[cfg(windows)]
    {
        // INFO: Stream the registry database and keep only the selected entries.
        let mut registry_matches: Vec<CleanerDataRegistry> = Vec::new();
        let _ = registry_database.for_each(|data| {
            let eff = effective_sub(&data.class, &data.sub_category);
            if let Some(subs) = selected_map.get(&data.category) {
                if subs.contains(&eff)
                    && !excluded_programs.contains(&data.program)
                    && !excluded_program_categories
                        .contains(&(data.program.clone(), data.category.clone()))
                {
                    registry_matches.push(data);
                }
            }
        });

        for data in registry_matches {
            let sender = progress_sender.clone();
            let name_msg = data.program.clone();
            let sem = sem.clone();
            futures.push(Box::pin(async move {
                let _p = sem.acquire_owned().await.unwrap();
                let _ = sender.send(format!("Cleaning: {}", name_msg)).await;
                clear_registry(&data)
            }));
        }
    }

    // INFO: Run built-in custom cleanings (functions defined in cleaner::custom_cleaners)
    for data in custom_database.iter() {
        let eff = effective_sub("", &data.sub_category);
        if let Some(subs) = selected_map.get(&data.category) {
            if subs.contains(&eff)
                && !excluded_programs.contains(&data.program)
                && !excluded_program_categories
                    .contains(&(data.program.clone(), data.category.clone()))
            {
                let data = data.clone();
                let sender = progress_sender.clone();
                let _name_msg = data.id.clone();
                let sem = sem.clone();
                futures.push(Box::pin(async move {
                    let _p = sem.acquire_owned().await.unwrap();
                    let progress_for_cleaner = sender.clone();
                    tokio::task::spawn_blocking(move || {
                        database::custom_cleaners::run_custom_cleaner(&data, Some(progress_for_cleaner))
                    })
                    .await
                    .unwrap_or_else(|_| CleanerResult {
                        files: 0,
                        folders: 0,
                        bytes: 0,
                        working: false,
                        path: String::new(),
                        program: String::new(),
                        category: String::new(),
                        sub_category: String::new(),
                    })
                }));
            }
        }
    }

    // INFO: Stream the database and keep only the selected entries. Each entry
    // is shared as an Arc so the per-path work inside clear_data does not clone it.
    let mut database_matches: Vec<Arc<CleanerData>> = Vec::new();
    let _ = database.for_each(|data| {
        let eff = effective_sub(&data.class, &data.sub_category);
        if let Some(subs) = selected_map.get(&data.category) {
            if subs.contains(&eff)
                && !excluded_programs.contains(&data.program)
                && !excluded_program_categories
                    .contains(&(data.program.clone(), data.category.clone()))
            {
                database_matches.push(Arc::new(data));
            }
        }
    });

    for data in database_matches {
        let sender = progress_sender.clone();
        let path_msg = data.program.clone();
        let sem = sem.clone();
        futures.push(Box::pin(async move {
            let _p = sem.acquire_owned().await.unwrap();
            let _ = sender.send(format!("Cleaning: {}", path_msg)).await;
            clear_data(&data).await
        }));
    }

    let total_tasks = futures.len();
    let _ = progress_sender
        .send(format!("PROGRESS:0:{}:0", total_tasks))
        .await;

    while let Some(result) = futures.next().await {
        current_task += 1;

        if result.working {
            bytes_cleared += result.bytes;
            removed_files += result.files;
            removed_directories += result.folders;

            if let Some(cleared) = cleared_programs
                .iter_mut()
                .find(|c| c.program == result.program)
            {
                cleared.removed_bytes += result.bytes;
                cleared.removed_files += result.files;
                cleared.removed_directories += result.folders;
                if !cleared.affected_categories.contains(&result.category) {
                    cleared.affected_categories.push(result.category);
                }
            } else {
                cleared_programs.push(Cleared {
                    program: result.program,
                    removed_bytes: result.bytes,
                    removed_files: result.files,
                    removed_directories: result.folders,
                    affected_categories: vec![result.category],
                });
            }
        }

        // Send only progress and cleared bytes; the program name was already sent before cleaning
        let _ = progress_sender
            .send(format!(
                "PROGRESS:{}:{}:{}",
                current_task, total_tasks, bytes_cleared
            ))
            .await;
    }

    let bytes_cleared_val = bytes_cleared;
    let removed_files_val = removed_files;
    let removed_directories_val = removed_directories;

    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(database::ICON_BYTES).unwrap();
    let icon_path = temp_file.path().to_str().unwrap();

    let notification_body = format!(
        "Removed: {}\nFiles: {}\nDirs: {}",
        get_file_size_string(bytes_cleared_val),
        removed_files_val,
        removed_directories_val
    );

    let mut notification = notify_rust::Notification::new();
    let notification = notification
        .summary("Cross Cleaner GUI")
        .body(&notification_body)
        .icon(icon_path);

    let notification_result = notification.show();

    temp_file.close().unwrap();
    if let Err(e) = notification_result {
        eprintln!("Failed to show notification: {:?}", e);
    }

    (
        bytes_cleared_val,
        removed_files_val,
        removed_directories_val,
        cleared_programs,
    )
}
