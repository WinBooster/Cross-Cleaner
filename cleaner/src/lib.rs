use database::structures::{CleanerData, CleanerResult};
use glob::glob;
use std::path::Path;
use std::{fs, io};

// NOTE: Recursively deletes the directory and updates the counters in `cleaner_result`.
// PERF: Fully optimized
fn remove_directory_recursive(path: &Path, cleaner_result: &mut CleanerResult) -> io::Result<()> {
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let entry_path = entry.path();
            if entry_path.is_dir() {
                remove_directory_recursive(&entry_path, cleaner_result)?;
            } else {
                remove_file(&entry_path, cleaner_result)?;
            }
        }

        fs::remove_dir(path)?;
        cleaner_result.folders += 1;
    } else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "The provided path is not a directory",
        ));
    }

    Ok(())
}
// NOTE: Deletes the file and updates the counters in `cleaner_result`.
// PERF: Fully optimized
fn remove_file(path: &Path, cleaner_result: &mut CleanerResult) -> io::Result<()> {
    let metadata = fs::metadata(path)?;
    fs::remove_file(path)?;

    cleaner_result.bytes += metadata.len();
    cleaner_result.files += 1;

    Ok(())
}

// NOTE: The main function for data cleansing.
// PERF: Fully optimized
pub fn clear_data(data: &CleanerData) -> CleanerResult {
    let mut cleaner_result = CleanerResult {
        files: 0,
        folders: 0,
        bytes: 0,
        working: false,
        program: data.program.clone(),
        path: data.path.clone(),
        category: data.category.clone(),
    };

    // NOTE: Use glob to search for files and directories
    if let Ok(results) = glob(&data.path) {
        for result in results.flatten() {
            let path = result.as_path();
            let is_dir = path.is_dir();
            let is_file = path.is_file();

            // NOTE: Deleting specified files
            for file in &data.files_to_remove {
                let file_path = path.join(file);
                if file_path.exists()
                    && file_path.is_file()
                    && remove_file(&file_path, &mut cleaner_result).is_ok()
                {
                    cleaner_result.working = true;
                }
            }

            // NOTE: Deleting specified directories
            for dir in &data.directories_to_remove {
                let dir_path = path.join(dir);
                if dir_path.exists()
                    && dir_path.is_dir()
                    && remove_directory_recursive(&dir_path, &mut cleaner_result).is_ok()
                {
                    cleaner_result.working = true;
                }
            }

            // NOTE: Deleting all files and directories if required
            if data.remove_all_in_dir
                && is_dir
                && remove_directory_recursive(path, &mut cleaner_result).is_ok()
            {
                cleaner_result.working = true;
            }

            // NOTE: Deleting files if required
            if data.remove_files && is_file && remove_file(path, &mut cleaner_result).is_ok() {
                cleaner_result.working = true;
            }

            // NOTE: Deleting directories if required
            if data.remove_directories
                && is_dir
                && remove_directory_recursive(path, &mut cleaner_result).is_ok()
            {
                cleaner_result.working = true;
            }

            // NOTE: Deleting a directory after cleaning, if required
            if data.remove_directory_after_clean && is_dir && fs::remove_dir_all(path).is_ok() {
                cleaner_result.folders += 1;
                cleaner_result.working = true;
            }
        }
    }

    cleaner_result
}
