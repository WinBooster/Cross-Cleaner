#[cfg(windows)]
use winreg::{
    RegKey,
    enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, KEY_WRITE},
};

/// `DELETE` from `winnt.h`, spelled out because `winreg` does not re-export it.
///
/// Requesting it in `RegOpenKeyExW` is the whole probe: the call either grants
/// the right (and the caller closes the handle again without deleting
/// anything) or refuses, which is the answer the scan needs.
#[cfg(windows)]
pub const DELETE_ACCESS: u32 = 0x0001_0000;

/// Whether a real run could delete from this key right now.
#[cfg(windows)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAccess {
    /// The key could be deleted.
    Yes,
    /// Another process has the key open, so deleting it would fail.
    Locked,
    /// The key's ACL refuses this user.
    Denied,
    /// The key is not there, or the probe could not tell.
    Unknown,
}

#[cfg(windows)]
impl KeyAccess {
    /// True when a run would go through.
    pub fn is_yes(self) -> bool {
        self == KeyAccess::Yes
    }
}

/// Asks whether `path` under `key` could be written to, without touching it.
///
/// The same two failures the filesystem probe distinguishes, reached the same
/// way: the access is requested, and a refusal is read as `ERROR_SHARING_VIOLATION`
/// or `ERROR_ACCESS_DENIED`. Those arrive as `io::Error`s out of `winreg`, which
/// does not surface the raw code, so the numeric `raw_os_error` is matched.
#[cfg(windows)]
pub fn probe_key_access(key: &RegKey, path: &str, flags: u32) -> KeyAccess {
    match key.open_subkey_with_flags(path, flags) {
        // Dropped immediately: the probe opens, it does not keep.
        Ok(handle) => {
            drop(handle);
            KeyAccess::Yes
        }
        Err(error) => match error.raw_os_error() {
            // ERROR_SHARING_VIOLATION
            Some(32) => KeyAccess::Locked,
            // ERROR_ACCESS_DENIED
            Some(5) => KeyAccess::Denied,
            // ERROR_FILE_NOT_FOUND / ERROR_PATH_NOT_FOUND: nothing to delete.
            Some(2) | Some(3) => KeyAccess::Unknown,
            _ => KeyAccess::Unknown,
        },
    }
}

/// The access a registry entry needs to do what the database says it does.
///
/// Deleting the key itself implies everything inside it, so an entry that
/// removes keys asks for the stronger right. An entry that only removes values
/// needs just permission to set values on the key.
#[cfg(windows)]
pub fn access_for(data: &crate::structures::CleanerDataRegistry) -> Option<u32> {
    let removes_keys = data.remove_all_in_tree
        || data.remove_all_in_registry
        || data.remove_trees == "true"
        || !data.remove_trees.is_empty()
        || !data.keys_to_remove.is_empty();
    let removes_values = data.remove_values == "true"
        || !data.remove_values.is_empty()
        || !data.values_to_remove.is_empty();

    match (removes_keys, removes_values) {
        (true, _) => Some(DELETE_ACCESS),
        (false, true) => Some(KEY_SET_VALUE),
        // The entry names nothing to remove, so there is nothing to probe.
        (false, false) => None,
    }
}

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

/// One registry item removed, named.
///
/// The database stores a pattern (`…\*\Histo*`), but what a cleaner deletes is a
/// list of concrete keys and values. A report naming only the pattern cannot be
/// checked against the registry afterwards, and it cannot say which of the keys
/// the pattern matched freed what — which is the question the results page is
/// there to answer.
#[cfg(windows)]
pub struct RegistryRemoval {
    /// Hive-relative path of the item that went. A value is addressed as its key
    /// plus its name, which is how regedit addresses one too. The caller
    /// prepends the hive: these helpers work from a predef `RegKey` and so never
    /// know its name.
    pub path: String,
    pub bytes: u64,
}

#[cfg(windows)]
pub fn remove_all_in_tree_in_registry(key: &RegKey, path: &str) -> Vec<RegistryRemoval> {
    let mut names = Vec::new();
    let mut removed = Vec::new();

    if let Ok(typed_path_read) = key.open_subkey_with_flags(path, KEY_READ) {
        for name in typed_path_read.enum_keys().flatten() {
            // The same accounting the running total used: the subkey's declared
            // value sizes, which is all the registry reports before a removal.
            let bytes = typed_path_read
                .open_subkey(&name)
                .and_then(|subkey| subkey.query_info())
                .map_or(0, |info| {
                    info.max_value_name_len as u64 + info.max_value_len as u64
                });
            removed.push(RegistryRemoval {
                path: format!("{}\\{}", path, name),
                bytes,
            });
            names.push(name);
        }
    }

    if let Ok(typed_path_write) = key.open_subkey_with_flags(path, KEY_WRITE) {
        for name in names {
            let _ = typed_path_write.delete_subkey_all(&name);
        }
    }

    removed
}

#[cfg(windows)]
pub fn remove_all_in_registry(key: &RegKey, path: &str) -> Vec<RegistryRemoval> {
    let mut names = Vec::new();
    let mut removed = Vec::new();

    if let Ok(typed_path_read) = key.open_subkey_with_flags(path, KEY_READ) {
        for (name, value) in typed_path_read.enum_values().flatten() {
            removed.push(RegistryRemoval {
                path: format!("{}\\{}", path, name),
                bytes: (name.len() + value.to_string().len()) as u64,
            });
            names.push(name);
        }
    }

    if let Ok(typed_path_write) = key.open_subkey_with_flags(path, KEY_WRITE) {
        for name in names {
            let _ = typed_path_write.delete_value(&name);
        }
    }

    removed
}

/// Removes one named value. `None` when it was not there.
///
/// Not reporting it is the point: `values_to_remove` names values that may never
/// have been written, and listing one would claim a removal that did not happen.
#[cfg(windows)]
pub fn remove_value_in_registry(
    key: &RegKey,
    path: &str,
    value_name: &str,
) -> Option<RegistryRemoval> {
    let existed = key
        .open_subkey_with_flags(path, KEY_READ)
        .ok()
        .and_then(|read| read.get_raw_value(value_name).ok())
        .map(|value| (value_name.len() + value.bytes.len()) as u64);

    if let Ok(typed_path_write) = key.open_subkey_with_flags(path, KEY_WRITE) {
        let _ = typed_path_write.delete_value(value_name);
    }

    existed.map(|bytes| RegistryRemoval {
        path: format!("{}\\{}", path, value_name),
        bytes,
    })
}

/// Removes a key and everything under it. `None` when the key was not there.
///
/// Reported as the one key carrying its whole size rather than as its values and
/// subkeys one by one: the database named the key, and a list whose lines add up
/// to twice the size of the thing they belong to reads as a bug, not as detail.
#[cfg(windows)]
pub fn remove_key_in_registry(key: &RegKey, path: &str) -> Option<RegistryRemoval> {
    key.open_subkey_with_flags(path, KEY_READ).ok()?;

    let values = remove_all_in_registry(key, path);
    let tree = remove_all_in_tree_in_registry(key, path);
    let bytes: u64 = values.iter().chain(&tree).map(|item| item.bytes).sum();

    if let Ok(parent) = key.open_subkey_with_flags("", KEY_WRITE) {
        let _ = parent.delete_subkey_all(path);
    }

    Some(RegistryRemoval {
        path: path.to_string(),
        bytes,
    })
}

// INFO: Remove values inside `path` whose names match glob pattern
// ("*" and "?" supported, case-insensitive)
#[cfg(windows)]
pub fn remove_values_matching_in_registry(
    key: &RegKey,
    path: &str,
    pattern: &str,
) -> Vec<RegistryRemoval> {
    let mut matched = Vec::new();
    let mut removed = Vec::new();

    if let Ok(typed_path_read) = key.open_subkey_with_flags(path, KEY_READ) {
        for (name, value) in typed_path_read.enum_values().flatten() {
            if registry_name_match(pattern, &name) {
                removed.push(RegistryRemoval {
                    path: format!("{}\\{}", path, name),
                    bytes: (name.len() + value.to_string().len()) as u64,
                });
                matched.push(name);
            }
        }
    }

    if let Ok(typed_path_write) = key.open_subkey_with_flags(path, KEY_WRITE) {
        for name in matched {
            let _ = typed_path_write.delete_value(&name);
        }
    }

    removed
}

// INFO: Remove the whole subtree of every direct subkey of `path` whose name
// matches glob pattern ("*" and "?" supported, case-insensitive)
#[cfg(windows)]
pub fn remove_trees_matching_in_registry(
    key: &RegKey,
    path: &str,
    pattern: &str,
) -> Vec<RegistryRemoval> {
    let mut matched = Vec::new();

    if let Ok(typed_path_read) = key.open_subkey_with_flags(path, KEY_READ) {
        for name in typed_path_read.enum_keys().flatten() {
            if registry_name_match(pattern, &name) {
                matched.push(name);
            }
        }
    }

    matched
        .into_iter()
        // The matched subkey was just seen to exist, so `None` here would mean
        // the tree changed underneath us; dropping it beats reporting a removal
        // that cannot have happened.
        .filter_map(|name| remove_key_in_registry(key, &format!("{}\\{}", path, name)))
        .collect()
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
fn expand_pattern_recursive(
    key: &RegKey,
    base: String,
    segments: &[&str],
    results: &mut Vec<String>,
) {
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
        let mut found =
            expand_registry_path_pattern(&hkcu, "Software\\CrossCleanerGlobTest\\Hist*");
        found.sort();
        assert_eq!(
            found,
            vec![
                String::from("Software\\CrossCleanerGlobTest\\History"),
                String::from("Software\\CrossCleanerGlobTest\\History64")
            ]
        );

        // INFO: question mark pattern matches exactly one char
        let found =
            expand_registry_path_pattern(&hkcu, "Software\\CrossCleanerGlobTest\\History6?");
        assert_eq!(
            found,
            vec![String::from("Software\\CrossCleanerGlobTest\\History64")]
        );

        // INFO: literal path returns itself when key exists
        let found = expand_registry_path_pattern(&hkcu, "Software\\CrossCleanerGlobTest\\History");
        assert_eq!(
            found,
            vec![String::from("Software\\CrossCleanerGlobTest\\History")]
        );

        // INFO: literal path that does not exist -> empty
        let found =
            expand_registry_path_pattern(&hkcu, "Software\\CrossCleanerGlobTest\\Missing\\Deep");
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
            path: format!("HKEY_CURRENT_USER\\{}\\*\\Histo*", test_base).into(),
            category: std::sync::Arc::from("Test"),
            program: std::sync::Arc::from("Test"),
            class: std::sync::Arc::from("Test"),
            sub_category: std::sync::Arc::from(""),
            remove_all_in_tree: false,
            remove_all_in_registry: false,
            values_to_remove: vec![],
            keys_to_remove: vec![],
            remove_values: String::from("*cent"),
            remove_trees: String::new(),
        };
        let result = clear_registry(&data);
        assert!(result.working, "glob path clear should remove values");

        let base = hkcu.open_subkey_with_flags(test_base, KEY_READ).unwrap();
        let app1 = base.open_subkey("App1\\History").unwrap();
        let app2 = base.open_subkey("App2\\History").unwrap();
        assert!(
            app1.get_value::<String, _>("recent").is_err(),
            "App1\\History recent value should be removed"
        );
        assert!(
            app2.get_value::<String, _>("recent").is_err(),
            "App2\\History recent value should be removed"
        );
        assert!(
            base.open_subkey("App1\\KeepMe").is_ok(),
            "KeepMe should survive"
        );

        hkcu.delete_subkey_all(test_base).unwrap();
    }

    /// A glob matches several concrete keys, and each removed value is its own
    /// path. Reporting only the pattern — or only the first match — is what made
    /// the results page unable to say where the bytes came from.
    #[cfg(windows)]
    #[test]
    fn test_clear_registry_lists_every_key_and_value_it_removed() {
        use crate::registry_database::clear_registry;
        use crate::structures::CleanerDataRegistry;
        use winreg::enums::HKEY_CURRENT_USER;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let test_base = "Software\\CrossCleanerPathsTest";
        let _ = hkcu.delete_subkey_all(test_base);
        let (base_key, _) = hkcu.create_subkey(test_base).unwrap();
        let (h1, _) = base_key.create_subkey("App1\\History").unwrap();
        h1.set_value("recent", &"x").unwrap();
        let (h2, _) = base_key.create_subkey("App2\\History").unwrap();
        h2.set_value("recent", &"yyyy").unwrap();

        let data = CleanerDataRegistry {
            path: format!("HKEY_CURRENT_USER\\{}\\*\\History", test_base).into(),
            category: std::sync::Arc::from("Test"),
            program: std::sync::Arc::from("Test"),
            class: std::sync::Arc::from("Test"),
            sub_category: std::sync::Arc::from(""),
            remove_all_in_tree: false,
            remove_all_in_registry: false,
            values_to_remove: vec![],
            keys_to_remove: vec![],
            remove_values: String::from("true"),
            remove_trees: String::new(),
        };
        let result = clear_registry(&data);
        assert!(result.working);

        let mut listed: Vec<String> = result
            .paths
            .iter()
            .map(|entry| entry.path.as_string())
            .collect();
        listed.sort();
        let base = format!("HKEY_CURRENT_USER\\{}", test_base);
        assert_eq!(
            listed,
            vec![
                format!("{}\\{}\\recent", base, "App1\\History"),
                format!("{}\\{}\\recent", base, "App2\\History"),
            ],
            "one entry per removed value, each with the hive in front",
        );
        // The bytes still add up to the total the run reports.
        let listed_bytes: u64 = result.paths.iter().map(|entry| entry.removed_bytes).sum();
        assert_eq!(listed_bytes, result.bytes);
        assert!(result.bytes > 0);

        hkcu.delete_subkey_all(test_base).unwrap();
    }

    /// A key named by `keys_to_remove` is listed under its own path, not under
    /// the pattern that reached it.
    #[cfg(windows)]
    #[test]
    fn test_clear_registry_lists_keys_to_remove_by_name() {
        use crate::registry_database::clear_registry;
        use crate::structures::CleanerDataRegistry;
        use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let test_base = "Software\\CrossCleanerKeysToRemoveTest";
        let _ = hkcu.delete_subkey_all(test_base);
        let (base_key, _) = hkcu.create_subkey(test_base).unwrap();
        let (inner, _) = base_key.create_subkey("Inner").unwrap();
        inner.set_value("recent", &"value").unwrap();
        base_key.create_subkey("Survivor").unwrap();

        let data = CleanerDataRegistry {
            path: format!("HKEY_CURRENT_USER\\{}", test_base).into(),
            category: std::sync::Arc::from("Test"),
            program: std::sync::Arc::from("Test"),
            class: std::sync::Arc::from("Test"),
            sub_category: std::sync::Arc::from(""),
            remove_all_in_tree: false,
            remove_all_in_registry: false,
            values_to_remove: vec![],
            keys_to_remove: vec![std::sync::Arc::from("Inner")],
            remove_values: String::new(),
            remove_trees: String::new(),
        };
        let result = clear_registry(&data);
        assert!(result.working);
        assert_eq!(result.paths.len(), 1, "{:?}", result.paths);
        assert_eq!(
            result.paths[0].path.as_string(),
            format!("HKEY_CURRENT_USER\\{}\\{}", test_base, "Inner"),
        );

        let base = hkcu.open_subkey_with_flags(test_base, KEY_READ).unwrap();
        assert!(base.open_subkey("Inner").is_err(), "Inner should be gone");
        assert!(base.open_subkey("Survivor").is_ok(), "Survivor stays");

        hkcu.delete_subkey_all(test_base).unwrap();
    }

    /// `keys_to_remove` and `values_to_remove` name items that may never have
    /// existed. A key that was not there must not be reported as removed, or the
    /// page would list a removal that never happened.
    #[cfg(windows)]
    #[test]
    fn test_clear_registry_reports_nothing_for_absent_items() {
        use crate::registry_database::clear_registry;
        use crate::structures::CleanerDataRegistry;
        use winreg::enums::HKEY_CURRENT_USER;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let test_base = "Software\\CrossCleanerAbsentTest";
        let _ = hkcu.delete_subkey_all(test_base);
        let (base_key, _) = hkcu.create_subkey(test_base).unwrap();
        base_key.set_value("keep", &"keep").unwrap();

        let data = CleanerDataRegistry {
            path: format!("HKEY_CURRENT_USER\\{}", test_base).into(),
            category: std::sync::Arc::from("Test"),
            program: std::sync::Arc::from("Test"),
            class: std::sync::Arc::from("Test"),
            sub_category: std::sync::Arc::from(""),
            remove_all_in_tree: false,
            remove_all_in_registry: false,
            values_to_remove: vec![std::sync::Arc::from("never-written")],
            keys_to_remove: vec![std::sync::Arc::from("NeverExisted")],
            remove_values: String::new(),
            remove_trees: String::new(),
        };
        let result = clear_registry(&data);
        assert!(!result.working, "nothing was removed");
        assert_eq!(result.bytes, 0);
        assert!(result.paths.is_empty(), "{:?}", result.paths);

        hkcu.delete_subkey_all(test_base).unwrap();
    }
}
