use crate::structures::{CleanerResult, CustomCleaner};
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

type Registry = RwLock<HashMap<String, CustomCleaner>>;

fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Register a built-in custom cleaner. Call once per cleaner at startup
/// (see cleaner::custom_cleaners::register_all).
/// Returns false if a cleaner with the same id is already registered.
pub fn register_custom_cleaner(cleaner: CustomCleaner) -> bool {
    registry()
        .write()
        .expect("custom cleaner registry poisoned")
        .insert(cleaner.id.clone(), cleaner)
        .is_none()
}

/// All registered custom cleaners for the current OS, with placeholders expanded.
pub fn get_custom_cleaners() -> Vec<CustomCleaner> {
    registry()
        .read()
        .expect("custom cleaner registry poisoned")
        .values()
        .filter(|cleaner| cleaner.matches_current_os())
        .cloned()
        .map(expand_placeholders)
        .collect()
}

/// Get a single registered cleaner by id, with placeholders expanded.
pub fn get_custom_cleaner(id: &str) -> Option<CustomCleaner> {
    registry()
        .read()
        .expect("custom cleaner registry poisoned")
        .get(id)
        .cloned()
        .map(expand_placeholders)
}

/// Ids of all registered custom cleaners (all OS, without filtering).
pub fn custom_cleaner_ids() -> Vec<String> {
    registry()
        .read()
        .expect("custom cleaner registry poisoned")
        .keys()
        .cloned()
        .collect()
}

/// Execute the cleaning function of a custom cleaner.
pub fn run_custom_cleaner(cleaner: &CustomCleaner) -> CleanerResult {
    (cleaner.function)(cleaner)
}

fn expand_placeholders(mut cleaner: CustomCleaner) -> CustomCleaner {
    cleaner.path = cleaner.path.replace("{username}", &whoami::username());
    cleaner
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_cleaner(id: &str) -> CustomCleaner {
        CustomCleaner {
            id: String::from(id),
            program: String::from("TestProgram"),
            category: String::from("Logs"),
            sub_category: String::from("Logs"),
            path: String::from("{username}/test.log"),
            args: vec![],
            os: vec![],
            function: |_| CleanerResult {
                files: 0,
                folders: 0,
                bytes: 0,
                working: false,
                path: String::new(),
                program: String::new(),
                category: String::new(),
                sub_category: String::new(),
            },
        }
    }

    #[test]
    fn test_register_and_get() {
        assert!(register_custom_cleaner(test_cleaner("reg_test_1")));
        assert!(!register_custom_cleaner(test_cleaner("reg_test_1")));

        let cleaner = get_custom_cleaner("reg_test_1").expect("should be registered");
        assert_eq!(cleaner.id, "reg_test_1");
        assert!(cleaner.matches_current_os());

        let expanded = get_custom_cleaner("reg_test_1").unwrap();
        assert!(!expanded.path.contains("{username}"));

        assert!(get_custom_cleaner("nonexistent_id").is_none());
    }

    #[test]
    fn test_ids_and_os_filter() {
        register_custom_cleaner(test_cleaner("ids_test_1"));

        let ids = custom_cleaner_ids();
        assert!(ids.iter().any(|id| id == "ids_test_1"));

        let all = get_custom_cleaners();
        assert!(all.iter().any(|c| c.id == "ids_test_1"));
    }

    #[test]
    fn test_run_custom_cleaner() {
        let mut cleaner = test_cleaner("run_test_1");
        cleaner.function = |data| {
            let mut r = CleanerResult {
                files: 0,
                folders: 0,
                bytes: 0,
                working: false,
                path: data.path.clone(),
                program: data.program.clone(),
                category: data.category.clone(),
                sub_category: data.sub_category.clone(),
            };
            r.working = true;
            r.files = 1;
            r
        };

        let result = run_custom_cleaner(&cleaner);
        assert!(result.working);
        assert_eq!(result.files, 1);
        assert_eq!(result.program, "TestProgram");
    }

    #[test]
    fn test_matches_current_os() {
        let mut cleaner = test_cleaner("os_test_1");

        cleaner.os = vec![];
        assert!(cleaner.matches_current_os());

        let current = if cfg!(windows) {
            "windows"
        } else if cfg!(target_os = "linux") {
            "linux"
        } else {
            "macos"
        };
        cleaner.os = vec![String::from(current)];
        assert!(cleaner.matches_current_os());

        cleaner.os = vec![String::from("someotheros")];
        assert!(!cleaner.matches_current_os());
    }
}
