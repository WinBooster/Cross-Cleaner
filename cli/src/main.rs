use clap::{ArgAction, Parser};
use cleaner::clear_data;
use crossterm::execute;
use database::get_pcbooster_version;
use database::structures::{CleanerData, CleanerResult, Cleared};
use database::utils::get_file_size_string;
use indicatif::{ProgressBar, ProgressStyle};
use inquire::formatter::MultiOptionFormatter;
use inquire::list_option::ListOption;
use inquire::validator::Validation;
use inquire::MultiSelect;
use notify_rust::Notification;
use std::collections::HashSet;
use std::sync::Arc;
use tabled::Table;
use tokio::sync::Mutex;
use tokio::task;

#[cfg(windows)]
use std::io::stdin;
use std::io::stdout;

async fn work(
    args: &Args,
    disabled_programs: Vec<&str>,
    categories: Vec<String>,
    database: &Vec<CleanerData>,
) {
    let bytes_cleared = Arc::new(Mutex::new(0));
    let removed_files = Arc::new(Mutex::new(0));
    let removed_directories = Arc::new(Mutex::new(0));
    let cleared_programs = Arc::new(Mutex::new(Vec::<Cleared>::new()));

    // Создаем прогресс-бар только если он включен
    let pb = if args.progress_bar {
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

    let has_last_activity = categories.contains(&"LastActivity".to_string());

    let mut tasks = Vec::new();

    if has_last_activity {
        let progress_bar = pb.clone();
        let task = task::spawn(async move {
            if let Some(ref pb) = progress_bar {
                pb.set_message("LastActivity");
            }
            #[cfg(windows)]
            database::registry_database::clear_last_activity();
            if let Some(ref pb) = progress_bar {
                pb.inc(1);
            }
            CleanerResult {
                files: 0,
                folders: 0,
                bytes: 0,
                working: false,
                path: String::new(),
                program: String::new(),
                category: String::new(),
            }
        });
        tasks.push(task);
    }

    for data in database
        .iter()
        .filter(|data| categories.contains(&data.category.to_lowercase()))
    {
        if disabled_programs.contains(&data.program.to_lowercase().as_str()) {
            continue;
        }

        let data = Arc::new(data.clone());
        let progress_bar = pb.clone();
        let bytes_cleared = Arc::clone(&bytes_cleared);
        let removed_files = Arc::clone(&removed_files);
        let removed_directories = Arc::clone(&removed_directories);
        let cleared_programs = Arc::clone(&cleared_programs);

        let task = task::spawn(async move {
            if let Some(ref pb) = progress_bar {
                pb.set_message(data.path.clone());
            }
            let result = clear_data(&data); // Убрали .await, если clear_data синхронная
            if let Some(ref pb) = progress_bar {
                pb.inc(1);
            }

            let mut bytes_cleared = bytes_cleared.lock().await;
            *bytes_cleared += result.bytes;

            let mut removed_files = removed_files.lock().await;
            *removed_files += result.files;

            let mut removed_directories = removed_directories.lock().await;
            *removed_directories += result.folders;

            if result.working {
                let mut cleared_programs = cleared_programs.lock().await;
                if let Some(cleared) = cleared_programs
                    .iter_mut()
                    .find(|c| c.program == result.program)
                {
                    cleared.removed_bytes += result.bytes as u64;
                    cleared.removed_files += result.files as u64;
                    cleared.removed_directories += result.folders as u64;
                    if !cleared.affected_categories.contains(&result.category) {
                        cleared.affected_categories.push(result.category.clone());
                    }
                } else {
                    let cleared = Cleared {
                        program: result.program.clone(),
                        removed_bytes: result.bytes as u64,
                        removed_files: result.files as u64,
                        removed_directories: result.folders as u64,
                        affected_categories: vec![result.category.clone()],
                    };
                    cleared_programs.push(cleared);
                }
            }

            result
        });
        tasks.push(task);
    }

    if let Some(ref pb) = &pb {
        pb.set_length(tasks.len() as u64);
    }

    for task in tasks {
        match task.await {
            Ok(_) => {}
            Err(e) => {
                eprintln!("Error waiting for task completion: {:?}", e);
            }
        }
    }

    if let Some(ref pb) = &pb {
        pb.set_message("done");
        pb.finish();
    }

    if args.result_table {
        println!("Cleared result:");
        let cleared_programs = cleared_programs.lock().await;
        let table = Table::new(cleared_programs.iter()).to_string();
        println!("{}", table);
    }
    if args.result_string {
        let bytes_cleared = bytes_cleared.lock().await;
        let removed_files = removed_files.lock().await;
        let removed_directories = removed_directories.lock().await;
        println!("Removed size: {}", get_file_size_string(*bytes_cleared));
        println!("Removed files: {}", *removed_files);
        println!("Removed directories: {}", *removed_directories);
    }

    if args.show_notification {
        if let Err(e) = Notification::new()
            .summary("Cross Cleaner CLI")
            .body(&format!(
                "Removed: {}\nFiles: {}",
                get_file_size_string(*bytes_cleared.lock().await),
                *removed_files.lock().await
            ))
            .show()
        {
            eprintln!("Failed to show notification: {:?}", e);
        }
    }
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Specify categories to clear (comma-separated)
    #[arg(long, value_name = "Categories")]
    clear: Option<String>,

    /// Specify programs to disable (comma-separated)
    #[arg(long, value_name = "Programs")]
    disabled: Option<String>,

    /// Show progress bar [default: true]
    #[arg(long, value_name = "Progress_bar", default_value_t = true)]
    progress_bar: bool,

    /// Show result table \[default: true\]
    #[arg(long, value_name = "Result_table", default_value_t = true)]
    result_table: bool,

    /// Show result string \[default: true\]
    #[arg(long, value_name = "Result_strings", default_value_t = true)]
    result_string: bool,

    /// Show notification \[default: true\]
    #[arg(long, value_name = "Notification", default_value_t = true)]
    show_notification: bool,
}

#[tokio::main]
async fn main() {
    execute!(
        stdout(),
        crossterm::terminal::SetTitle(format!("Cross Cleaner CLI v{}", get_pcbooster_version()))
    )
    .unwrap();

    let args = Args::parse();

    let database: &Vec<CleanerData> = database::cleaner_database::get_database();

    let mut options: HashSet<String> = HashSet::new();
    let mut programs: HashSet<String> = HashSet::new();

    for data in database.to_vec() {
        let program = data.program.clone();
        options.insert(String::from(data.category.clone()));
        programs.insert(program);
    }

    println!(
        "DataBase programs: {}, DataBase paths: {}",
        programs.len(),
        database.len()
    );

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

        let options_str: Vec<&str> = options.iter().map(|s| s.as_str()).collect();
        let ans_categories = MultiSelect::new("Select the clearing categories:", options_str)
            .with_validator(validator)
            .with_formatter(formatter_categories)
            .prompt();

        if let Ok(ans_categories) = ans_categories {
            let ans_categories: Vec<String> =
                ans_categories.into_iter().map(|s| s.to_string()).collect();

            let programs2: Vec<&str> = database
                .iter()
                .filter(|data| ans_categories.contains(&data.category))
                .map(|data| data.program.as_str())
                .collect::<HashSet<_>>()
                .into_iter()
                .collect();

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
        )
        .await;
    }

    #[cfg(windows)]
    {
        let mut s = String::new();
        let _ = stdin().read_line(&mut s);
    }
}
