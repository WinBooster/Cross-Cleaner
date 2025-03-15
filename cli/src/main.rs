use std::fmt::{Debug};
use std::{env, fs};
use std::io::stdin;
use std::path::Path;
use std::sync::Arc;
use crossterm::execute;
use glob::{glob, Paths, PatternError};
use inquire::formatter::MultiOptionFormatter;
use inquire::list_option::ListOption;
use inquire::MultiSelect;
use inquire::validator::Validation;
use tabled::{Table, Tabled};
use tokio::task;
use indicatif::{ProgressBar, ProgressStyle};
use notify_rust::Notification;
use cleaner::clear_data;
use database::registry_database;
use database::structures::{CleanerData, CleanerResult, Cleared};
use database::utils::get_file_size_string;

async fn work(disabledPrograms: Vec<&str>, categories: Vec<&str>, database: Vec<CleanerData>) {
    let sty = ProgressStyle::with_template(
        "[{elapsed_precise}] {prefix:.bold.dim} {spinner:.green}\n[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} [{msg}]",
    ).unwrap().progress_chars("##-").tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ");


    let cat2 = categories.clone();
    let mut bytes_cleared = 0;
    let mut removed_files = 0;
    let mut removed_directories = 0;
    let mut cleared_programs:Vec<Cleared> = vec![];

    let pb = ProgressBar::new(0);
    pb.set_style(sty.clone());
    pb.set_prefix("Clearing");

    let database2 = database.iter().to_owned();
    let database3 = database.iter().to_owned();

    let async_list: Vec<_> = database3
        .filter(|data| categories.contains(&&*"LastActivity"))
        .map(|data| {})
        .collect();

    let mut threads = vec![];

    let mut has_last_activity = !async_list.is_empty();

    let clear_last_activity_task = {
        let progress_bar = Arc::new(pb.clone());
        task::spawn(async move {
            let data = CleanerResult {
                files: 0,
                folders: 0,
                bytes: 0,
                working: false,
                path: String::new(),
                program: String::new(),
            };

            if has_last_activity {
                progress_bar.set_message("LastActivity");
                #[cfg(windows)]
                registry_database::clear_last_activity();
            }

            progress_bar.inc(1);
            data
        })
    };

    threads.push(clear_last_activity_task);

    let async_list: Vec<_> = database2
        .filter(|data| categories.contains(&&*data.category) && !disabledPrograms.contains(&&*data.program))
        .map(|data| {
            let data = Arc::new(data.clone());
            let progress_bar = Arc::new(pb.clone());
            task::spawn(async move {
                progress_bar.set_message(format!("{}", data.path));
                let result = clear_data(&data);
                progress_bar.inc(1);
                result
            })
        })
        .collect();
    threads.extend(async_list);

    pb.set_length(threads.len() as u64);

    for async_task in threads {
        match async_task.await {
            Ok(result) => {
                removed_files += result.files;
                removed_directories += result.folders;
                bytes_cleared += result.bytes;
                if result.working {
                    let data2 = Cleared { Program: result.program };
                    if !cleared_programs.contains(&data2) {
                        cleared_programs.push(data2);
                    }
                }
            },
            Err(_) => {
                eprintln!("Error waiting for task completion");
            }
        }
    }

    pb.set_message(format!("{}", "done"));
    pb.finish();

    println!("Cleared programs:");
    let table = Table::new(cleared_programs).to_string();
    println!("{}", table);
    println!("Removed: {}", get_file_size_string(bytes_cleared));
    println!("Removed files: {}", removed_files);
    println!("Removed directories: {}", removed_directories);

    let _ = Notification::new()
        .summary("WinBooster CLI")
        .body(&*("Removed: ".to_owned() + &*get_file_size_string(bytes_cleared) + "\nFiles: " + &*removed_files.to_string()))
        .icon("assets\\icon.png")
        .show();
}

#[tokio::main]
async fn main() {
    execute!(
        std::io::stdout(),
        crossterm::terminal::SetTitle("WinBooster Definitive Edition CLI v".to_owned() + &*database::get_winbooster_version())
    );


    let database: Vec<CleanerData> = database::cleaner_database::get_database();

    let mut options: Vec<&str> = vec![];
    let mut programs: Vec<&str> = vec![];

    for data in database.iter().clone() {
        if !options.contains(&&*data.category) {
            options.push(&*data.category);
        }
        if !programs.contains(&&*data.program) {
            programs.push(&*data.program);
        }

    }
    println!("DataBase Programs: {}", programs.iter().count());
    let validator = |a: &[ListOption<&&str>]| {
        return if a.len() < 1 {
            Ok(Validation::Invalid("No category is selected!".into()))
        } else {
            Ok(Validation::Valid)
        }
    };

    let mut ans = vec![];
    for argument in env::args() {
        if options.contains(&&*argument) {
            ans.push(argument);
        }
    }

    if ans.is_empty() {
        let formatter_categories: MultiOptionFormatter<'_, &str> = &|a| format!("{} selected categories", a.len());
        let ans_categories = MultiSelect::new("Select the clearing categories:", options)
            .with_validator(validator)
            .with_formatter(formatter_categories)
            .prompt();

        if let Ok(ans_categories) = ans_categories {
            let mut programs2 = vec![];

            for data in database.iter().clone() {
                if ans_categories.contains(&&*data.category) && !programs2.contains(&&*data.program) {
                    programs2.push(&*data.program);
                }
            }

            let ans_programs = MultiSelect::new("Select the disabled programs for clearing:", programs2)
                .with_formatter(formatter_categories)
                .prompt();

            if let Ok(ans_programs) = ans_programs {
                work(ans_programs, ans_categories, database.clone()).await;
            }
        }
    }
    else {
        let v2: Vec<&str> = ans.iter().map(|s| &**s).collect();
        work(vec![], v2, database.clone()).await;
    }

    let mut s= String::new();
    let _ = stdin().read_line(&mut s);
}
