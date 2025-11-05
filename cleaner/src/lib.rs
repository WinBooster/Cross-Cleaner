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

#[cfg(test)]
mod tests {
    use super::*;
    use database::structures::CleanerData;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_data(path: String) -> CleanerData {
        CleanerData {
            path,
            category: String::from("TestCategory"),
            program: String::from("TestProgram"),
            class: String::from("TestClass"),
            files_to_remove: vec![],
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
            remove_directories: false,
            remove_files: false,
        }
    }

    #[test]
    fn test_clear_data_nonexistent_path() {
        let data = create_test_data(String::from("/nonexistent/path/*"));
        let result = clear_data(&data);

        assert_eq!(result.files, 0);
        assert_eq!(result.folders, 0);
        assert_eq!(result.bytes, 0);
        assert!(!result.working);
    }

    #[test]
    fn test_clear_data_remove_files() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test_file.txt");
        fs::write(&file_path, b"test content").unwrap();

        let mut data = create_test_data(file_path.to_str().unwrap().to_string());
        data.remove_files = true;

        let result = clear_data(&data);

        assert!(result.working);
        assert_eq!(result.files, 1);
        assert!(result.bytes > 0);
        assert!(!file_path.exists());
    }

    #[test]
    fn test_clear_data_remove_directory() {
        let temp_dir = TempDir::new().unwrap();
        let sub_dir = temp_dir.path().join("sub_dir");
        fs::create_dir(&sub_dir).unwrap();
        fs::write(sub_dir.join("file.txt"), b"content").unwrap();

        let mut data = create_test_data(sub_dir.to_str().unwrap().to_string());
        data.remove_directories = true;

        let result = clear_data(&data);

        assert!(result.working);
        assert!(result.folders > 0);
        assert!(!sub_dir.exists());
    }

    #[test]
    fn test_clear_data_remove_all_in_dir() {
        let temp_dir = TempDir::new().unwrap();
        let target_dir = temp_dir.path().join("target");
        fs::create_dir(&target_dir).unwrap();
        fs::write(target_dir.join("file1.txt"), b"content1").unwrap();
        fs::write(target_dir.join("file2.txt"), b"content2").unwrap();

        let mut data = create_test_data(target_dir.to_str().unwrap().to_string());
        data.remove_all_in_dir = true;

        let result = clear_data(&data);

        assert!(result.working);
        assert!(result.files >= 2);
        assert!(!target_dir.exists());
    }

    #[test]
    fn test_clear_data_specific_files() {
        let temp_dir = TempDir::new().unwrap();
        let target_dir = temp_dir.path().join("target");
        fs::create_dir(&target_dir).unwrap();
        fs::write(target_dir.join("remove_me.tmp"), b"temp").unwrap();
        fs::write(target_dir.join("keep_me.txt"), b"keep").unwrap();

        let mut data = create_test_data(target_dir.to_str().unwrap().to_string());
        data.files_to_remove = vec![String::from("remove_me.tmp")];

        let result = clear_data(&data);

        assert!(result.working);
        assert_eq!(result.files, 1);
        assert!(!target_dir.join("remove_me.tmp").exists());
        assert!(target_dir.join("keep_me.txt").exists());
    }

    #[test]
    fn test_clear_data_specific_directories() {
        let temp_dir = TempDir::new().unwrap();
        let target_dir = temp_dir.path().join("target");
        fs::create_dir(&target_dir).unwrap();

        let remove_dir = target_dir.join("cache");
        fs::create_dir(&remove_dir).unwrap();
        fs::write(remove_dir.join("cache_file.txt"), b"cache").unwrap();

        let keep_dir = target_dir.join("data");
        fs::create_dir(&keep_dir).unwrap();

        let mut data = create_test_data(target_dir.to_str().unwrap().to_string());
        data.directories_to_remove = vec![String::from("cache")];

        let result = clear_data(&data);

        assert!(result.working);
        assert!(result.folders >= 1);
        assert!(!remove_dir.exists());
        assert!(keep_dir.exists());
    }

    #[test]
    fn test_clear_data_glob_pattern() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("file1.tmp"), b"temp1").unwrap();
        fs::write(temp_dir.path().join("file2.tmp"), b"temp2").unwrap();
        fs::write(temp_dir.path().join("file3.txt"), b"text").unwrap();

        let pattern = format!("{}/*.tmp", temp_dir.path().to_str().unwrap());
        let mut data = create_test_data(pattern);
        data.remove_files = true;

        let result = clear_data(&data);

        assert!(result.working);
        assert_eq!(result.files, 2);
        assert!(!temp_dir.path().join("file1.tmp").exists());
        assert!(!temp_dir.path().join("file2.tmp").exists());
        assert!(temp_dir.path().join("file3.txt").exists());
    }

    #[test]
    fn test_clear_data_nested_directories() {
        let temp_dir = TempDir::new().unwrap();
        let nested = temp_dir.path().join("level1").join("level2").join("level3");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("deep_file.txt"), b"deep content").unwrap();

        let mut data =
            create_test_data(temp_dir.path().join("level1").to_str().unwrap().to_string());
        data.remove_directories = true;

        let result = clear_data(&data);

        assert!(result.working);
        assert!(result.folders >= 3);
        assert!(result.files >= 1);
    }

    #[test]
    fn test_clear_data_result_fields() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, b"test").unwrap();

        let mut data = create_test_data(file_path.to_str().unwrap().to_string());
        data.remove_files = true;

        let result = clear_data(&data);

        assert_eq!(result.program, "TestProgram");
        assert_eq!(result.category, "TestCategory");
        assert_eq!(result.path, file_path.to_str().unwrap());
        assert!(result.working);
    }

    #[test]
    fn test_clear_data_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let empty_dir = temp_dir.path().join("empty");
        fs::create_dir(&empty_dir).unwrap();

        let mut data = create_test_data(empty_dir.to_str().unwrap().to_string());
        data.remove_directories = true;

        let result = clear_data(&data);

        assert!(result.working);
        assert_eq!(result.folders, 1);
        assert_eq!(result.files, 0);
    }

    #[test]
    fn test_clear_data_byte_counting() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("sized_file.txt");
        let content = b"0123456789"; // 10 bytes
        fs::write(&file_path, content).unwrap();

        let mut data = create_test_data(file_path.to_str().unwrap().to_string());
        data.remove_files = true;

        let result = clear_data(&data);

        assert_eq!(result.bytes, 10);
    }

    #[test]
    fn test_clear_data_multiple_operations() {
        let temp_dir = TempDir::new().unwrap();
        let target_dir = temp_dir.path().join("multi_test");
        fs::create_dir(&target_dir).unwrap();

        // Create files to remove by name
        fs::write(target_dir.join("temp.tmp"), b"temp").unwrap();

        // Create directory to remove by name
        let cache_dir = target_dir.join("cache");
        fs::create_dir(&cache_dir).unwrap();
        fs::write(cache_dir.join("cache.dat"), b"cache").unwrap();

        let mut data = create_test_data(target_dir.to_str().unwrap().to_string());
        data.files_to_remove = vec![String::from("temp.tmp")];
        data.directories_to_remove = vec![String::from("cache")];

        let result = clear_data(&data);

        assert!(result.working);
        assert!(result.files >= 2); // temp.tmp + cache.dat
        assert!(result.folders >= 1); // cache dir
        assert!(!target_dir.join("temp.tmp").exists());
        assert!(!cache_dir.exists());
    }
}

// Property-based tests with proptest
#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;
    use tempfile::TempDir;

    proptest! {
        /// Property: byte counting should always match actual file sizes
        #[test]
        fn prop_byte_counting_accurate(content in prop::collection::vec(any::<u8>(), 0..1000)) {
            let temp_dir = TempDir::new().unwrap();
            let file_path = temp_dir.path().join("test_file.bin");
            fs::write(&file_path, &content).unwrap();

            let data = CleanerData {
                path: file_path.to_str().unwrap().to_string(),
                category: String::from("Test"),
                program: String::from("Test"),
                class: String::from("Test"),
                files_to_remove: vec![],
                directories_to_remove: vec![],
                remove_all_in_dir: false,
                remove_directory_after_clean: false,
                remove_directories: false,
                remove_files: true,
            };

            let result = clear_data(&data);
            prop_assert_eq!(result.bytes, content.len() as u64);
        }

        /// Property: file counter should match number of files deleted
        #[test]
        fn prop_file_counter_accurate(num_files in 1usize..50) {
            let temp_dir = TempDir::new().unwrap();
            let target_dir = temp_dir.path().join("files");
            fs::create_dir(&target_dir).unwrap();

            for i in 0..num_files {
                fs::write(target_dir.join(format!("file_{}.txt", i)), b"content").unwrap();
            }

            let pattern = format!("{}/*.txt", target_dir.to_str().unwrap());
            let data = CleanerData {
                path: pattern,
                category: String::from("Test"),
                program: String::from("Test"),
                class: String::from("Test"),
                files_to_remove: vec![],
                directories_to_remove: vec![],
                remove_all_in_dir: false,
                remove_directory_after_clean: false,
                remove_directories: false,
                remove_files: true,
            };

            let result = clear_data(&data);
            prop_assert_eq!(result.files, num_files as u64);
        }

        /// Property: clearing non-existent path should always be safe
        #[test]
        fn prop_nonexistent_path_safe(path in "[a-z]{1,20}/[a-z]{1,20}") {
            let non_existent = format!("/tmp/nonexistent_{}/file.txt", path);
            let data = CleanerData {
                path: non_existent,
                category: String::from("Test"),
                program: String::from("Test"),
                class: String::from("Test"),
                files_to_remove: vec![],
                directories_to_remove: vec![],
                remove_all_in_dir: false,
                remove_directory_after_clean: false,
                remove_directories: false,
                remove_files: true,
            };

            let result = clear_data(&data);
            prop_assert!(!result.working);
            prop_assert_eq!(result.files, 0);
            prop_assert_eq!(result.folders, 0);
            prop_assert_eq!(result.bytes, 0);
        }

        /// Property: removing empty directories should work
        #[test]
        fn prop_empty_directory_removal(num_dirs in 1usize..20) {
            let temp_dir = TempDir::new().unwrap();

            for i in 0..num_dirs {
                let dir = temp_dir.path().join(format!("empty_dir_{}", i));
                fs::create_dir(&dir).unwrap();
            }

            let pattern = format!("{}/*", temp_dir.path().to_str().unwrap());
            let data = CleanerData {
                path: pattern,
                category: String::from("Test"),
                program: String::from("Test"),
                class: String::from("Test"),
                files_to_remove: vec![],
                directories_to_remove: vec![],
                remove_all_in_dir: false,
                remove_directory_after_clean: false,
                remove_directories: true,
                remove_files: false,
            };

            let result = clear_data(&data);
            prop_assert_eq!(result.folders, num_dirs as u64);
            prop_assert_eq!(result.files, 0);
        }

        /// Property: result should always have correct program/category
        #[test]
        fn prop_result_metadata(program in "[A-Za-z]{3,20}", category in "[A-Za-z]{3,20}") {
            let temp_dir = TempDir::new().unwrap();
            let file_path = temp_dir.path().join("test.txt");
            fs::write(&file_path, b"test").unwrap();

            let data = CleanerData {
                path: file_path.to_str().unwrap().to_string(),
                category: category.clone(),
                program: program.clone(),
                class: String::from("Test"),
                files_to_remove: vec![],
                directories_to_remove: vec![],
                remove_all_in_dir: false,
                remove_directory_after_clean: false,
                remove_directories: false,
                remove_files: true,
            };

            let result = clear_data(&data);
            prop_assert_eq!(result.program, program);
            prop_assert_eq!(result.category, category);
        }

        /// Property: nested directory deletion should count all subdirectories
        #[test]
        fn prop_nested_directory_counting(depth in 1usize..5) {
            let temp_dir = TempDir::new().unwrap();
            let mut current = temp_dir.path().join("level_0");
            fs::create_dir(&current).unwrap();

            for i in 1..depth {
                current = current.join(format!("level_{}", i));
                fs::create_dir(&current).unwrap();
            }

            let start_dir = temp_dir.path().join("level_0");
            let data = CleanerData {
                path: start_dir.to_str().unwrap().to_string(),
                category: String::from("Test"),
                program: String::from("Test"),
                class: String::from("Test"),
                files_to_remove: vec![],
                directories_to_remove: vec![],
                remove_all_in_dir: false,
                remove_directory_after_clean: false,
                remove_directories: true,
                remove_files: false,
            };

            let result = clear_data(&data);
            prop_assert_eq!(result.folders, depth as u64);
        }

        /// Property: specific file removal should only remove specified files
        #[test]
        fn prop_specific_file_removal(filename in "[a-z]{3,10}\\.(txt|tmp|log)") {
            let temp_dir = TempDir::new().unwrap();
            let target_dir = temp_dir.path().join("target");
            fs::create_dir(&target_dir).unwrap();

            // Create the target file
            fs::write(target_dir.join(&filename), b"remove").unwrap();
            // Create other files
            fs::write(target_dir.join("keep1.txt"), b"keep").unwrap();
            fs::write(target_dir.join("keep2.txt"), b"keep").unwrap();

            let data = CleanerData {
                path: target_dir.to_str().unwrap().to_string(),
                category: String::from("Test"),
                program: String::from("Test"),
                class: String::from("Test"),
                files_to_remove: vec![filename.clone()],
                directories_to_remove: vec![],
                remove_all_in_dir: false,
                remove_directory_after_clean: false,
                remove_directories: false,
                remove_files: false,
            };

            let result = clear_data(&data);
            prop_assert_eq!(result.files, 1);
            prop_assert!(!target_dir.join(&filename).exists());
            prop_assert!(target_dir.join("keep1.txt").exists());
            prop_assert!(target_dir.join("keep2.txt").exists());
        }

        /// Property: total bytes should equal sum of all file sizes
        #[test]
        fn prop_total_bytes_sum(file_sizes in prop::collection::vec(0u64..10000, 1..10)) {
            let temp_dir = TempDir::new().unwrap();
            let target_dir = temp_dir.path().join("bytes_test");
            fs::create_dir(&target_dir).unwrap();

            let mut expected_bytes = 0u64;
            for (i, size) in file_sizes.iter().enumerate() {
                let content = vec![0u8; *size as usize];
                fs::write(target_dir.join(format!("file_{}.dat", i)), &content).unwrap();
                expected_bytes += size;
            }

            let pattern = format!("{}/*.dat", target_dir.to_str().unwrap());
            let data = CleanerData {
                path: pattern,
                category: String::from("Test"),
                program: String::from("Test"),
                class: String::from("Test"),
                files_to_remove: vec![],
                directories_to_remove: vec![],
                remove_all_in_dir: false,
                remove_directory_after_clean: false,
                remove_directories: false,
                remove_files: true,
            };

            let result = clear_data(&data);
            prop_assert_eq!(result.bytes, expected_bytes);
            prop_assert_eq!(result.files, file_sizes.len() as u64);
        }
    }
}
