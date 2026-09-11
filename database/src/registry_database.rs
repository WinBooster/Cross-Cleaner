use std::error::Error;
use std::fs;
use std::sync::OnceLock;

#[cfg(windows)]
use crate::registry_utils::{
    expand_registry_path_pattern, remove_all_in_registry, remove_all_in_tree_in_registry,
    remove_key_in_registry, remove_trees_matching_in_registry, remove_values_matching_in_registry,
};
#[cfg(windows)]
use crate::structures::CleanerDataRegistry;
#[cfg(windows)]
use crate::structures::CleanerResult;
use flate2::read::GzDecoder;
use std::io::Read;
#[cfg(windows)]
use winreg::RegKey;
#[cfg(windows)]
use winreg::enums::*;

#[cfg(windows)]
static DATABASE: OnceLock<Vec<CleanerDataRegistry>> = OnceLock::new();

#[cfg(windows)]
pub fn get_default_database() -> &'static Vec<CleanerDataRegistry> {
    DATABASE.get_or_init(|| {
        let compressed_data =
            include_bytes!(concat!(env!("OUT_DIR"), "/registry_database.min.json.gz"));

        // NOTE: Decompress the data
        let mut decoder = GzDecoder::new(&compressed_data[..]);
        // INFO: Read decompressed data
        let mut json_data = String::new();
        decoder
            .read_to_string(&mut json_data)
            .expect("Failed to decompress database");
        // INFO: Deserialization JSON to Vec<CleanerDataRegistry>
        let database: Vec<CleanerDataRegistry> =
            serde_json::from_str::<Vec<CleanerDataRegistry>>(&json_data)
                .expect("Failed to parse database");

        database
    })
}

#[cfg(windows)]
pub fn get_database_from_file(file_path: &str) -> Result<Vec<CleanerDataRegistry>, Box<dyn Error>> {
    // INFO: Read file
    let data = fs::read_to_string(file_path)?;

    // INFO: Deserialization JSON to Vec<CleanerDataRegistry>
    let database: Vec<CleanerDataRegistry> = serde_json::from_str(&data)?;

    Ok(database)
}

#[cfg(windows)]
pub fn clear_registry(data: &CleanerDataRegistry) -> CleanerResult {
    // INFO: Temporary clearing bytes result
    let mut removed: u64 = 0;

    // INFO: Creating output struct
    let mut result = CleanerResult {
        files: 0,
        folders: 0,
        bytes: 0,
        working: false,
        path: data.path.clone(),
        program: data.program.clone(),
        category: data.category.clone(),
        sub_category: data.sub_category.clone(),
    };

    // INFO: Parsing registry key
    let root = if data.path.starts_with("HKEY_CURRENT_USER") {
        Some(RegKey::predef(HKEY_CURRENT_USER))
    } else if data.path.starts_with("HKEY_LOCAL_MACHINE") {
        Some(RegKey::predef(HKEY_LOCAL_MACHINE))
    } else if data.path.starts_with("HKEY_CLASSES_ROOT") {
        Some(RegKey::predef(HKEY_CLASSES_ROOT))
    } else if data.path.starts_with("HKEY_USERS") {
        Some(RegKey::predef(HKEY_USERS))
    } else if data.path.starts_with("HKEY_CURRENT_CONFIG") {
        Some(RegKey::predef(HKEY_CURRENT_CONFIG))
    } else {
        None
    };

    // INFO: Removing registry key from path
    let path = if data.path.starts_with("HKEY_CURRENT_USER\\") {
        Some(data.path.replace("HKEY_CURRENT_USER\\", ""))
    } else if data.path.starts_with("HKEY_LOCAL_MACHINE\\") {
        Some(data.path.replace("HKEY_LOCAL_MACHINE\\", ""))
    } else if data.path.starts_with("HKEY_CLASSES_ROOT\\") {
        Some(data.path.replace("HKEY_CLASSES_ROOT\\", ""))
    } else if data.path.starts_with("HKEY_USERS\\") {
        Some(data.path.replace("HKEY_USERS\\", ""))
    } else if data.path.starts_with("HKEY_CURRENT_CONFIG\\") {
        Some(data.path.replace("HKEY_CURRENT_CONFIG\\", ""))
    } else {
        None
    };

    // INFO: Main logic
    if root.is_some() && path.is_some() {
        let root = root.unwrap();
        let path = path.unwrap();

        // INFO: Expand glob pattern in main path ("*" and "?" per segment)
        let paths: Vec<String> = if path.contains('*') || path.contains('?') {
            expand_registry_path_pattern(&root, &path)
        } else {
            vec![path.clone()]
        };

        for current_path in paths {
            if data.remove_all_in_tree {
                removed += remove_all_in_tree_in_registry(&root, current_path.clone())
            }
            if data.remove_all_in_registry {
                removed += remove_all_in_registry(&root, current_path.clone())
            }
            // INFO: remove_values is the glob for value names matched at
            // the end of resolved paths, "true" removes all values
            if data.remove_values == "true" {
                removed += remove_all_in_registry(&root, current_path.clone())
            } else if !data.remove_values.is_empty() {
                removed += remove_values_matching_in_registry(
                    &root,
                    current_path.clone(),
                    data.remove_values.clone(),
                )
            }
            // INFO: remove_trees is the glob for subkey trees matched at
            // the end of resolved paths, "true" removes resolved keys
            if data.remove_trees == "true" {
                removed += remove_key_in_registry(&root, current_path.clone())
            } else if !data.remove_trees.is_empty() {
                removed += remove_trees_matching_in_registry(
                    &root,
                    current_path.clone(),
                    data.remove_trees.clone(),
                )
            }
            for value in data.values_to_remove.iter() {
                use crate::registry_utils::remove_value_in_registry;

                removed +=
                    remove_value_in_registry(&root, current_path.clone(), value.to_string());
            }
            for value in data.keys_to_remove.iter() {
                removed += remove_key_in_registry(&root, current_path.clone() + "\\" + value);
            }
        }
    }

    if removed > 0 {
        result.working = true;
        result.bytes = removed;
    }

    result
}
