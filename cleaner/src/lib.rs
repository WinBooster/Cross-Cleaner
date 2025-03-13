use std::fs;
use std::path::Path;
use glob::{glob, Paths, PatternError};
use database::structures::{CleanerData, CleanerResult, Cleared};

pub fn clear_data(data: &CleanerData) -> CleanerResult {
    let mut cleaner_result: CleanerResult = CleanerResult {
        files: 0,
        folders: 0,
        bytes: 0,
        working: false,
        program: String::new(),
        path: String::new()
    };

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
                            match metadata {
                                Ok(res) => { lenght += res.len(); }
                                Err(_) => {}
                            }
                            match fs::remove_dir_all(file_path) {
                                Ok(_) => {
                                    cleaner_result.folders += 1;
                                    cleaner_result.working = true;
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
                        if data.remove_directory_after_clean {
                            match fs::remove_dir_all(path) {
                                Ok(_) => {
                                    cleaner_result.folders += 1;
                                    cleaner_result.working = true;
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