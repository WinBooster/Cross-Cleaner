use crate::CleanerData;
use crate::registry_utils::get_steam_directory_from_registry;
use disk_name::get_letters;
use flate2::read::GzDecoder;
use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::sync::{Arc, OnceLock};

static DATABASE: OnceLock<Arc<[CleanerData]>> = OnceLock::new();

pub fn get_default_database() -> Arc<[CleanerData]> {
    DATABASE
        .get_or_init(|| {
            #[cfg(target_os = "linux")]
            // NOTE: DataBase for Linux and Unix (minified and compressed at compile time)
            let compressed_data =
                include_bytes!(concat!(env!("OUT_DIR"), "/linux_database.min.json.gz"));
            #[cfg(windows)]
            // NOTE: DataBase for Windows (minified and compressed at compile time)
            let compressed_data =
                include_bytes!(concat!(env!("OUT_DIR"), "/windows_database.min.json.gz"));
            #[cfg(target_os = "macos")]
            // NOTE: DataBase for MacOS (minified and compressed at compile time)
            let compressed_data =
                include_bytes!(concat!(env!("OUT_DIR"), "/macos_database.min.json.gz"));

            // NOTE: Stream-decompress and deserialize directly into Vec (no full JSON string in RAM)
            let decoder = GzDecoder::new(&compressed_data[..]);
            let database: Vec<CleanerData> =
                serde_json::from_reader(decoder).expect("Failed to parse database");

            expand_placeholders(database).into()
        })
        .clone()
}

pub fn get_database_from_file(file_path: &str) -> Result<Arc<[CleanerData]>, Box<dyn Error>> {
    // INFO: Stream-read and deserialize directly from the file
    let reader = BufReader::new(File::open(file_path)?);
    let database: Vec<CleanerData> = serde_json::from_reader(reader)?;

    Ok(expand_placeholders(database).into())
}

/// Expand {username}, {steam} and {drive} placeholders in-place, consuming the
/// parsed database so entries are moved (not cloned) into the result.
fn expand_placeholders(database: Vec<CleanerData>) -> Vec<CleanerData> {
    // INFO: Get the username
    let username = whoami::username().expect("Failed to get username");

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

    let mut expanded_database = Vec::with_capacity(database.len());

    for mut entry in database {
        // INFO: Replacing username placeholder
        entry.path = entry.path.replace("{username}", &username);

        // INFO: Replacing steam placeholder
        // WARN: Windows only
        entry.path = entry.path.replace("{steam}", &steam_directory);

        // INFO: Replacing drive placeholder
        // WARN: Windows only
        if cfg!(windows) && entry.path.contains("{drive}") {
            for drive in &drives {
                let mut drive_entry = entry.clone();
                drive_entry.path = drive_entry.path.replace("{drive}", drive);
                expanded_database.push(drive_entry);
            }
        } else {
            expanded_database.push(entry);
        }
    }

    expanded_database
}
