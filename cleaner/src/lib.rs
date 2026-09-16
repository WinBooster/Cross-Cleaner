use database::structures::{CleanerData, CleanerResult};
use futures::stream::{FuturesUnordered, StreamExt};
use glob::glob;
use std::future::Future;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use tokio::io;
use tokio::sync::Semaphore;

pub mod custom_cleaners;

// INFO: Re-export so macro_rules! ($crate::database::...) resolves in any consumer crate
pub use database;

// INFO: Safe join for untrusted names (from DB): only plain relative components.
// Rejects "..", ".", absolute paths, Windows prefixes (C:\, \\?\) and empty names.
fn safe_join(base: &Path, name: &str) -> Option<PathBuf> {
    let rel = Path::new(name);
    let mut out = base.to_path_buf();
    let mut any = false;
    for c in rel.components() {
        match c {
            Component::Normal(s) => {
                out.push(s);
                any = true;
            }
            _ => return None,
        }
    }
    if any { Some(out) } else { None }
}

// B: blocking fast helpers - use cap-std inside spawn_blocking
async fn remove_file_fast(path: PathBuf) -> io::Result<u64> {
    tokio::task::spawn_blocking(move || {
        let authority = cap_std::ambient_authority();
        let name = path
            .file_name()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no file name in path"))?;
        let parent = path.parent().unwrap_or_else(|| Path::new(""));
        let dir = if parent.as_os_str().is_empty() {
            cap_std::fs::Dir::open_ambient_dir(".", authority)?
        } else {
            cap_std::fs::Dir::open_ambient_dir(parent, authority)?
        };
        let meta = dir.symlink_metadata(&name)?;
        if meta.is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "symlink not supported",
            ));
        }
        if !meta.is_file() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a file"));
        }
        let len = meta.len();
        dir.remove_file(&name)?;
        Ok(len)
    })
    .await
    .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("join: {e}")))?
}

fn remove_dir_sync(root: PathBuf) -> io::Result<(u64, u64, u64)> {
    let authority = cap_std::ambient_authority();
    let name = root.file_name().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "cannot remove filesystem root")
    })?;
    let parent = root.parent().unwrap_or_else(|| Path::new("."));
    // Open the parent as a capability handle (dirfd on Unix, HANDLE on Windows).
    // Every further operation is resolved against open handles, so paths swapped
    // in mid-traversal cannot escape the parent's tree - this closes the TOCTOU
    // window that plain name-based std::fs calls have.
    let parent_dir = cap_std::fs::Dir::open_ambient_dir(parent, authority)?;
    // Refuse a symlinked root: check the final component with lstat semantics
    // before opening it relative to the parent handle.
    let meta = parent_dir.symlink_metadata(name)?;
    if meta.is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "symlink not supported",
        ));
    }
    let dir = parent_dir.open_dir(name)?;
    // Keep the handle open only while walking; on Windows a directory cannot
    // be removed while any handle to it is open (cap-std omits FILE_SHARE_DELETE).
    let (files, folders, bytes) = remove_dir_recursive(&dir)?;
    drop(dir);
    parent_dir.remove_dir(name)?; // root is now empty; counts itself
    Ok((files, folders + 1, bytes))
}

// INFO: Depth-first deletion relative to open handles. Entry types come from
// the handle (lstat semantics, never follows links). Symlinks and Windows
// junctions are removed as links; their targets are never touched.
fn remove_dir_recursive(dir: &cap_std::fs::Dir) -> io::Result<(u64, u64, u64)> {
    let mut files = 0u64;
    let mut folders = 0u64;
    let mut bytes = 0u64;

    for entry in dir.entries()? {
        let entry = entry?;
        let name = entry.file_name();
        let ft = entry.file_type()?;
        if ft.is_symlink() {
            // Never follow links. The removal syscall differs per platform:
            // - Windows: directory reparse points (junctions, dir symlinks) have
            //   FILE_ATTRIBUTE_DIRECTORY but FileType::is_dir() is false for them,
            //   so they must go through RemoveDirectory (removes the link itself).
            // - Unix: unlink removes any symlink, including symlink-to-dir.
            #[cfg(windows)]
            {
                // Junctions and dir symlinks are reparse points; DeleteFile
                // rejects them, RemoveDirectory removes the link itself.
                // Regular file symlinks go through DeleteFile.
                if dir.remove_file(&name).is_ok() {
                    files += 1;
                } else {
                    dir.remove_dir(&name)?;
                    folders += 1;
                }
            }
            #[cfg(not(windows))]
            {
                dir.remove_file(&name)?;
                files += 1;
            }
        } else if ft.is_dir() {
            let sub = dir.open_dir(&name)?;
            let (f, fo, b) = remove_dir_recursive(&sub)?;
            drop(sub); // release handle before removing (Windows FILE_SHARE_DELETE)
            files += f;
            folders += fo;
            bytes += b;
            dir.remove_dir(&name)?; // sub is now empty
            folders += 1;
        } else {
            bytes += entry.metadata()?.len();
            files += 1;
            dir.remove_file(&name)?;
        }
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
        sub_category: data.sub_category.clone(),
    };

    // INFO: Reject parent-dir traversal in the DB-supplied glob pattern
    if Path::new(&data.path)
        .components()
        .any(|c| matches!(c, Component::ParentDir))
    {
        return out;
    }

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
                sub_category: data.sub_category.clone(),
            };

            // A: parallel files_to_remove with C limit
            if !data.files_to_remove.is_empty() {
                let mut inner: FuturesUnordered<
                    std::pin::Pin<Box<dyn Future<Output = Option<u64>> + Send>>,
                > = FuturesUnordered::new();
                for fname in &data.files_to_remove {
                    let Some(fpath) = safe_join(&path, fname) else {
                        eprintln!(
                            "cleaner: skipping unsafe file name {:?} in {}",
                            fname, data.path
                        );
                        continue;
                    };
                    let sem2 = sem.clone();
                    inner.push(Box::pin(async move {
                        let _p = sem2.acquire_owned().await.unwrap();
                        match remove_file_fast(fpath.clone()).await {
                            Ok(b) => Some(b),
                            Err(e) => {
                                eprintln!("cleaner: remove_file {}: {}", fpath.display(), e);
                                None
                            }
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
                    let Some(dpath) = safe_join(&path, dname) else {
                        eprintln!(
                            "cleaner: skipping unsafe dir name {:?} in {}",
                            dname, data.path
                        );
                        continue;
                    };
                    let sem2 = sem.clone();
                    inner.push(Box::pin(async move {
                        let _p = sem2.acquire_owned().await.unwrap();
                        match remove_dir_fast(dpath.clone()).await {
                            Ok(v) => Some(v),
                            Err(e) => {
                                eprintln!("cleaner: remove_dir {}: {}", dpath.display(), e);
                                None
                            }
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
            sub_category: String::from("TestSub"),
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

    #[tokio::test]
    async fn test_clear_data_rejects_parent_traversal() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path().join("base");
        fs::create_dir(&base).unwrap();
        fs::write(base.join("ok.txt"), b"x").unwrap();

        let outside = temp_dir.path().join("outside");
        fs::create_dir(&outside).unwrap();
        let secret = outside.join("secret.txt");
        fs::write(&secret, b"secret").unwrap();

        let mut data = create_test_data(base.to_str().unwrap().to_string());
        data.files_to_remove = vec![String::from("../outside/secret.txt")];
        data.directories_to_remove = vec![String::from("../outside")];

        let result = clear_data(&data).await;

        assert!(!result.working);
        assert!(secret.exists());
        assert!(outside.exists());
        assert!(base.join("ok.txt").exists());
    }

    #[tokio::test]
    async fn test_clear_data_rejects_absolute_names() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path().join("base");
        fs::create_dir(&base).unwrap();

        let outside = temp_dir.path().join("outside");
        fs::create_dir(&outside).unwrap();
        let secret = outside.join("secret.txt");
        fs::write(&secret, b"secret").unwrap();

        let mut data = create_test_data(base.to_str().unwrap().to_string());
        data.files_to_remove = vec![secret.to_string_lossy().to_string()];
        data.directories_to_remove = vec![outside.to_string_lossy().to_string()];

        let result = clear_data(&data).await;

        assert!(!result.working);
        assert!(secret.exists());
        assert!(outside.exists());
    }

    #[tokio::test]
    async fn test_clear_data_rejects_parent_in_pattern() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path().join("base");
        fs::create_dir(&base).unwrap();
        let f = base.join("f.txt");
        fs::write(&f, b"x").unwrap();

        let pattern = format!(
            "{}/../base/*.txt",
            temp_dir.path().join("base").to_str().unwrap()
        );
        let mut data = create_test_data(pattern);
        data.remove_files = true;

        let result = clear_data(&data).await;

        assert!(!result.working);
        assert!(f.exists());
    }

    #[tokio::test]
    async fn test_clear_data_skips_dot_and_empty_names() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path().join("base");
        fs::create_dir(&base).unwrap();
        fs::write(base.join("ok.txt"), b"x").unwrap();

        let mut data = create_test_data(base.to_str().unwrap().to_string());
        data.files_to_remove = vec![String::from("."), String::from("")];

        let result = clear_data(&data).await;

        assert!(!result.working);
        assert!(base.join("ok.txt").exists());
    }

    // INFO: junction on Windows, symlink on Unix. The cleaner must remove the
    // link itself, never descend into or delete the target's contents.
    #[cfg(windows)]
    #[tokio::test]
    async fn test_clear_data_junction_inside_tree_not_followed() {
        use std::os::windows::fs::symlink_dir;

        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path().join("base");
        fs::create_dir(&base).unwrap();

        let target = temp_dir.path().join("target");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("valuable.txt"), b"keep").unwrap();

        let link = base.join("link");
        symlink_dir(&target, &link).unwrap();

        let mut data = create_test_data(base.to_str().unwrap().to_string());
        data.remove_all_in_dir = true;

        let result = clear_data(&data).await;

        assert!(result.working);
        assert!(target.join("valuable.txt").exists());
        assert!(!link.exists());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_clear_data_symlink_inside_tree_not_followed() {
        use std::os::unix::fs::symlink;

        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path().join("base");
        fs::create_dir(&base).unwrap();

        let target = temp_dir.path().join("target");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("valuable.txt"), b"keep").unwrap();

        let link = base.join("link");
        symlink(&target, &link).unwrap();

        let mut data = create_test_data(base.to_str().unwrap().to_string());
        data.remove_all_in_dir = true;

        let result = clear_data(&data).await;

        assert!(result.working);
        assert!(target.join("valuable.txt").exists());
        assert!(!link.exists());
    }

    // INFO: root itself is a link -> refuse instead of following it.
    #[cfg(windows)]
    #[tokio::test]
    async fn test_clear_data_refuses_junction_root() {
        use std::os::windows::fs::symlink_dir;

        let temp_dir = TempDir::new().unwrap();
        let target = temp_dir.path().join("target");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("valuable.txt"), b"keep").unwrap();

        let link = temp_dir.path().join("link");
        symlink_dir(&target, &link).unwrap();

        let mut data = create_test_data(link.to_str().unwrap().to_string());
        data.remove_all_in_dir = true;

        let result = clear_data(&data).await;

        assert!(!result.working);
        assert!(target.join("valuable.txt").exists());
        assert!(link.exists());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_clear_data_refuses_symlink_root() {
        use std::os::unix::fs::symlink;

        let temp_dir = TempDir::new().unwrap();
        let target = temp_dir.path().join("target");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("valuable.txt"), b"keep").unwrap();

        let link = temp_dir.path().join("link");
        symlink(&target, &link).unwrap();

        let mut data = create_test_data(link.to_str().unwrap().to_string());
        data.remove_all_in_dir = true;

        let result = clear_data(&data).await;

        assert!(!result.working);
        assert!(target.join("valuable.txt").exists());
        assert!(link.exists());
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
                sub_category: String::from("Test"),
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
                sub_category: String::from("Test"),
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
                sub_category: String::from("Test"),
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
                sub_category: String::from("Test"),
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
                sub_category: String::from("Test"),
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
                sub_category: String::from("Test"),
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
                sub_category: String::from("Test"),
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
                sub_category: String::from("Test"),
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
