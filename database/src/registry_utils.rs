#[cfg(windows)]
use winreg::{
    RegKey,
    enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE},
};

#[cfg(not(windows))]
pub fn get_steam_directory_from_registry() -> String {
    String::new()
}

#[cfg(windows)]
pub fn get_steam_directory_from_registry() -> String {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    hkcu.open_subkey("SOFTWARE\\Valve\\Steam")
        .and_then(|steam| steam.get_value("SteamPath"))
        .unwrap_or_default()
}

#[cfg(windows)]
pub fn remove_all_in_tree_in_registry(key: &RegKey, path: String) -> u64 {
    let mut keys = Vec::new();
    let mut total_bytes = 0;

    if let Ok(typed_path_read) = key.open_subkey_with_flags(&path, KEY_READ) {
        for val in typed_path_read.enum_keys().flatten() {
            if let Ok(subkey) = typed_path_read.open_subkey(&val) {
                if let Ok(info) = subkey.query_info() {
                    total_bytes += info.max_value_name_len as u64 + info.max_value_len as u64;
                }
            }
            keys.push(val);
        }
    }

    if let Ok(typed_path_write) = key.open_subkey_with_flags(path, KEY_WRITE) {
        for key_name in keys {
            let _ = typed_path_write.delete_subkey_all(&key_name);
        }
    }

    total_bytes
}

#[cfg(windows)]
pub fn remove_all_in_registry(key: &RegKey, value: String) -> u64 {
    let mut keys = Vec::new();
    let mut total_bytes = 0;

    if let Ok(typed_path_read) = key.open_subkey_with_flags(&value, KEY_READ) {
        for val in typed_path_read.enum_values().flatten() {
            total_bytes += (val.0.len() + val.1.to_string().len()) as u64;
            keys.push(val.0);
        }
    }

    if let Ok(typed_path_write) = key.open_subkey_with_flags(value, KEY_WRITE) {
        for key_name in keys {
            let _ = typed_path_write.delete_value(&key_name);
        }
    }

    total_bytes
}
