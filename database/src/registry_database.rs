use std::sync::OnceLock;

#[cfg(windows)]
use crate::registry_utils::{remove_all_in_registry, remove_all_in_tree_in_registry};
use crate::structures::CleanerDataRegistry;
#[cfg(windows)]
use crate::structures::CleanerResult;
use flate2::read::GzDecoder;
use std::io::Read;
#[cfg(windows)]
use winreg::RegKey;
#[cfg(windows)]
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};

static DATABASE: OnceLock<Vec<CleanerDataRegistry>> = OnceLock::new();

pub fn get_default_database() -> &'static Vec<CleanerDataRegistry> {
    DATABASE.get_or_init(|| {
        let compressed_data =
            include_bytes!(concat!(env!("OUT_DIR"), "/registry_database.min.json.gz"));

        // NOTE: Decompress the data
        let mut decoder = GzDecoder::new(&compressed_data[..]);
        let mut json_data = String::new();
        decoder
            .read_to_string(&mut json_data)
            .expect("Failed to decompress database");
        let database: Vec<CleanerDataRegistry> =
            serde_json::from_str::<Vec<CleanerDataRegistry>>(&json_data)
                .expect("Failed to parse database");

        database
    })
}

#[cfg(windows)]
pub fn clear_last_activity(data: &CleanerDataRegistry) -> CleanerResult {
    let mut removed: u64 = 0;

    let mut result = CleanerResult {
        files: 0,
        folders: 0,
        bytes: 0,
        working: false,
        path: data.path.clone(),
        program: data.program.clone(),
        category: data.category.clone(),
    };

    let root = if data.path.starts_with("HKEY_CURRENT_USER") {
        Some(RegKey::predef(HKEY_CURRENT_USER))
    } else if data.path.starts_with("HKEY_LOCAL_MACHINE") {
        Some(RegKey::predef(HKEY_LOCAL_MACHINE))
    } else {
        None
    };

    let path = if data.path.starts_with("HKEY_CURRENT_USER") {
        data.path.replace("HKEY_CURRENT_USER\\", "")
    } else if data.path.starts_with("HKEY_LOCAL_MACHINE") {
        data.path.replace("HKEY_LOCAL_MACHINE\\", "")
    } else {
        String::new()
    };

    if root.is_some() {
        let root = root.unwrap();
        if data.remove_all_in_tree {
            removed += remove_all_in_tree_in_registry(&root, path.to_string())
        }
        if data.remove_all_in_registry {
            removed += remove_all_in_registry(&root, path.to_string())
        }
        for value in data.values_to_remove.iter() {
            use crate::registry_utils::remove_value_in_registry;

            removed += remove_value_in_registry(&root, path.to_string(), value.to_string());
        }
        for value in data.keys_to_remove.iter() {
            use crate::registry_utils::remove_key_in_registry;

            removed += remove_key_in_registry(&root, path.to_string() + "\\" + value);
        }
    }

    if removed > 0 {
        result.working = true;
        result.bytes = removed;
    }

    result
}
