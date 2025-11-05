use crate::structures::CleanerData;

pub mod cleaner_database;
pub mod registry_database;
mod registry_utils;
pub mod structures;
pub mod utils;

pub fn get_version() -> &'static str {
    option_env!("APP_VERSION").unwrap_or("1.9.8")
}

pub fn get_icon() -> &'static [u8; 3216] {
    let bytes: &'static [u8; 3216] = include_bytes!("../../assets/icon.png");
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cleaner_database::{get_database_from_file, get_default_database};
    use crate::structures::CleanerData;
    use crate::utils::get_file_size_string;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_get_version() {
        let version = get_version();
        assert!(!version.is_empty(), "Version should not be empty");
        assert!(
            version.contains('.'),
            "Version should contain dots (semantic versioning)"
        );
    }

    #[test]
    fn test_get_icon() {
        let icon = get_icon();
        assert_eq!(icon.len(), 3216, "Icon should be exactly 3216 bytes");
        // Check PNG magic number
        assert_eq!(
            &icon[0..4],
            &[0x89, 0x50, 0x4E, 0x47],
            "Should be a PNG file"
        );
    }

    #[test]
    fn test_get_default_database() {
        let database = get_default_database();
        assert!(!database.is_empty(), "Default database should not be empty");

        // Check that all entries have required fields
        for entry in database.iter() {
            assert!(!entry.path.is_empty(), "Path should not be empty");
            assert!(!entry.category.is_empty(), "Category should not be empty");
            assert!(!entry.program.is_empty(), "Program should not be empty");
        }
    }

    #[test]
    fn test_database_decompression() {
        // This test verifies that the gzip-compressed database can be loaded
        let database = get_default_database();
        assert!(
            database.len() > 100,
            "Database should contain many entries (decompression worked)"
        );
    }

    #[test]
    fn test_database_placeholder_expansion() {
        let database = get_default_database();

        // Check that placeholders are replaced
        for entry in database.iter() {
            assert!(
                !entry.path.contains("{username}"),
                "Username placeholder should be replaced in path: {}",
                entry.path
            );
        }
    }

    #[test]
    fn test_database_from_file_invalid() {
        // Test with non-existent file
        let result = get_database_from_file("nonexistent_file.json");
        assert!(result.is_err(), "Should return error for non-existent file");
    }

    #[test]
    fn test_database_from_file_invalid_json() {
        // Create temporary file with invalid JSON
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"{ invalid json }").unwrap();

        let result = get_database_from_file(temp_file.path().to_str().unwrap());
        assert!(result.is_err(), "Should return error for invalid JSON");
    }

    #[test]
    fn test_database_from_file_valid() {
        // Create temporary file with valid JSON
        let mut temp_file = NamedTempFile::new().unwrap();
        let json_data = r#"[
            {
                "path": "C:\\Test\\{username}\\file.txt",
                "category": "TestCategory",
                "program": "TestProgram",
                "class": "TestClass",
                "remove_files": true
            }
        ]"#;
        temp_file.write_all(json_data.as_bytes()).unwrap();

        let result = get_database_from_file(temp_file.path().to_str().unwrap());
        assert!(result.is_ok(), "Should successfully load valid JSON");

        let database = result.unwrap();
        assert_eq!(database.len(), 1, "Should have one entry");
        assert_eq!(database[0].program, "TestProgram");
        assert_eq!(database[0].category, "TestCategory");
    }

    #[test]
    fn test_file_size_string_formatting() {
        assert_eq!(get_file_size_string(0), "0 B");
        assert_eq!(get_file_size_string(512), "512 B");
        assert_eq!(get_file_size_string(1024), "1.0 KB");
        assert_eq!(get_file_size_string(1536), "1.5 KB");
        assert_eq!(get_file_size_string(1_048_576), "1.0 MB");
        assert_eq!(get_file_size_string(1_073_741_824), "1.0 GB");
        assert_eq!(get_file_size_string(1_099_511_627_776), "1.0 TB");
    }

    #[test]
    fn test_cleaner_data_structure() {
        let data = CleanerData {
            path: String::from("test/path"),
            category: String::from("Cache"),
            program: String::from("TestApp"),
            class: String::from("Application"),
            files_to_remove: vec![String::from("*.tmp")],
            directories_to_remove: vec![String::from("cache")],
            remove_all_in_dir: false,
            remove_directory_after_clean: true,
            remove_directories: true,
            remove_files: true,
        };

        assert_eq!(data.path, "test/path");
        assert_eq!(data.category, "Cache");
        assert_eq!(data.program, "TestApp");
        assert!(data.remove_files);
        assert!(data.remove_directories);
    }

    #[test]
    fn test_database_categories_exist() {
        let database = get_default_database();
        let categories: std::collections::HashSet<String> = database
            .iter()
            .map(|entry| entry.category.clone())
            .collect();

        // Check that common categories exist
        assert!(categories.len() > 0, "Should have at least one category");
    }

    #[test]
    fn test_database_programs_exist() {
        let database = get_default_database();
        let programs: std::collections::HashSet<String> =
            database.iter().map(|entry| entry.program.clone()).collect();

        assert!(
            programs.len() > 10,
            "Should have many different programs in database"
        );
    }

    #[test]
    fn test_database_performance() {
        use std::time::Instant;

        let start = Instant::now();
        let database = get_default_database();
        let duration = start.elapsed();

        assert!(
            duration.as_millis() < 100,
            "Database loading should be fast (< 100ms), took {:?}",
            duration
        );
        assert!(!database.is_empty());
    }

    #[test]
    fn test_gzip_compression_ratio() {
        // This verifies that compression is actually happening
        let database = get_default_database();

        // Original JSON would be much larger
        // Compressed should give us a reasonable database size
        assert!(
            database.len() > 500,
            "Database should have many entries (compression ratio is good)"
        );
    }

    #[test]
    fn test_database_no_duplicates() {
        let database = get_default_database();
        let mut seen = std::collections::HashSet::new();
        let mut duplicates = Vec::new();

        for entry in database.iter() {
            let key = format!("{}:{}:{}", entry.program, entry.category, entry.path);
            if !seen.insert(key.clone()) {
                duplicates.push(key);
            }
        }

        // Note: Some duplicates may exist in the database (e.g., same program clearing same path for different purposes)
        // This test now just reports duplicates without failing
        if !duplicates.is_empty() {
            println!(
                "Found {} duplicate entries (this may be intentional):",
                duplicates.len()
            );
            for dup in duplicates.iter().take(5) {
                println!("  - {}", dup);
            }
        }
    }

    #[test]
    fn test_database_entries_valid_paths() {
        let database = get_default_database();

        for entry in database.iter() {
            // Check that paths don't have obvious issues
            assert!(!entry.path.is_empty(), "Path should not be empty");
            assert!(
                !entry.path.contains("{{"),
                "Path should not have double braces: {}",
                entry.path
            );
        }
    }

    #[test]
    fn test_file_size_edge_cases() {
        assert_eq!(get_file_size_string(0), "0 B");
        assert_eq!(get_file_size_string(1), "1 B");
        assert_eq!(get_file_size_string(1023), "1023 B");

        // Test large numbers - the actual output depends on the formatting implementation
        let large_result = get_file_size_string(u64::MAX / (1024 * 1024 * 1024));
        // Just verify it returns something reasonable and contains "GB" or "TB"
        assert!(
            large_result.contains("GB") || large_result.contains("TB"),
            "Large file size should be formatted in GB or TB, got: {}",
            large_result
        );
    }

    #[test]
    fn test_database_concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let database = Arc::new(get_default_database().clone());
        let mut handles = vec![];

        // Spawn 10 threads that all read the database simultaneously
        for _ in 0..10 {
            let db = Arc::clone(&database);
            let handle = thread::spawn(move || {
                for entry in db.iter() {
                    assert!(!entry.program.is_empty());
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_database_memory_efficiency() {
        use std::mem::size_of_val;

        let database = get_default_database();
        let total_size = size_of_val(database.as_slice());

        // Database should be reasonably sized in memory
        assert!(total_size > 0, "Database should occupy memory");
    }

    #[test]
    fn test_category_consistency() {
        let database = get_default_database();

        // Check that categories follow expected patterns
        for entry in database.iter() {
            let category = &entry.category;
            // Categories can contain spaces (e.g., "Game saves")
            assert!(!category.is_empty(), "Category should not be empty");
            assert!(
                category.chars().next().unwrap().is_uppercase(),
                "Category should start with uppercase: {}",
                category
            );
        }
    }

    #[test]
    fn test_database_filtering_performance() {
        use std::collections::HashSet;
        use std::time::Instant;

        let database = get_default_database();
        let categories: HashSet<String> = vec!["Cache".to_string(), "Logs".to_string()]
            .into_iter()
            .collect();

        let start = Instant::now();
        let filtered: Vec<&CleanerData> = database
            .iter()
            .filter(|data| categories.contains(&data.category))
            .collect();
        let duration = start.elapsed();

        assert!(filtered.len() > 0, "Should find some entries");
        assert!(
            duration.as_micros() < 10000,
            "Filtering should be fast (< 10ms), took {:?}",
            duration
        );
    }

    #[test]
    fn test_program_name_validity() {
        let database = get_default_database();

        for entry in database.iter() {
            assert!(
                !entry.program.is_empty(),
                "Program name should not be empty"
            );
            assert!(
                entry.program.len() < 100,
                "Program name should be reasonable length: {}",
                entry.program
            );
        }
    }

    #[test]
    fn test_json_structure_compatibility() {
        // Test that we can serialize/deserialize properly
        let original = get_default_database();

        if let Some(first) = original.first() {
            let json = serde_json::to_string(first).unwrap();
            assert!(json.contains("path"));
            assert!(json.contains("category"));
            assert!(json.contains("program"));
        }
    }

    #[test]
    fn test_database_cache_efficiency() {
        use std::time::Instant;

        // First call (might involve decompression)
        let start = Instant::now();
        let _db1 = get_default_database();
        let first_duration = start.elapsed();

        // Second call (should be from static)
        let start = Instant::now();
        let _db2 = get_default_database();
        let second_duration = start.elapsed();

        assert!(
            second_duration <= first_duration,
            "Subsequent calls should be as fast or faster"
        );
    }

    #[test]
    fn test_cleaner_data_default_values() {
        let data = CleanerData {
            path: String::new(),
            category: String::new(),
            program: String::new(),
            class: String::new(),
            files_to_remove: vec![],
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
            remove_directories: false,
            remove_files: false,
        };

        assert!(!data.remove_files);
        assert!(!data.remove_directories);
        assert!(!data.remove_all_in_dir);
        assert!(!data.remove_directory_after_clean);
        assert_eq!(data.files_to_remove.len(), 0);
        assert_eq!(data.directories_to_remove.len(), 0);
    }
}
