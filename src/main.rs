mod database;

use std::ffi::{CString, OsStr};
use std::fmt::{format, Debug};
use std::fs;
use std::io::stdin;
use std::path::Path;
use std::sync::Arc;
use crossterm::execute;
use glob::{glob, GlobResult, Paths, PatternError};
use inquire::formatter::MultiOptionFormatter;
use inquire::list_option::ListOption;
use inquire::MultiSelect;
use inquire::validator::Validation;
use tabled::{Table, Tabled};
use tokio::task;

#[derive(PartialEq, Tabled)]
struct Cleared {
    Program: String,

}
impl PartialEq<Option<Cleared>> for &Cleared {
    fn eq(&self, other: &Option<Cleared>) -> bool {
        match other {
            Some(other) => return other.Program.eq(&*self.Program),
            None => return false,
        }
    }
}
#[derive(Clone)]
struct CleanerData {
    pub path: String,
    pub category: String,
    pub program: String,

    pub files_to_remove: Vec<String>,
    pub folders_to_remove: Vec<String>,
    pub directories_to_remove: Vec<String>,

    pub remove_all_in_dir: bool,
    pub remove_directory_after_clean: bool,
    pub remove_directories: bool,
    pub remove_files: bool
}
struct CleanerResult {
    pub files: u64,
    pub folders: u64,
    pub bytes: u64,
    pub working: bool,
    pub program: String
}

fn clear_category(data: &CleanerData) -> CleanerResult{
    let mut cleaner_result: CleanerResult = CleanerResult { files: 0, folders: 0, bytes: 0, working: false, program: "".parse().unwrap() };
    let results: Result<Paths, PatternError> = glob(&*data.path);
    cleaner_result.program = (&*data.program).parse().unwrap();
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
                                    //println!("Removed file: {}", name.unwrap());
                                }
                                Err(_) => {}
                            }
                        }
                        for directory in &data.directories_to_remove {
                            let file_path = path.to_owned() + "\\" + &*directory;
                            let metadata = fs::metadata(file_path.clone());
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
                            match fs::remove_dir(dir_path) {
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
                            match fs::remove_file(path) {
                                Ok(_) => {
                                    cleaner_result.files += 1;
                                    cleaner_result.bytes += lenght;
                                    cleaner_result.working = true;
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
                    }
                    Err(_) => {}
                }
            }
        }
        Err(_) => {}
    }
    return cleaner_result;
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


#[tokio::main]
async fn main() {
    execute!(
        std::io::stdout(),
        crossterm::terminal::SetTitle("WinBooster CLI v1.0.7.1")
    );
    let mut database: Vec<CleanerData> = database::get_database();

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

    let formatter: MultiOptionFormatter<'_, &str> = &|a| format!("{} selected categories", a.len());
    let ans = MultiSelect::new("Select the clearing categories:", options)
        .with_validator(validator)
        .with_formatter(formatter)
        .prompt();

    let mut bytes_cleared = 0;
    let mut removed_files = 0;
    let mut removed_directories = 0;
    let mut cleared_programs:Vec<Cleared> = vec![];

    let database2 = database.iter().cloned();
    if let Ok(ans) = ans {
        let async_list: Vec<_> = database2
            .filter(|data| ans.contains(&&*data.category)) // Убедитесь, что ans содержит правильные значения
            .map(|data| {
                let data = Arc::new(data.clone()); // Клонируем значение и оборачиваем в Arc
                task::spawn(async move {
                    clear_category(&data) // Предполагаем, что clear_category асинхронный
                })
            })
            .collect();

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
    }

    println!("Cleared programms:");
    let table = Table::new(cleared_programs).to_string();
    println!("{}", table);
    println!("Removed: {}", get_file_size_string(bytes_cleared));
    println!("Removed files: {}", removed_files);
    println!("Removed directories: {}", removed_directories);
    let mut s=String::new();
    stdin().read_line(&mut s).expect("Did not enter a correct string");




}
