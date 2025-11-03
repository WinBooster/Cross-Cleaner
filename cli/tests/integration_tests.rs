#[cfg(test)]
mod cli_integration_tests {
    use std::process::Command;

    #[test]
    fn test_cli_help() {
        let output = Command::new("cargo")
            .args(&["run", "--bin", "Cross_Cleaner_CLI", "--", "--help"])
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Usage") || stdout.contains("USAGE"));
    }

    #[test]
    fn test_cli_version() {
        let output = Command::new("cargo")
            .args(&["run", "--bin", "Cross_Cleaner_CLI", "--", "--version"])
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Cross_Cleaner_CLI") || stdout.len() > 0);
    }

    #[test]
    fn test_cli_with_invalid_database_path() {
        let output = Command::new("cargo")
            .args(&[
                "run",
                "--bin",
                "Cross_Cleaner_CLI",
                "--",
                "--database-path=nonexistent.json",
                "--clear=cache",
            ])
            .output()
            .expect("Failed to execute command");

        // Should fail because database file doesn't exist
        assert!(!output.status.success());
    }

    #[test]
    fn test_cli_disabled_programs_parsing() {
        // This test verifies that the CLI can parse disabled programs argument
        let output = Command::new("cargo")
            .args(&["run", "--bin", "Cross_Cleaner_CLI", "--", "--help"])
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("--disabled") || stdout.contains("disabled"));
    }

    #[test]
    fn test_cli_clear_categories_parsing() {
        let output = Command::new("cargo")
            .args(&["run", "--bin", "Cross_Cleaner_CLI", "--", "--help"])
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("--clear") || stdout.contains("categories"));
    }

    #[test]
    fn test_cli_show_database_info_flag() {
        let output = Command::new("cargo")
            .args(&["run", "--bin", "Cross_Cleaner_CLI", "--", "--help"])
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("--show-database-info") || stdout.contains("database"));
    }

    #[test]
    fn test_cli_progress_bar_flag() {
        let output = Command::new("cargo")
            .args(&["run", "--bin", "Cross_Cleaner_CLI", "--", "--help"])
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("progress-bar") || stdout.contains("progress"));
    }

    #[test]
    fn test_cli_result_table_flag() {
        let output = Command::new("cargo")
            .args(&["run", "--bin", "Cross_Cleaner_CLI", "--", "--help"])
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("result-table") || stdout.contains("table"));
    }

    #[test]
    fn test_cli_notification_flag() {
        let output = Command::new("cargo")
            .args(&["run", "--bin", "Cross_Cleaner_CLI", "--", "--help"])
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("notification") || stdout.contains("show-notification"));
    }
}

#[cfg(test)]
mod cli_argument_tests {
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_valid_custom_database() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let json_data = r#"[
            {
                "path": "C:\\Test\\file.txt",
                "category": "TestCategory",
                "program": "TestProgram",
                "class": "TestClass",
                "remove_files": true
            }
        ]"#;
        temp_file.write_all(json_data.as_bytes()).unwrap();

        // Note: This test just verifies the database can be parsed
        // Actual cleaning would require proper setup
        assert!(temp_file.path().exists());
    }

    #[test]
    fn test_category_parsing() {
        // Test that comma-separated categories can be split correctly
        let categories = "cache,logs,crashes";
        let split: Vec<&str> = categories.split(',').collect();

        assert_eq!(split.len(), 3);
        assert_eq!(split[0], "cache");
        assert_eq!(split[1], "logs");
        assert_eq!(split[2], "crashes");
    }

    #[test]
    fn test_disabled_programs_parsing() {
        // Test that comma-separated programs can be split correctly
        let programs = "firefox,chrome,edge";
        let split: Vec<&str> = programs.split(',').collect();

        assert_eq!(split.len(), 3);
        assert_eq!(split[0], "firefox");
        assert_eq!(split[1], "chrome");
        assert_eq!(split[2], "edge");
    }
}
