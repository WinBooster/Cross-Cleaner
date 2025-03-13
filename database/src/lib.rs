use crate::structures::CleanerData;

pub mod structures;
pub mod cleaner_database;
pub mod registry_database;
pub mod utils;
mod registry_utils;

pub fn get_winbooster_version() -> String {
    String::from("1.8.9")
}