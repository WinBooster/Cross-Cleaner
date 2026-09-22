use serde::{Deserialize, Serialize};
use std::sync::{LazyLock, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub sound_volume: f32,
    pub click_volume: f32,
    pub check_volume: f32,
    pub done_volume: f32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            sound_volume: 1.0,
            click_volume: 1.0,
            check_volume: 1.0,
            done_volume: 1.0,
        }
    }
}

static CONFIG: LazyLock<Mutex<AppConfig>> = LazyLock::new(|| Mutex::new(load()));

fn config_path() -> Option<std::path::PathBuf> {
    std::env::current_exe()
        .ok()?
        .parent()
        .map(|p| p.join("config.json"))
}

fn load() -> AppConfig {
    let Some(path) = config_path() else {
        return AppConfig::default();
    };
    let Ok(data) = std::fs::read_to_string(&path) else {
        return AppConfig::default();
    };
    serde_json::from_str(&data).unwrap_or_default()
}

pub fn save(cfg: &AppConfig) {
    let Some(path) = config_path() else {
        return;
    };
    if let Ok(json) = serde_json::to_string_pretty(cfg) {
        let _ = std::fs::write(path, json);
    }
}

pub fn get() -> AppConfig {
    CONFIG.lock().unwrap().clone()
}

pub fn update(f: impl FnOnce(&mut AppConfig)) {
    let mut cfg = CONFIG.lock().unwrap();
    f(&mut cfg);
    save(&cfg);
}
