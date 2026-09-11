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

#[cfg(windows)]
pub fn remove_value_in_registry(key: &RegKey, path: String, value_name: String) -> u64 {
    let mut total_bytes = 0;

    if let Ok(typed_path_read) = key.open_subkey_with_flags(&path, KEY_READ) {
        if let Ok(reg_value) = typed_path_read.get_raw_value(&value_name) {
            total_bytes = (value_name.len() + reg_value.bytes.len()) as u64;
        }
    }

    if let Ok(typed_path_write) = key.open_subkey_with_flags(path, KEY_WRITE) {
        let _ = typed_path_write.delete_value(&value_name);
    }

    total_bytes
}

#[cfg(windows)]
pub fn remove_key_in_registry(key: &RegKey, path: String) -> u64 {
    let values_size = remove_all_in_registry(key, path.clone());
    let tree_size = remove_all_in_tree_in_registry(key, path.clone());

    if let Ok(parent) = key.open_subkey_with_flags("", KEY_WRITE) {
        let _ = parent.delete_subkey_all(&path);
    }

    values_size + tree_size
}

// INFO: Remove values inside `path` whose names match glob pattern
// ("*" and "?" supported, case-insensitive)
#[cfg(windows)]
pub fn remove_values_matching_in_registry(key: &RegKey, path: String, pattern: String) -> u64 {
    let mut matched = Vec::new();
    let mut total_bytes = 0;

    if let Ok(typed_path_read) = key.open_subkey_with_flags(&path, KEY_READ) {
        for val in typed_path_read.enum_values().flatten() {
            if registry_name_match(&pattern, &val.0) {
                total_bytes += (val.0.len() + val.1.to_string().len()) as u64;
                matched.push(val.0);
            }
        }
    }

    if let Ok(typed_path_write) = key.open_subkey_with_flags(path, KEY_WRITE) {
        for value_name in matched {
            let _ = typed_path_write.delete_value(&value_name);
        }
    }

    total_bytes
}

// INFO: Remove the whole subtree of every direct subkey of `path` whose name
// matches glob pattern ("*" and "?" supported, case-insensitive)
#[cfg(windows)]
pub fn remove_trees_matching_in_registry(key: &RegKey, path: String, pattern: String) -> u64 {
    let mut matched = Vec::new();

    if let Ok(typed_path_read) = key.open_subkey_with_flags(&path, KEY_READ) {
        for name in typed_path_read.enum_keys().flatten() {
            if registry_name_match(&pattern, &name) {
                matched.push(name);
            }
        }
    }

    let mut total_bytes = 0;
    for name in matched {
        total_bytes += remove_key_in_registry(key, format!("{}\\{}", path, name));
    }

    total_bytes
}

// INFO: Case-insensitive wildcard match for one registry key name segment.
// Supports "*" (any number of characters) and "?" (exactly one character)
#[cfg(windows)]
pub fn registry_name_match(pattern: &str, name: &str) -> bool {
    let pattern_chars: Vec<char> = pattern.chars().flat_map(|c| c.to_lowercase()).collect();
    let name_chars: Vec<char> = name.chars().flat_map(|c| c.to_lowercase()).collect();
    wildcard_match(&pattern_chars, &name_chars)
}

#[cfg(windows)]
fn wildcard_match(pattern: &[char], name: &[char]) -> bool {
    if pattern.is_empty() {
        return name.is_empty();
    }

    match pattern[0] {
        '*' => {
            for i in 0..=name.len() {
                if wildcard_match(&pattern[1..], &name[i..]) {
                    return true;
                }
            }
            false
        }
        '?' => !name.is_empty() && wildcard_match(&pattern[1..], &name[1..]),
        c => !name.is_empty() && name[0] == c && wildcard_match(&pattern[1..], &name[1..]),
    }
}

// INFO: Expand registry path pattern (relative, may contain "*" and "?" in any
// segment) into list of concrete relative paths by walking the registry tree.
// Literal segments are checked against real registry hierarchy, so patterns
// like "Software\\MyApp\\*\\History" return only existing keys
#[cfg(windows)]
pub fn expand_registry_path_pattern(key: &RegKey, pattern: &str) -> Vec<String> {
    let segments: Vec<&str> = pattern.split('\\').filter(|s| !s.is_empty()).collect();
    let mut results = Vec::new();
    expand_pattern_recursive(key, String::new(), &segments, &mut results);
    results
}

#[cfg(windows)]
fn expand_pattern_recursive(key: &RegKey, base: String, segments: &[&str], results: &mut Vec<String>) {
    if segments.is_empty() {
        // INFO: Only report paths that actually exist in registry
        if !base.is_empty() && key.open_subkey_with_flags(&base, KEY_READ).is_ok() {
            results.push(base);
        }
        return;
    }

    let segment = segments[0];

    if segment.contains('*') || segment.contains('?') {
        let subkeys: Vec<String> = if base.is_empty() {
            key.enum_keys().flatten().collect()
        } else if let Ok(dir) = key.open_subkey_with_flags(&base, KEY_READ) {
            dir.enum_keys().flatten().collect()
        } else {
            Vec::new()
        };

        for name in subkeys {
            if registry_name_match(segment, &name) {
                let next = if base.is_empty() {
                    name
                } else {
                    format!("{}\\{}", base, name)
                };
                expand_pattern_recursive(key, next, &segments[1..], results);
            }
        }
    } else {
        let next = if base.is_empty() {
            segment.to_string()
        } else {
            format!("{}\\{}", base, segment)
        };
        expand_pattern_recursive(key, next, &segments[1..], results);
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn test_registry_name_match_exact() {
        assert!(registry_name_match("History", "History"));
        assert!(!registry_name_match("History", "History64"));
    }

    #[test]
    fn test_registry_name_match_case_insensitive() {
        assert!(registry_name_match("history", "HiStOrY"));
        assert!(registry_name_match("HISTORY", "history"));
    }

    #[test]
    fn test_registry_name_match_star() {
        assert!(registry_name_match("*", "anything"));
        assert!(registry_name_match("", ""));
        assert!(!registry_name_match("", "x"));
        assert!(registry_name_match("History*", "History64"));
        assert!(registry_name_match("*History", "RecentHistory"));
        assert!(registry_name_match("H*ry", "History"));
        assert!(!registry_name_match("H*ry", "Histor"));
    }

    #[test]
    fn test_registry_name_match_question() {
        assert!(registry_name_match("Histo?y", "History"));
        assert!(!registry_name_match("Histo?y", "History64"));
        assert!(!registry_name_match("?", ""));
        assert!(registry_name_match("?", "a"));
    }

    #[test]
    fn test_registry_name_match_cyrillic() {
        assert!(registry_name_match("Прог*", "Программы"));
        assert!(!registry_name_match("Прог*", "Настройки"));
    }

    #[cfg(windows)]
    #[test]
    fn test_expand_registry_path_pattern_hkcu() {
        use winreg::enums::HKEY_CURRENT_USER;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let test_base = "Software\\CrossCleanerGlobTest";
        let _ = hkcu.delete_subkey_all(test_base);
        let (base_key, _) = hkcu.create_subkey(test_base).unwrap();
        let (v1, _) = base_key.create_subkey("History").unwrap();
        v1.set_value("recent", &"a").unwrap();
        let (v2, _) = base_key.create_subkey("History64").unwrap();
        v2.set_value("recent", &"b").unwrap();
        base_key.create_subkey("Settings").unwrap();

        // INFO: star pattern matches History + History64, not Settings
        let mut found = expand_registry_path_pattern(
            &hkcu,
            "Software\\CrossCleanerGlobTest\\Hist*",
        );
        found.sort();
        assert_eq!(
            found,
            vec![
                String::from("Software\\CrossCleanerGlobTest\\History"),
                String::from("Software\\CrossCleanerGlobTest\\History64")
            ]
        );

        // INFO: question mark pattern matches exactly one char
        let found = expand_registry_path_pattern(
            &hkcu,
            "Software\\CrossCleanerGlobTest\\History6?",
        );
        assert_eq!(
            found,
            vec![String::from("Software\\CrossCleanerGlobTest\\History64")]
        );

        // INFO: literal path returns itself when key exists
        let found = expand_registry_path_pattern(
            &hkcu,
            "Software\\CrossCleanerGlobTest\\History",
        );
        assert_eq!(
            found,
            vec![String::from("Software\\CrossCleanerGlobTest\\History")]
        );

        // INFO: literal path that does not exist -> empty
        let found = expand_registry_path_pattern(
            &hkcu,
            "Software\\CrossCleanerGlobTest\\Missing\\Deep",
        );
        assert!(found.is_empty());

        hkcu.delete_subkey_all(test_base).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn test_clear_registry_glob_path_and_keys() {
        use crate::registry_database::clear_registry;
        use crate::structures::CleanerDataRegistry;
        use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let test_base = "Software\\CrossCleanerGlobClearTest";
        let _ = hkcu.delete_subkey_all(test_base);
        let (base_key, _) = hkcu.create_subkey(test_base).unwrap();
        let (h1, _) = base_key.create_subkey("App1\\History").unwrap();
        h1.set_value("recent", &"x").unwrap();
        let (h2, _) = base_key.create_subkey("App2\\History").unwrap();
        h2.set_value("recent", &"y").unwrap();
        base_key.create_subkey("App1\\KeepMe").unwrap();

        // INFO: remove_values with glob "*name" - removes values ending on name
        let data = CleanerDataRegistry {
            path: format!("HKEY_CURRENT_USER\\{}\\*\\Histo*", test_base),
            category: String::from("Test"),
            program: String::from("Test"),
            class: String::from("Test"),
            sub_category: String::new(),
            remove_all_in_tree: false,
            remove_all_in_registry: false,
            values_to_remove: vec![],
            keys_to_remove: vec![],
            remove_values: String::from("*cent"),
            remove_trees: String::new(),
        };
        let result = clear_registry(&data);
        assert!(result.working, "glob path clear should remove values");

        let base = hkcu
            .open_subkey_with_flags(test_base, KEY_READ)
            .unwrap();
        let app1 = base.open_subkey("App1\\History").unwrap();
        let app2 = base.open_subkey("App2\\History").unwrap();
        assert!(app1.get_value::<String, _>("recent").is_err(), "App1\\History recent value should be removed");
        assert!(app2.get_value::<String, _>("recent").is_err(), "App2\\History recent value should be removed");
        assert!(base.open_subkey("App1\\KeepMe").is_ok(), "KeepMe should survive");

        hkcu.delete_subkey_all(test_base).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn test_clear_registry_remove_values_true_removes_all() {
        use crate::registry_database::clear_registry;
        use crate::structures::CleanerDataRegistry;
        use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let test_base = "Software\\CrossCleanerGlobValuesAllTest";
        let _ = hkcu.delete_subkey_all(test_base);
        let (base_key, _) = hkcu.create_subkey(test_base).unwrap();
        let (h1, _) = base_key.create_subkey("History").unwrap();
        h1.set_value("recent", &"x").unwrap();
        h1.set_value("other", &"y").unwrap();

        // INFO: remove_values = "true" removes all values
        let data = CleanerDataRegistry {
            path: format!("HKEY_CURRENT_USER\\{}\\History", test_base),
            category: String::from("Test"),
            program: String::from("Test"),
            class: String::from("Test"),
            sub_category: String::new(),
            remove_all_in_tree: false,
            remove_all_in_registry: false,
            values_to_remove: vec![],
            keys_to_remove: vec![],
            remove_values: String::from("true"),
            remove_trees: String::new(),
        };
        let result = clear_registry(&data);
        assert!(result.working);

        let h1 = hkcu
            .open_subkey_with_flags(format!("{}\\History", test_base), KEY_READ)
            .unwrap();
        assert!(h1.get_value::<String, _>("recent").is_err());
        assert!(h1.get_value::<String, _>("other").is_err());

        hkcu.delete_subkey_all(test_base).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn test_clear_registry_remove_trees_glob() {
        use crate::registry_database::clear_registry;
        use crate::structures::CleanerDataRegistry;
        use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let test_base = "Software\\CrossCleanerGlobTreeTest";
        let _ = hkcu.delete_subkey_all(test_base);
        let (base_key, _) = hkcu.create_subkey(test_base).unwrap();
        let (h1, _) = base_key.create_subkey("App1\\History\\Deep").unwrap();
        h1.set_value("recent", &"x").unwrap();
        let (h2, _) = base_key.create_subkey("App2\\History").unwrap();
        h2.set_value("recent", &"y").unwrap();
        base_key.create_subkey("App1\\KeepMe").unwrap();

        // INFO: remove_trees with glob "Hist*" - deletes matched subkey trees
        let data = CleanerDataRegistry {
            path: format!("HKEY_CURRENT_USER\\{}\\*", test_base),
            category: String::from("Test"),
            program: String::from("Test"),
            class: String::from("Test"),
            sub_category: String::new(),
            remove_all_in_tree: false,
            remove_all_in_registry: false,
            values_to_remove: vec![],
            keys_to_remove: vec![],
            remove_values: String::new(),
            remove_trees: String::from("Hist*"),
        };
        let result = clear_registry(&data);
        assert!(result.working, "remove_trees should delete matched trees");

        let base = hkcu
            .open_subkey_with_flags(test_base, KEY_READ)
            .unwrap();
        assert!(base.open_subkey("App1\\History").is_err(), "App1\\History tree should be removed");
        assert!(base.open_subkey("App2\\History").is_err(), "App2\\History tree should be removed");
        assert!(base.open_subkey("App1\\KeepMe").is_ok(), "KeepMe should survive");

        hkcu.delete_subkey_all(test_base).unwrap();
    }
}
