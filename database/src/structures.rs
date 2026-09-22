use crate::utils;
use serde::{Deserialize, Serialize};
use tabled::Tabled;

// INFO: Struct for GUI table
#[derive(PartialEq, Clone, Tabled)]
pub struct Cleared {
    #[tabled(rename = "Program")]
    pub program: String,
    #[tabled(display = "display_removed_bytes", rename = "Size")]
    pub removed_bytes: u64,
    #[tabled(rename = "Files")]
    pub removed_files: u64,
    #[tabled(rename = "Dirs")]
    pub removed_directories: u64,
    #[tabled(display = "display_categories", rename = "Categories")]
    pub affected_categories: Vec<String>,
}

fn display_removed_bytes(size: &u64) -> String {
    utils::get_file_size_string(*size)
}

fn display_categories(categories: &[String]) -> String {
    categories.join(", ")
}

impl PartialEq<Option<Cleared>> for &Cleared {
    fn eq(&self, other: &Option<Cleared>) -> bool {
        match other {
            Some(other) => other.program.eq(&*self.program),
            None => false,
        }
    }
}

// INFO: Struct for clearing files and folders
#[derive(Serialize, Deserialize, Clone)]
pub struct CleanerData {
    pub path: String,
    pub category: String,
    pub program: String,
    #[serde(default = "default_class")]
    pub class: String,
    #[serde(default, alias = "sub_class", alias = "subCategory")]
    pub sub_category: String,

    #[serde(default)]
    pub files_to_remove: Vec<String>,
    #[serde(default)]
    pub directories_to_remove: Vec<String>,

    #[serde(default)]
    pub remove_all_in_dir: bool,
    #[serde(default)]
    pub remove_directory_after_clean: bool,
    #[serde(default)]
    pub remove_directories: bool,
    #[serde(default)]
    pub remove_files: bool,
}

// INFO: Struct for clearing registry
// WARN: Windows only
#[cfg(windows)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CleanerDataRegistry {
    pub category: String,
    pub program: String,
    #[serde(default = "default_class")]
    pub class: String,
    #[serde(default, alias = "sub_class")]
    pub sub_category: String,

    #[serde(default)]
    pub remove_all_in_tree: bool,
    #[serde(default)]
    pub remove_all_in_registry: bool,

    #[serde(default)]
    pub path: String,

    #[serde(default)]
    pub values_to_remove: Vec<String>,

    #[serde(default)]
    pub keys_to_remove: Vec<String>,

    // INFO: Glob matched against the last segment(s) of resolved paths:
    // value names for remove_values, subkey names for remove_trees.
    // "true" = remove all values / the resolved key tree itself
    #[serde(default)]
    pub remove_values: String,

    #[serde(default)]
    pub remove_trees: String,
}

// INFO: Lightweight projection of CleanerData used for category scans. Unknown
// fields (files_to_remove, directories_to_remove, flags, class, ...) are ignored
// by serde and never allocated. `path` is kept only to detect the {drive}
// placeholder so entry counts stay identical to the full database.
#[derive(Deserialize, Clone)]
pub struct CleanerIndex {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub program: String,
    #[serde(default, alias = "sub_class", alias = "subCategory")]
    pub sub_category: String,
}

impl From<&CleanerData> for CleanerIndex {
    fn from(data: &CleanerData) -> Self {
        Self {
            path: data.path.clone(),
            category: data.category.clone(),
            program: data.program.clone(),
            sub_category: data.sub_category.clone(),
        }
    }
}

// INFO: Lightweight projection of CleanerDataRegistry for category scans.
// WARN: Windows only
#[cfg(windows)]
#[derive(Deserialize, Clone)]
pub struct RegistryIndex {
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub program: String,
    #[serde(default, alias = "sub_class")]
    pub sub_category: String,
}

#[cfg(windows)]
impl From<&CleanerDataRegistry> for RegistryIndex {
    fn from(data: &CleanerDataRegistry) -> Self {
        Self {
            category: data.category.clone(),
            program: data.program.clone(),
            sub_category: data.sub_category.clone(),
        }
    }
}

fn default_class() -> String {
    String::from("Other")
}

// INFO: Built-in custom cleaning (defined in code, not JSON).
// Each entry describes: category, sub_category, target OS and the cleaning
// function itself. Functions are registered at runtime via
// database::custom_cleaners::register_custom_cleaner (see cleaner::custom_cleaners::register_all).
pub type CustomCleanFn =
    fn(&CustomCleaner, Option<tokio::sync::mpsc::Sender<String>>) -> CleanerResult;

#[derive(Clone)]
pub struct CustomCleaner {
    /// Unique id, used to enable/disable this cleaner from CLI/GUI
    pub id: String,
    pub program: String,
    pub category: String,
    pub sub_category: String,
    /// Target file or directory. Supports {username} placeholder
    pub path: String,
    /// Extra arguments passed to the cleaning function
    pub args: Vec<String>,
    /// Operating systems this cleaning applies to (empty = all)
    /// Values: "windows", "linux", "macos"
    pub os: Vec<String>,
    /// The cleaning function that performs the custom cleanup
    pub function: CustomCleanFn,
    /// If true, this cleaner runs sequentially (not concurrent with others)
    /// so that its progress messages don't mix with other cleaners.
    pub sequential: bool,
}

impl CustomCleaner {
    pub fn matches_current_os(&self) -> bool {
        if self.os.is_empty() {
            return true;
        }
        let current = if cfg!(windows) {
            "windows"
        } else if cfg!(target_os = "linux") {
            "linux"
        } else {
            "macos"
        };
        self.os.iter().any(|os| os.eq_ignore_ascii_case(current))
    }
}

// INFO: Struct for task clearing (Result cleared)
pub struct CleanerResult {
    pub files: u64,
    pub folders: u64,
    pub bytes: u64,
    pub working: bool,
    pub path: String,
    pub program: String,
    pub category: String,
    pub sub_category: String,
}
