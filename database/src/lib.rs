use crate::structures::CleanerData;

pub mod cleaner_database;
mod minecraft_launchers_database;
pub mod registry_database;
mod registry_utils;
pub mod structures;
pub mod utils;

pub fn get_pcbooster_version() -> String {
    String::from("1.9.3")
}
