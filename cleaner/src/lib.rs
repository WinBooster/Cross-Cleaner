use database::structures::{CleanerData, CleanerResult};
use futures::stream::{FuturesUnordered, StreamExt};
use glob::glob;
use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io;
use tokio::sync::Semaphore;

// B: blocking fast helpers - use std::fs inside spawn_blocking
async fn remove_file_fast(path: PathBuf) -> io::Result<u64> {
    tokio::task::spawn_blocking(move || {
        let meta = std::fs::metadata(&path)?;
        if !meta.is_file() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a file"));
        }
        let len = meta.len();
        std::fs::remove_file(&path)?;
        Ok(len)
    })
    .await
    .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("join: {e}")))?
}

fn remove_dir_sync(root: PathBuf) -> io::Result<(u64, u64, u64)> {
    let meta = std::fs::metadata(&root)?;
    if !meta.is_dir() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a dir"));
    }
    let mut files = 0u64;
    let mut folders = 0u64;
    let mut bytes = 0u64;
    let mut stack = vec![root];
    let mut to_delete: Vec<PathBuf> = Vec::new();

    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir)?;
        for e in entries {
            let e = e?;
            let p = e.path();
            let m = e.metadata()?;
            if m.is_dir() {
                stack.push(p);
            } else {
                bytes += m.len();
                files += 1;
                std::fs::remove_file(&p)?;
            }
        }
        to_delete.push(dir);
    }
    for d in to_delete.iter().rev() {
        std::fs::remove_dir(d)?;
        folders += 1;
    }
    Ok((files, folders, bytes))
}

async fn remove_dir_fast(path: PathBuf) -> io::Result<(u64, u64, u64)> {
    tokio::task::spawn_blocking(move || remove_dir_sync(path))
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("join: {e}")))?
}

async fn remove_dir_all_fast(path: PathBuf) -> io::Result<()> {
    tokio::task::spawn_blocking(move || std::fs::remove_dir_all(&path))
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("join: {e}")))?
}

// NOTE: The main function for data cleansing.
// PERF: A (parallel intra-entry) + B (spawn_blocking) + C (Semaphore 32)
pub async fn clear_data(data: &CleanerData) -> CleanerResult {
    let mut out = CleanerResult {
        files: 0,
        folders: 0,
        bytes: 0,
        working: false,
        program: data.program.clone(),
        path: data.path.clone(),
        category: data.category.clone(),
    };

    let paths: Vec<PathBuf> = match glob(&data.path) {
        Ok(g) => g.filter_map(Result::ok).collect(),
        Err(_) => return out,
    };
    if paths.is_empty() {
        return out;
    }

    // C: global limit inside cleaner
    let sem = Arc::new(Semaphore::new(32));
    let mut path_futs: FuturesUnordered<
        std::pin::Pin<Box<dyn Future<Output = CleanerResult> + Send>>,
    > = FuturesUnordered::new();

    for path in paths {
        let data = data.clone();
        let sem = sem.clone();
        path_futs.push(Box::pin(async move {
            let mut local = CleanerResult {
                files: 0,
                folders: 0,
                bytes: 0,
                working: false,
                path: path.to_string_lossy().to_string(),
                program: data.program.clone(),
                category: data.category.clone(),
            };

            // A: parallel files_to_remove with C limit
            if !data.files_to_remove.is_empty() {
                let mut inner: FuturesUnordered<
                    std::pin::Pin<Box<dyn Future<Output = Option<u64>> + Send>>,
                > = FuturesUnordered::new();
                for fname in &data.files_to_remove {
                    let fpath = path.join(fname);
                    let sem2 = sem.clone();
                    inner.push(Box::pin(async move {
                        let _p = sem2.acquire_owned().await.unwrap();
                        match remove_file_fast(fpath).await {
                            Ok(b) => Some(b),
                            Err(_) => None,
                        }
                    }));
                }
                while let Some(opt) = inner.next().await {
                    if let Some(b) = opt {
                        local.files += 1;
                        local.bytes += b;
                        local.working = true;
                    }
                }
            }

            // A: parallel directories_to_remove
            if !data.directories_to_remove.is_empty() {
                let mut inner: FuturesUnordered<
                    std::pin::Pin<Box<dyn Future<Output = Option<(u64, u64, u64)>> + Send>>,
                > = FuturesUnordered::new();
                for dname in &data.directories_to_remove {
                    let dpath = path.join(dname);
                    let sem2 = sem.clone();
                    inner.push(Box::pin(async move {
                        let _p = sem2.acquire_owned().await.unwrap();
                        match remove_dir_fast(dpath).await {
                            Ok(v) => Some(v),
                            Err(_) => None,
                        }
                    }));
                }
                while let Some(opt) = inner.next().await {
                    if let Some((f, fo, b)) = opt {
                        local.files += f;
                        local.folders += fo;
                        local.bytes += b;
                        local.working = true;
                    }
                }
            }

            // remove_all_in_dir - single, needs semaphore
            if data.remove_all_in_dir {
                let sem2 = sem.clone();
                let _p = sem2.acquire_owned().await.unwrap();
                // try fast; skip is_dir check for speed (A)
                if let Ok((f, fo, b)) = remove_dir_fast(path.clone()).await {
                    local.files += f;
                    local.folders += fo;
                    local.bytes += b;
                    local.working = true;
                    // path now gone, following ops will quickly fail (NotFound) - keep for semantics
                }
            }

            // remove_files (path itself is file)
            if data.remove_files {
                let sem2 = sem.clone();
                let _p = sem2.acquire_owned().await.unwrap();
                if let Ok(b) = remove_file_fast(path.clone()).await {
                    local.files += 1;
                    local.bytes += b;
                    local.working = true;
                }
            }

            // remove_directories
            if data.remove_directories {
                let sem2 = sem.clone();
                let _p = sem2.acquire_owned().await.unwrap();
                if let Ok((f, fo, b)) = remove_dir_fast(path.clone()).await {
                    local.files += f;
                    local.folders += fo;
                    local.bytes += b;
                    local.working = true;
                }
            }

            // remove_directory_after_clean - B via spawn_blocking, no counting bytes/files
            if data.remove_directory_after_clean {
                let sem2 = sem.clone();
                let _p = sem2.acquire_owned().await.unwrap();
                if remove_dir_all_fast(path.clone()).await.is_ok() {
                    local.folders += 1;
                    local.working = true;
                }
            }

            local
        }));
    }

    while let Some(partial) = path_futs.next().await {
        if partial.working {
            out.working = true;
            out.files += partial.files;
            out.folders += partial.folders;
            out.bytes += partial.bytes;
        }
    }

    out
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

    #[tokio::test]
    async fn test_clear_data_nonexistent_path() {
        let data = create_test_data(String::from("/nonexistent/path/*"));
        let result = clear_data(&data).await;

        assert_eq!(result.files, 0);
        assert_eq!(result.folders, 0);
        assert_eq!(result.bytes, 0);
        assert!(!result.working);
    }

    #[tokio::test]
    async fn test_clear_data_remove_files() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test_file.txt");
        fs::write(&file_path, b"test content").unwrap();

        let mut data = create_test_data(file_path.to_str().unwrap().to_string());
        data.remove_files = true;

        let result = clear_data(&data).await;

        assert!(result.working);
        assert_eq!(result.files, 1);
        assert!(result.bytes > 0);
        assert!(!file_path.exists());
    }

    #[tokio::test]
    async fn test_clear_data_remove_directory() {
        let temp_dir = TempDir::new().unwrap();
        let sub_dir = temp_dir.path().join("sub_dir");
        fs::create_dir(&sub_dir).unwrap();
        fs::write(sub_dir.join("file.txt"), b"content").unwrap();

        let mut data = create_test_data(sub_dir.to_str().unwrap().to_string());
        data.remove_directories = true;

        let result = clear_data(&data).await;

        assert!(result.working);
        assert!(result.folders > 0);
        assert!(!sub_dir.exists());
    }

    #[tokio::test]
    async fn test_clear_data_remove_all_in_dir() {
        let temp_dir = TempDir::new().unwrap();
        let target_dir = temp_dir.path().join("target");
        fs::create_dir(&target_dir).unwrap();
        fs::write(target_dir.join("file1.txt"), b"content1").unwrap();
        fs::write(target_dir.join("file2.txt"), b"content2").unwrap();

        let mut data = create_test_data(target_dir.to_str().unwrap().to_string());
        data.remove_all_in_dir = true;

        let result = clear_data(&data).await;

        assert!(result.working);
        assert!(result.files >= 2);
        assert!(!target_dir.exists());
    }

    #[tokio::test]
    async fn test_clear_data_specific_files() {
        let temp_dir = TempDir::new().unwrap();
        let target_dir = temp_dir.path().join("target");
        fs::create_dir(&target_dir).unwrap();
        fs::write(target_dir.join("remove_me.tmp"), b"temp").unwrap();
        fs::write(target_dir.join("keep_me.txt"), b"keep").unwrap();

        let mut data = create_test_data(target_dir.to_str().unwrap().to_string());
        data.files_to_remove = vec![String::from("remove_me.tmp")];

        let result = clear_data(&data).await;

        assert!(result.working);
        assert_eq!(result.files, 1);
        assert!(!target_dir.join("remove_me.tmp").exists());
        assert!(target_dir.join("keep_me.txt").exists());
    }

    #[tokio::test]
    async fn test_clear_data_specific_directories() {
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

        let result = clear_data(&data).await;

        assert!(result.working);
        assert!(result.folders >= 1);
        assert!(!remove_dir.exists());
        assert!(keep_dir.exists());
    }

    #[tokio::test]
    async fn test_clear_data_glob_pattern() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("file1.tmp"), b"temp1").unwrap();
        fs::write(temp_dir.path().join("file2.tmp"), b"temp2").unwrap();
        fs::write(temp_dir.path().join("file3.txt"), b"text").unwrap();

        let pattern = format!("{}/*.tmp", temp_dir.path().to_str().unwrap());
        let mut data = create_test_data(pattern);
        data.remove_files = true;

        let result = clear_data(&data).await;

        assert!(result.working);
        assert_eq!(result.files, 2);
        assert!(!temp_dir.path().join("file1.tmp").exists());
        assert!(!temp_dir.path().join("file2.tmp").exists());
        assert!(temp_dir.path().join("file3.txt").exists());
    }

    #[tokio::test]
    async fn test_clear_data_nested_directories() {
        let temp_dir = TempDir::new().unwrap();
        let nested = temp_dir.path().join("level1").join("level2").join("level3");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("deep_file.txt"), b"deep content").unwrap();

        let mut data =
            create_test_data(temp_dir.path().join("level1").to_str().unwrap().to_string());
        data.remove_directories = true;

        let result = clear_data(&data).await;

        assert!(result.working);
        assert!(result.folders >= 3);
        assert!(result.files >= 1);
    }

    #[tokio::test]
    async fn test_clear_data_result_fields() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, b"test").unwrap();

        let mut data = create_test_data(file_path.to_str().unwrap().to_string());
        data.remove_files = true;

        let result = clear_data(&data).await;

        assert_eq!(result.program, "TestProgram");
        assert_eq!(result.category, "TestCategory");
        assert_eq!(result.path, file_path.to_str().unwrap());
        assert!(result.working);
    }

    #[tokio::test]
    async fn test_clear_data_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let empty_dir = temp_dir.path().join("empty");
        fs::create_dir(&empty_dir).unwrap();

        let mut data = create_test_data(empty_dir.to_str().unwrap().to_string());
        data.remove_directories = true;

        let result = clear_data(&data).await;

        assert!(result.working);
        assert_eq!(result.folders, 1);
        assert_eq!(result.files, 0);
    }

    #[tokio::test]
    async fn test_clear_data_byte_counting() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("sized_file.txt");
        let content = b"0123456789"; // 10 bytes
        fs::write(&file_path, content).unwrap();

        let mut data = create_test_data(file_path.to_str().unwrap().to_string());
        data.remove_files = true;

        let result = clear_data(&data).await;

        assert_eq!(result.bytes, 10);
    }

    #[tokio::test]
    async fn test_clear_data_multiple_operations() {
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

        let result = clear_data(&data).await;

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
    use std::fs;
    use tempfile::TempDir;

    // helper to run async clear_data inside sync proptest
    fn run_async<F, T>(f: F) -> T
    where
        F: std::future::Future<Output = T>,
    {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(f)
    }

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

            let result = run_async(clear_data(&data));
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

            let result = run_async(clear_data(&data));
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

            let result = run_async(clear_data(&data));
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

            let result = run_async(clear_data(&data));
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

            let result = run_async(clear_data(&data));
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

            let result = run_async(clear_data(&data));
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

            let result = run_async(clear_data(&data));
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

            let result = run_async(clear_data(&data));
            prop_assert_eq!(result.bytes, expected_bytes);
            prop_assert_eq!(result.files, file_sizes.len() as u64);
        }
    }
}
