mod database;
mod structures;
mod registry_utils;

use std::fmt::{Debug};
use std::{env, fs};
use std::io::stdin;
use std::iter::Cloned;
use std::path::Path;
use std::slice::Iter;
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
use crate::structures::{CleanerData, CleanerResult, Cleared};

fn clear_category(data: &CleanerData) -> CleanerResult{
    let mut cleaner_result: CleanerResult = CleanerResult { files: 0, folders: 0, bytes: 0, working: false, program: "".parse().unwrap(), path: "".parse().unwrap() };
    let results: Result<Paths, PatternError> = glob(&*data.path);
    cleaner_result.program = (&*data.program).parse().unwrap();
    cleaner_result.path = (&*data.path).parse().unwrap();
    match results {
        Ok(results) => {
            for result in results {
                match result {
                    Ok(result) => {
                        let is_dir: bool = result.is_dir();
                        let is_file: bool = result.is_file();
                        let path: &str = result.as_path().to_str().unwrap();
                        let name: Option<&str> = result.file_name().unwrap().to_str();
                        let mut lenght = 0;
                        match result.metadata() {
                            Ok(res) => { lenght += res.len(); }
                            Err(_) => {}
                        }
                        //println!("Found: {}", path);
                        for file in &data.files_to_remove {
                            let file_path = path.to_owned() + "\\" + &*file;
                            match fs::remove_file(file_path) {
                                Ok(_) => {
                                    cleaner_result.files += 1;
                                    cleaner_result.bytes += lenght;
                                    cleaner_result.working = true;
                                }
                                Err(_) => {}
                            }
                        }
                        for directory in &data.directories_to_remove {
                            let file_path = path.to_owned() + "\\" + &*directory;
                            let metadata = fs::metadata(file_path.clone());
                            let mut name = String::new();
                            match metadata {
                                Ok(res) => { lenght += res.len(); }
                                Err(_) => {}
                            }
                            match fs::remove_dir_all(file_path) {
                                Ok(_) => {
                                    cleaner_result.folders += 1;
                                    cleaner_result.working = true;
                                    //println!("Removed file: {}", name.unwrap());
                                }
                                Err(_) => {}
                            }
                        }

                        for dir in &data.directories_to_remove {
                            let dir_path = path.to_owned() + "\\" + &*dir;
                            let metadata = fs::metadata(dir_path.clone());
                            match metadata {
                                Ok(res) => { lenght += res.len(); }
                                Err(_) => {}
                            }
                            match fs::remove_dir_all(dir_path) {
                                Ok(_) => {
                                    cleaner_result.folders += 1;
                                    cleaner_result.bytes += lenght;
                                    cleaner_result.working = true;
                                    //println!("Removed directory: {}", name.unwrap());
                                }
                                Err(_) => {}
                            }
                        }

                        //println!("Found: {}", path);
                        if data.remove_files && is_file {
                            let path = Path::new(path);
                            match fs::remove_file(path) {
                                Ok(_) => {
                                    cleaner_result.files += 1;
                                    cleaner_result.bytes += lenght;
                                    cleaner_result.working = true;

                                    //cleaner_result.files_vec.push(String::from(path.file_name()));
                                    //println!("Removed file: {}", name.unwrap());
                                }
                                Err(_) => {}
                            }
                        }
                        if data.remove_directories && is_dir {
                            match fs::remove_dir_all(path) {
                                Ok(_) => {
                                    cleaner_result.folders += 1;
                                    cleaner_result.bytes += lenght;
                                    cleaner_result.working = true;
                                    //println!("Removed directory: {}", name.unwrap());
                                }
                                Err(_) => {}
                            }
                        }
                        if data.remove_all_in_dir {
                            let results: Result<Paths, PatternError> = glob(&*(path.to_owned() + "\\*"));
                            let mut files = 0;
                            let mut dirs = 0;
                            match results {
                                Ok(results) => {
                                    for result in results {
                                        match result {
                                            Ok(result) => {
                                                if result.is_file() {
                                                    //cleaner_result.files_vec.push(String::from(result.file_name().unwrap().to_str()));
                                                    files += 1;
                                                }
                                                if result.is_dir() {
                                                    dirs += 1;
                                                }
                                            }
                                            Err(_) => {}
                                        }
                                    }
                                    match fs::remove_dir_all(path) {
                                        Ok(_) => {
                                            cleaner_result.files += files;
                                            cleaner_result.folders += dirs;
                                            cleaner_result.bytes += lenght;
                                            cleaner_result.working = true;
                                        }
                                        Err(_) => {}
                                    }
                                }
                                Err(_) => {}
                            }
                        }
                        if data.remove_directory_after_clean {
                            match fs::remove_dir_all(path) {
                                Ok(_) => {
                                    cleaner_result.folders += 1;
                                    cleaner_result.working = true;
                                    //println!("Removed directory: {}", name.unwrap());
                                }
                                Err(_) => {}
                            }
                        }
                    }
                    Err(_) => {}
                }
            }
        }
        Err(_) => {}
    }

    cleaner_result
}

fn get_file_size_string(size: u64) -> String {
    if size <= 0 {
        return "0 B".to_string();
    }

    let units = ["B", "KB", "MB", "GB", "TB"];
    let digit_groups = ((size as f64).log(1024.0)).floor() as usize;

    let size_in_units = size as f64 / 1024_f64.powi(digit_groups as i32);
    format!("{:.1} {}", size_in_units, units[digit_groups])
}

async fn work(categories: Vec<&str>, database: Vec<CleanerData>) {
    let sty = ProgressStyle::with_template(
        "[{elapsed_precise}] {prefix:.bold.dim} {spinner:.green}\n[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} [{msg}]",
    ).unwrap().progress_chars("##-").tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ");


    let mut bytes_cleared = 0;
    let mut removed_files = 0;
    let mut removed_directories = 0;
    let mut cleared_programs:Vec<Cleared> = vec![];

    let pb = ProgressBar::new(0);
    pb.set_style(sty.clone());
    pb.set_prefix("Clearing");

    let database2 = database.iter().to_owned();
    let async_list: Vec<_> = database2
        .filter(|data| categories.contains(&&*data.category))
        .map(|data| {
            let data = Arc::new(data.clone());
            let progress_bar = Arc::new(pb.clone());
            task::spawn(async move {
                progress_bar.set_message(format!("{}", data.path));
                let result = clear_category(&data);
                progress_bar.inc(1);
                result
            })
        })
        .collect();
    pb.set_length(async_list.len() as u64);
    for async_task in async_list {
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
}

#[tokio::main]
async fn main() {
    execute!(
        std::io::stdout(),
        crossterm::terminal::SetTitle("WinBooster CLI v1.8.4")
    );


    let database: Vec<CleanerData> = database::get_database();

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
        if a.len() < 1 {
            return Ok(Validation::Invalid("No category is selected!".into()));
        }
        else {
            return Ok(Validation::Valid);
        }
    };

    let mut ans = vec![];
    for argument in env::args() {
        if (options.contains(&&*argument)) {
            ans.push(argument);
        }
    }

    if ans.is_empty() {
        let formatter: MultiOptionFormatter<'_, &str> = &|a| format!("{} selected categories", a.len());
        let ans = MultiSelect::new("Select the clearing categories:", options)
            .with_validator(validator)
            .with_formatter(formatter)
            .prompt();

        if let Ok(ans) = ans {
            work(ans, database.clone()).await;
        }
    }
    else {
        let v2: Vec<&str> = ans.iter().map(|s| &**s).collect();
        work(v2, database.clone()).await;
    }

    let mut s= String::new();
    stdin().read_line(&mut s).expect("Did not enter a correct string");
}
