// PERFORMANCE: Use mimalloc for blazing fast memory allocation
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use clap::{ArgAction, Parser};
use cleaner::clear_data;
use crossterm::execute;
#[cfg(windows)]
use database::registry_database::clear_registry;
#[cfg(windows)]
use database::structures::CleanerDataRegistry;
use database::structures::{CleanerData, CleanerResult, Cleared};
use database::utils::get_file_size_string;
use database::{get_icon, get_version};
use futures::stream::{FuturesUnordered, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use inquire::formatter::MultiOptionFormatter;
use inquire::list_option::ListOption;
use inquire::validator::Validation;
use inquire::MultiSelect;
use lazy_static::lazy_static;
use notify_rust::Notification;
use rhai::Engine;
use script_runner;
use std::collections::HashSet;
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use tabled::Table;
use tempfile::NamedTempFile;

use tokio::task;

#[cfg(windows)]
use std::io::stdin;
use std::io::stdout;

#[cfg(windows)]
fn is_admin() -> bool {
    use winapi::um::processthreadsapi::OpenProcessToken;
    unsafe {
        let mut token = std::ptr::null_mut();
        if OpenProcessToken(
            winapi::um::processthreadsapi::GetCurrentProcess(),
            winapi::um::winnt::TOKEN_QUERY,
            &mut token,
        ) == 0
        {
            return false;
        }
        let mut elevation = winapi::um::winnt::TOKEN_ELEVATION::default();
        let mut size = std::mem::size_of::<winapi::um::winnt::TOKEN_ELEVATION>() as u32;
        let ret = winapi::um::securitybaseapi::GetTokenInformation(
            token,
            winapi::um::winnt::TokenElevation,
            &mut elevation as *mut _ as *mut _,
            size,
            &mut size,
        );
        winapi::um::handleapi::CloseHandle(token);
        ret != 0 && elevation.TokenIsElevated != 0
    }
}

async fn work(
    args: &Args,
    disabled_programs: Vec<&str>,
    categories: Vec<String>,
    database: &[CleanerData],
    #[cfg(windows)] registry_database: &[CleanerDataRegistry],
) {
    // Use atomics for lock-free concurrent counting - BLAZING FAST!
    let bytes_cleared = Arc::new(AtomicU64::new(0));
    let removed_files = Arc::new(AtomicU64::new(0));
    let removed_directories = Arc::new(AtomicU64::new(0));

    // Pre-allocate with exact capacity
    let cleared_programs = Arc::new(Mutex::new(Vec::<Cleared>::with_capacity(database.len())));

    let pb = if args.show_progress_bar {
        let sty = ProgressStyle::with_template(
            "[{elapsed_precise}] {prefix:.bold.dim} {spinner:.green}\n[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} [{msg}]",
        )
            .unwrap()
            .progress_chars("##-")
            .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ");

        let pb = ProgressBar::new(0);
        pb.set_style(sty);
        pb.set_prefix("Clearing");
        Some(pb)
    } else {
        None
    };

    // Pre-convert to HashSet for O(1) lookups instead of O(n) contains
    let disabled_programs_set: HashSet<&str> = disabled_programs.into_iter().collect();
    let categories_lower: HashSet<String> = categories.iter().map(|s| s.to_lowercase()).collect();

    let mut tasks = Vec::with_capacity(database.len() + 1);

    // INFO: Clear LastActivity from Registry
    // WARN: Windows only
    #[cfg(windows)]
    {
        for data in registry_database.iter() {
            if categories_lower.contains(&data.category.to_lowercase())
                && !disabled_programs_set.contains(data.program.to_lowercase().as_str())
            {
                let data = Arc::new(data.clone());
                let task = task::spawn(async move { clear_registry(&data) });
                tasks.push(task);
            }
        }
    }

    // Filter and spawn tasks - avoid cloning in filter closure
    for data in database.iter() {
        if categories_lower.contains(&data.category.to_lowercase())
            && !disabled_programs_set.contains(data.program.to_lowercase().as_str())
        {
            let data = Arc::new(data.clone());
            let task = task::spawn(async move { clear_data(&data) });
            tasks.push(task);
        }
    }

    if let Some(ref pb) = &pb {
        pb.set_length(tasks.len() as u64);
    }

    // Use FuturesUnordered to process tasks as they complete (smoother progress)
    let mut futures = tasks.into_iter().collect::<FuturesUnordered<_>>();

    // Batch progress updates every N items for better performance
    const PROGRESS_BATCH_SIZE: u64 = 5;
    let mut completed_since_update = 0u64;

    while let Some(task_result) = futures.next().await {
        match task_result {
            Ok(result) => {
                if result.working {
                    // Atomic operations - no locks, blazing fast!
                    bytes_cleared.fetch_add(result.bytes, Ordering::Relaxed);
                    removed_files.fetch_add(result.files, Ordering::Relaxed);
                    removed_directories.fetch_add(result.folders, Ordering::Relaxed);

                    // Only lock once per task for the Vec update
                    let mut cleared_programs = cleared_programs.lock().unwrap();

                    // Use binary search hint or direct lookup for large datasets
                    if let Some(cleared) = cleared_programs
                        .iter_mut()
                        .find(|c| c.program == result.program)
                    {
                        cleared.removed_bytes += result.bytes as u64;
                        cleared.removed_files += result.files as u64;
                        cleared.removed_directories += result.folders as u64;
                        if !cleared.affected_categories.contains(&result.category) {
                            cleared.affected_categories.push(result.category);
                        }
                    } else {
                        cleared_programs.push(Cleared {
                            program: result.program,
                            removed_bytes: result.bytes as u64,
                            removed_files: result.files as u64,
                            removed_directories: result.folders as u64,
                            affected_categories: vec![result.category],
                        });
                    }
                    drop(cleared_programs); // Explicit drop to release lock ASAP
                }

                // Batch progress bar updates
                completed_since_update += 1;
                if completed_since_update >= PROGRESS_BATCH_SIZE || futures.is_empty() {
                    if let Some(ref pb) = pb {
                        pb.set_message(result.path);
                        pb.inc(completed_since_update);
                    }
                    completed_since_update = 0;
                }
            }
            Err(e) => {
                eprintln!("Error waiting for task completion: {:?}", e);
                completed_since_update += 1;
                if completed_since_update >= PROGRESS_BATCH_SIZE || futures.is_empty() {
                    if let Some(ref pb) = pb {
                        pb.inc(completed_since_update);
                    }
                    completed_since_update = 0;
                }
            }
        }
    }

    if let Some(ref pb) = &pb {
        pb.set_message("done");
        pb.finish();
    }

    // Load atomic values once at the end
    let bytes_cleared_val = bytes_cleared.load(Ordering::Relaxed);
    let removed_files_val = removed_files.load(Ordering::Relaxed);
    let removed_directories_val = removed_directories.load(Ordering::Relaxed);
    let _cleared_programs_val = cleared_programs.lock().unwrap().clone();

    if args.show_result_table {
        println!("Cleared result:");
        let cleared_programs = cleared_programs.lock().unwrap();
        let table = Table::new(cleared_programs.iter()).to_string();
        println!("{}", table);
    }

    if args.show_result_string {
        println!(
            "Removed size: {}, files: {}, dirs: {}, programs: {}",
            get_file_size_string(bytes_cleared_val),
            removed_files_val,
            removed_directories_val,
            cleared_programs.lock().unwrap().len()
        );
    }

    if args.show_notification {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(get_icon()).unwrap();
        let icon_path = temp_file.path().to_str().unwrap();

        let notification_body = format!(
            "Removed: {}\nFiles: {}\nDirs: {}",
            get_file_size_string(bytes_cleared_val),
            removed_files_val,
            removed_directories_val
        );

        let notification_result = Notification::new()
            .summary("Cross Cleaner CLI")
            .body(&notification_body)
            .icon(icon_path)
            .show();

        temp_file.close().unwrap();
        if let Err(e) = notification_result {
            eprintln!("Failed to show notification: {:?}", e);
        }
    }
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Specify categories to clear (comma-separated)
    /// Example: --clear=logs,cache
    #[arg(long, value_name = "categories")]
    clear: Option<String>,

    /// Specify programs to disable (comma-separated)
    /// Example: --disabled=minecraft,firefox
    #[arg(long, value_name = "programs")]
    disabled: Option<String>,

    /// Show database statistic.
    /// Example: --show-database-info=true
    #[arg(long, value_name = "bool", default_value_t = false, action = ArgAction::Set)]
    show_database_info: bool,

    /// Show progress bar during execution.
    /// Example: --progress-bar=false
    #[arg(long, value_name = "bool", default_value_t = true, action = ArgAction::Set)]
    show_progress_bar: bool,

    /// Show the result as a table after execution.
    /// Example: --result-table=false
    #[arg(long, value_name = "bool", default_value_t = true, action = ArgAction::Set)]
    show_result_table: bool,

    /// Show the result as a string after execution.
    /// Example: --result-string=false
    #[arg(long, value_name = "bool", default_value_t = true, action = ArgAction::Set)]
    show_result_string: bool,

    /// Show a desktop notification after execution.
    /// Example: --show-notification=false
    #[arg(long, value_name = "bool", default_value_t = true, action = ArgAction::Set)]
    show_notification: bool,

    /// Specify a custom database file path.
    /// Example: --database-path=custom_database.json
    #[arg(long, value_name = "path")]
    database_path: Option<String>,

    /// Specify a custom registry database file path.
    /// Example: --registry-database-path=custom_database.json
    #[cfg(windows)]
    #[arg(long, value_name = "registry_path")]
    registry_database_path: Option<String>,
}

lazy_static! {
    static ref ADDITIONAL_DATA: Mutex<Vec<CleanerData>> = Mutex::new(Vec::new());
}

fn add_cleaner_data(data: rhai::Map) {
    let cleaner_data = CleanerData {
        path: data
            .get("path")
            .and_then(|v| v.clone().try_cast::<String>())
            .unwrap_or_default(),
        category: data
            .get("category")
            .and_then(|v| v.clone().try_cast::<String>())
            .unwrap_or_default(),
        program: data
            .get("program")
            .and_then(|v| v.clone().try_cast::<String>())
            .unwrap_or_default(),
        class: data
            .get("class")
            .and_then(|v| v.clone().try_cast::<String>())
            .unwrap_or_else(|| String::from("Other")),
        files_to_remove: data
            .get("files_to_remove")
            .and_then(|v| v.clone().try_cast::<rhai::Array>())
            .map(|arr| {
                arr.into_iter()
                    .filter_map(|v| v.try_cast::<String>())
                    .collect()
            })
            .unwrap_or_default(),
        directories_to_remove: data
            .get("directories_to_remove")
            .and_then(|v| v.clone().try_cast::<rhai::Array>())
            .map(|arr| {
                arr.into_iter()
                    .filter_map(|v| v.try_cast::<String>())
                    .collect()
            })
            .unwrap_or_default(),
        remove_all_in_dir: data
            .get("remove_all_in_dir")
            .and_then(|v| v.clone().try_cast::<bool>())
            .unwrap_or(false),
        remove_directory_after_clean: data
            .get("remove_directory_after_clean")
            .and_then(|v| v.clone().try_cast::<bool>())
            .unwrap_or(false),
        remove_directories: data
            .get("remove_directories")
            .and_then(|v| v.clone().try_cast::<bool>())
            .unwrap_or(false),
        remove_files: data
            .get("remove_files")
            .and_then(|v| v.clone().try_cast::<bool>())
            .unwrap_or(false),
    };
    ADDITIONAL_DATA.lock().unwrap().push(cleaner_data);
}

fn run_scripts() -> Result<Vec<CleanerData>, Box<dyn std::error::Error>> {
    let mut engine = Engine::new();

    // Register types
    engine.register_type::<CleanerData>();
    engine.register_type::<CleanerResult>();

    // Register the clear_data function
    engine.register_fn("clear_data", |data: CleanerData| -> CleanerResult {
        clear_data(&data)
    });

    // Register function to add custom cleaning data
    engine.register_fn("add_cleaner_data", add_cleaner_data);

    // Register file system functions
    engine.register_fn("delete_file", |path: String| -> i64 {
        if let Ok(metadata) = std::fs::metadata(&path) {
            let size = metadata.len() as i64;
            if std::fs::remove_file(&path).is_ok() {
                size
            } else {
                0
            }
        } else {
            0
        }
    });

    script_runner::run_scripts(&mut engine, add_cleaner_data)?;

    Ok(ADDITIONAL_DATA.lock().unwrap().drain(..).collect())
}

#[tokio::main]
async fn main() {
    execute!(
        stdout(),
        crossterm::terminal::SetTitle(format!("Cross Cleaner CLI v{}", get_version()))
    )
    .unwrap();

    let args = Args::parse();

    #[cfg(windows)]
    if !is_admin() {
        eprintln!(
            "The application is not launched with administrator rights, functionality is limited"
        );
    }

    let mut database: Vec<CleanerData> = if let Some(db_path) = &args.database_path {
        match database::cleaner_database::get_database_from_file(db_path) {
            Ok(db) => db,
            Err(e) => {
                eprintln!("Failed to load database from file: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        database::cleaner_database::get_default_database().clone()
    };

    // Load scripts and add custom cleaner data
    match run_scripts() {
        Ok(additional_data) => {
            database.extend(additional_data);
        }
        Err(e) => {
            eprintln!("Failed to run scripts: {}", e);
            // Continue without scripts
        }
    }
    #[cfg(windows)]
    let registry_database: Vec<CleanerDataRegistry> = {
        if let Some(db_path) = &args.registry_database_path {
            match database::registry_database::get_database_from_file(db_path) {
                Ok(db) => db,
                Err(e) => {
                    eprintln!("Failed to load database from file: {}", e);
                    std::process::exit(1);
                }
            }
        } else {
            database::registry_database::get_default_database().clone()
        }
    };

    // Use HashSet for O(1) lookups
    let mut options: HashSet<String> = HashSet::new();
    let mut programs: HashSet<String> = HashSet::new();

    for data in &database {
        options.insert(data.category.clone());
        programs.insert(data.program.clone());
    }

    #[cfg(windows)]
    {
        for data in &registry_database {
            options.insert(data.category.clone());
            programs.insert(data.program.clone());
        }
    }

    if args.show_database_info {
        println!(
            "DataBase programs: {}, DataBase paths: {}",
            programs.len(),
            database.len()
        );
    }

    let validator = |a: &[ListOption<&&str>]| {
        if a.is_empty() {
            Ok(Validation::Invalid("No category is selected!".into()))
        } else {
            Ok(Validation::Valid)
        }
    };

    let clear_categories: HashSet<String> = args
        .clear
        .as_ref()
        .map(|s| s.split(',').map(|x| x.trim().to_lowercase()).collect())
        .unwrap_or_default();

    let disabled_programs: HashSet<String> = args
        .disabled
        .as_ref()
        .map(|s| s.split(',').map(|x| x.trim().to_lowercase()).collect())
        .unwrap_or_default();

    if clear_categories.is_empty() && disabled_programs.is_empty() {
        let formatter_categories: MultiOptionFormatter<'_, &str> =
            &|a| format!("{} selected categories", a.len());

        let mut options_str: Vec<&str> = options.iter().map(|s| s.as_str()).collect();
        options_str.sort_unstable_by(|a, b| {
            let priority = |s: &str| match s {
                "Cache" => 0,
                "Logs" => 1,
                "Crashes" => 2,
                "Documentation" => 3,
                "Backups" => 4,
                "LastActivity" => 5,
                _ => 6,
            };

            let a_prio = priority(a);
            let b_prio = priority(b);

            if a_prio == b_prio {
                a.cmp(b)
            } else {
                a_prio.cmp(&b_prio)
            }
        });

        let ans_categories = MultiSelect::new("Select the clearing categories:", options_str)
            .with_validator(validator)
            .with_formatter(formatter_categories)
            .with_page_size(10)
            .prompt();

        if let Ok(ans_categories) = ans_categories {
            let ans_categories: Vec<String> =
                ans_categories.into_iter().map(|s| s.to_string()).collect();

            let mut programs2: Vec<&str> = database
                .iter()
                .filter(|data| ans_categories.contains(&data.category))
                .map(|data| data.program.as_str())
                .collect();

            #[cfg(windows)]
            {
                let registry_programs: Vec<&str> = registry_database
                    .iter()
                    .filter(|data| ans_categories.contains(&data.category))
                    .map(|data| data.program.as_str())
                    .collect();
                programs2.extend(registry_programs);
            }

            programs2 = programs2
                .into_iter()
                .collect::<HashSet<_>>()
                .into_iter()
                .collect();
            programs2.sort_unstable();

            let ans_programs =
                MultiSelect::new("Select the disabled programs for clearing:", programs2)
                    .with_formatter(formatter_categories)
                    .prompt();

            if let Ok(ans_programs) = ans_programs {
                work(
                    &args,
                    ans_programs.iter().map(|s| &**s).collect(),
                    ans_categories.iter().map(|s| s.to_lowercase()).collect(),
                    &database,
                    #[cfg(windows)]
                    &registry_database,
                )
                .await;
            }
        }
    } else {
        let ans_categories: Vec<String> =
            clear_categories.iter().map(|s| s.to_lowercase()).collect();
        let ans_programs: Vec<String> =
            disabled_programs.iter().map(|s| s.to_lowercase()).collect();

        work(
            &args,
            ans_programs.iter().map(|s| s.as_str()).collect(),
            ans_categories,
            &database,
            #[cfg(windows)]
            &registry_database,
        )
        .await;
    }

    // WARN: Windows only
    #[cfg(windows)]
    {
        let mut s = String::new();
        let _ = stdin().read_line(&mut s);
    }
}
