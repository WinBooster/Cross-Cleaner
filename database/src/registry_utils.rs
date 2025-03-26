#[cfg(windows)]
use winreg::RegKey;
#[cfg(windows)]
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};

#[cfg(unix)]
pub fn get_steam_directory_from_registry() -> String {
    String::new()
}

#[cfg(windows)]
pub fn get_steam_directory_from_registry() -> String {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    match hkcu.open_subkey("SOFTWARE\\Valve\\Steam") {
        Ok(steam) => steam.get_value("SteamPath").unwrap_or_default(),
        Err(_) => String::from(""),
    }
}

#[cfg(windows)]
pub fn remove_all_in_tree_in_registry(key: &RegKey, path: String) -> u64 {
    let mut keys = Vec::<String>::new();
    let mut total_bytes = 0;

    if let Ok(typed_path_read) = key.open_subkey_with_flags(path.clone(), KEY_READ) {
        for val in typed_path_read.enum_keys() {
            if let Ok(name) = val {
                if let Ok(subkey) = typed_path_read.open_subkey(&name) {
                    if let Ok(info) = subkey.query_info() {
                        total_bytes += info.max_value_name_len as u64 + info.max_value_len as u64;
                    }
                }
                keys.push(name);
            }
        }
    }

    if let Ok(typed_path_write) = key.open_subkey_with_flags(path, KEY_WRITE) {
        for key in keys {
            typed_path_write.delete_subkey_all(key).unwrap_or_default();
        }
    }

    total_bytes
}

#[cfg(windows)]
pub fn remove_all_in_registry(key: &RegKey, value: String) -> u64 {
    let mut keys = Vec::<String>::new();
    let mut total_bytes = 0;

    if let Ok(typed_path_read) = key.open_subkey_with_flags(value.clone(), KEY_READ) {
        for val in typed_path_read.enum_values() {
            if let Ok((name, reg_value)) = val {
                total_bytes += (name.len() + reg_value.to_string().bytes().len()) as u64;
                keys.push(name);
            }
        }
    }

    if let Ok(typed_path_write) = key.open_subkey_with_flags(value, KEY_WRITE) {
        for key in keys {
            typed_path_write.delete_value(key).unwrap_or_default();
        }
    }

    total_bytes
}
