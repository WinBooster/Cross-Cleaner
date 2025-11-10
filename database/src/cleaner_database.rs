use crate::CleanerData;
use crate::registry_utils::get_steam_directory_from_registry;
use disk_name::get_letters;
use flate2::read::GzDecoder;
use serde_json;
use std::error::Error;
use std::fs;
use std::io::Read;
use std::sync::OnceLock;

static DATABASE: OnceLock<Vec<CleanerData>> = OnceLock::new();

pub fn get_default_database() -> &'static Vec<CleanerData> {
    DATABASE.get_or_init(|| {
        #[cfg(unix)]
        // NOTE: DataBase for Linux and Unix (minified and compressed at compile time)
        let compressed_data =
            include_bytes!(concat!(env!("OUT_DIR"), "/linux_database.min.json.gz"));
        #[cfg(windows)]
        // NOTE: DataBase for Windows (minified and compressed at compile time)
        let compressed_data =
            include_bytes!(concat!(env!("OUT_DIR"), "/windows_database.min.json.gz"));
        #[cfg(macos)]
        // NOTE: DataBase for MacOS (minified and compressed at compile time)
        let compressed_data =
            include_bytes!(concat!(env!("OUT_DIR"), "/macos_database.min.json.gz"));

        // NOTE: Decompress the data
        let mut decoder = GzDecoder::new(&compressed_data[..]);
        let mut json_data = String::new();
        decoder
            .read_to_string(&mut json_data)
            .expect("Failed to decompress database");

        // NOTE: Deserialization JSON to Vec<CleanerData>
        let database: Vec<CleanerData> =
            serde_json::from_str::<Vec<CleanerData>>(&json_data).expect("Failed to parse database");

        // NOTE: Get the username
        let username = whoami::username();

        // NOTE: Getting a list of disks
        // WARN: Windows only
        let drives = if cfg!(windows) { get_letters() } else { vec![] };

        // NOTE: Get the path to Steam
        // WARN: Windows only
        let steam_directory = if cfg!(windows) {
            get_steam_directory_from_registry()
        } else {
            String::new()
        };

        // NOTE: Create a new database with placeholders replacement
        let mut expanded_database = Vec::new();

        for entry in database {
            let mut new_entry = entry.clone();

            // NOTE: Replacing username placeholder
            new_entry.path = new_entry.path.replace("{username}", &username);

            // NOTE: Replacing steam placeholder
            // WARN: Windows only
            new_entry.path = new_entry.path.replace("{steam}", &steam_directory);

            // NOTE: Replacing drive placeholder
            // WARN: Windows only
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
    // INFO: Read file
    let data = fs::read_to_string(file_path)?;

    // INFO: Deserialization JSON to Vec<CleanerData>
    let database: Vec<CleanerData> = serde_json::from_str(&data)?;

    // INFO: Get the username
    let username = whoami::username();

    // INFO: Getting a list of disks
    // WARN: Windows only
    let drives = if cfg!(windows) { get_letters() } else { vec![] };

    // INFO: Get the path to Steam
    // WARN: Windows only
    let steam_directory = if cfg!(windows) {
        get_steam_directory_from_registry()
    } else {
        String::new()
    };

    let mut expanded_database = Vec::new();

    for entry in database {
        let mut new_entry = entry.clone();

        // INFO: Replace {username}
        new_entry.path = new_entry.path.replace("{username}", &username);

        // INFO: Replace {steam}
        // WARN: Windows only
        new_entry.path = new_entry.path.replace("{steam}", &steam_directory);

        // INFO: Replace {drive}
        // WARN: Windows only
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

    Ok(expanded_database)
}
