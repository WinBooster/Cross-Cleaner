use crate::utils;
use serde::{Deserialize, Serialize};
use tabled::Tabled;

#[derive(PartialEq, Tabled)]
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

fn display_categories(categories: &Vec<String>) -> String {
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

#[derive(Serialize, Deserialize, Clone)]
pub struct CleanerData {
    pub path: String,
    pub category: String,
    pub program: String,
    #[serde(default = "default_class")]
    pub class: String,

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

fn default_class() -> String {
    String::from("Other")
}

pub struct CleanerResult {
    pub files: u64,
    pub folders: u64,
    pub bytes: u64,
    pub working: bool,
    pub path: String,
    pub program: String,
    pub category: String,
}
