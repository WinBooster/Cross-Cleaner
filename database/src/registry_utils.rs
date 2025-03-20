#[cfg(windows)]
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
#[cfg(windows)]
use winreg::RegKey;

#[cfg(windows)]
pub fn get_steam_directory_from_registry() -> String {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let software_valve_steam = hkcu.open_subkey("SOFTWARE\\Valve\\Steam");
    match software_valve_steam {
        Ok(software_valve_steam) => {
            software_valve_steam.get_value("SteamPath").unwrap_or_default()
        }
        Err(_) => { String::from("") }
    }
}

#[cfg(windows)]
pub fn remove_all_in_tree_in_registry(key: &RegKey, path: String) {
    let mut keys = Vec::<String>::new();

    let typed_path_read = key.open_subkey_with_flags(path.clone(), KEY_READ);
    match typed_path_read {
        Ok(typed_path_read) => {
            // Enumerate all values in the TypedPaths subkey
            for val in typed_path_read.enum_keys() {
                match val {
                    Ok(name) => {
                        keys.push(name);
                    }
                    Err(e) => {}
                }
            }
        }
        Err(e) => { }
    }

    let typed_path_write = key.open_subkey_with_flags(path, KEY_WRITE);
    match typed_path_write {
        Ok(typed_path_write) => {
            for key in keys {
                typed_path_write.delete_subkey_all(key).unwrap_or_default();
            }
        }
        Err(_) => {}
    }
}
#[cfg(windows)]
pub fn remove_all_in_registry(key: &RegKey, value: String) {
    let mut keys = Vec::<String>::new();

    let path = value;

    let typed_path_read = key.open_subkey_with_flags(path.clone(), KEY_READ);
    match typed_path_read {
        Ok(typed_path_read) => {
            // Enumerate all values in the TypedPaths subkey
            for val in typed_path_read.enum_values() {
                match val {
                    Ok((name, reg_value)) => {
                        keys.push(name);
                    }
                    Err(e) => {}
                }
            }
        }
        Err(e) => { }
    }

    let typed_path_write = key.open_subkey_with_flags(path, KEY_WRITE);
    match typed_path_write {
        Ok(typed_path_write) => {
            for key in keys {
                typed_path_write.delete_value(key).unwrap_or_default();
            }
        }
        Err(_) => {}
    }
}