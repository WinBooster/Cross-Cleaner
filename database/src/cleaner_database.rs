use crate::CleanerData;
use crate::registry_utils::get_steam_directory_from_registry;
use disk_name::get_letters;
use serde_json;
use std::error::Error;
use std::fs;
use std::sync::OnceLock;

static DATABASE: OnceLock<Vec<CleanerData>> = OnceLock::new();

fn get_default_database() -> &'static Vec<CleanerData> {
    DATABASE.get_or_init(|| {
        #[cfg(unix)]
        let data = include_str!("../linux_database.json");
        #[cfg(windows)]
        let data = include_str!("../windows_database.json");

        // Deserialization JSON to Vec<CleanerData>
        let database: Vec<CleanerData> =
            serde_json::from_str(&data).expect(&"Failed to parse database".to_string());

        // Get the username
        let username = whoami::username();

        // Getting a list of disks (Windows only)
        let drives = if cfg!(windows) {
            get_letters()
        } else {
            vec![] // Linux does not use disks
        };

        // Get the path to Steam
        let steam_directory = if cfg!(windows) {
            get_steam_directory_from_registry()
        } else {
            String::new() // Not used on Linux
        };

        // Create a new database with placeholders replacement
        let mut expanded_database = Vec::new();

        for entry in database {
            let mut new_entry = entry.clone();

            // Replace {username}
            new_entry.path = new_entry.path.replace("{username}", &username);

            // Replace {steam}
            new_entry.path = new_entry.path.replace("{steam}", &steam_directory);

            // Replace {drive} (Windows only)
            if cfg!(windows) && new_entry.path.contains("{drive}") {
                for drive in &drives {
                    let mut drive_entry = new_entry.clone();
                    drive_entry.path = drive_entry.path.replace("{drive}", drive);
                    expanded_database.push(drive_entry);
                }
            } else {
                expanded_database.push(new_entry);
            }
        }

        expanded_database
    })
}

pub fn get_database_from_file(file_path: &str) -> Result<Vec<CleanerData>, Box<dyn Error>> {
    // Read file
    let data = fs::read_to_string(file_path)?;

    // Deserialization JSON to Vec<CleanerData>
    let database: Vec<CleanerData> = serde_json::from_str(&data)?;

    Ok(database)
}
