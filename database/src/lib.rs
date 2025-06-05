use crate::structures::CleanerData;
use std::env;

pub mod cleaner_database;
pub mod registry_database;
mod registry_utils;
pub mod structures;
pub mod utils;

pub fn get_version() -> String {
    env::var("APP_VERSION").unwrap_or_else(|_| String::from("1.9.7"))
}

pub fn get_icon() -> &'static [u8; 3216] {
    let bytes: &'static [u8; 3216] = include_bytes!("../../assets/icon.png");
    bytes
}
