use crate::structures::CleanerData;

pub mod structures;
pub mod cleaner_database;
pub mod registry_database;
pub mod utils;
mod registry_utils;
mod minecraft_launchers_database;

pub fn get_pcbooster_version() -> String {
    String::from("1.9.3")
}