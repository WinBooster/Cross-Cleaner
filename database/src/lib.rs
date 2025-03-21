use crate::structures::CleanerData;

pub mod cleaner_database;
pub mod registry_database;
mod registry_utils;
pub mod structures;
pub mod utils;

pub fn get_pcbooster_version() -> String {
    String::from("1.9.5")
}

pub fn get_icon() -> &'static [u8; 1024 * 4] {
    let bytes: &'static [u8; 1024 * 4] = include_bytes!("../../assets/icon.png");
    bytes
}