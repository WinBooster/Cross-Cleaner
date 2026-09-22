use serde::{Deserialize, Deserializer, Serialize};
use std::sync::{LazyLock, Mutex};

fn default_volume() -> f32 {
    1.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeConfig {
    #[serde(default = "default_volume")]
    pub done: f32,
    #[serde(default = "default_volume")]
    pub checkbox: f32,
    #[serde(default = "default_volume")]
    pub click: f32,
    #[serde(default = "default_volume")]
    pub popup: f32,
}

impl Default for VolumeConfig {
    fn default() -> Self {
        Self {
            done: 1.0,
            checkbox: 1.0,
            click: 1.0,
            popup: 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct AppConfig {
    pub volume: VolumeConfig,
    #[serde(default)]
    pub auto_update: bool,
}

// Backward-compatible deserialization: supports both new nested `volume: {done, checkbox, click, popup}`
// and old flat `sound_volume, click_volume, check_volume, done_volume`.
#[derive(Deserialize)]
struct RawVolume {
    #[serde(default = "default_volume")]
    done: f32,
    #[serde(default = "default_volume")]
    checkbox: f32,
    #[serde(default = "default_volume")]
    click: f32,
    #[serde(default = "default_volume")]
    popup: f32,
}

#[derive(Deserialize)]
struct RawAppConfig {
    volume: Option<RawVolume>,
    #[serde(default)]
    auto_update: Option<bool>,
    #[serde(default)]
    auto_updates: Option<bool>,
    // old flat aliases
    sound_volume: Option<f32>,
    click_volume: Option<f32>,
    check_volume: Option<f32>,
    done_volume: Option<f32>,
}

impl<'de> Deserialize<'de> for AppConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawAppConfig::deserialize(deserializer)?;

        // Resolve volume: start from nested if present, else defaults, then overlay flat old keys.
        let mut volume = if let Some(v) = raw.volume {
            VolumeConfig {
                done: v.done,
                checkbox: v.checkbox,
                click: v.click,
                popup: v.popup,
            }
        } else {
            VolumeConfig::default()
        };

        // Old flat keys override if present (migration path)
        if let Some(v) = raw.sound_volume {
            volume.popup = v;
        }
        if let Some(v) = raw.click_volume {
            volume.click = v;
        }
        if let Some(v) = raw.check_volume {
            volume.checkbox = v;
        }
        if let Some(v) = raw.done_volume {
            volume.done = v;
        }

        let auto_update = raw
            .auto_update
            .or(raw.auto_updates)
            .unwrap_or(false);

        Ok(Self { volume, auto_update })
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
