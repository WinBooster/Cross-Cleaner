//! Configuration of the loaded cleaner.
//!
//! Everything is read from the environment: the library runs inside a program
//! that somebody else started, so there is no command line to pass options
//! through. `CROSS_CLEANER_*` variables are the only knobs, and every one of
//! them has a working default.

use std::path::PathBuf;
use std::time::Duration;

use crate::embed::Mode;
use crate::hotkey::Hotkey;
use crate::log;

/// What a hotkey press does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Show the window when hidden, hide it when shown (default).
    Toggle,
    /// Always show the window (a second press only re-focuses it).
    Show,
    /// Start a cleaning run over every category immediately.
    Clean,
}

impl std::fmt::Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Toggle => "toggle",
            Self::Show => "show",
            Self::Clean => "clean",
        })
    }
}

/// Graphics API for the cleaner window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// OpenGL, then DirectX 12, then Vulkan, skipping what the machine has no
    /// adapter for.
    Auto,
    /// Desktop OpenGL (glow / glutin).
    OpenGl,
    /// DirectX 12 (wgpu).
    DirectX,
    /// Vulkan (wgpu).
    Vulkan,
}

impl std::str::FromStr for Backend {
    type Err = String;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        match name.trim().to_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "opengl" | "gl" | "glow" => Ok(Self::OpenGl),
            "directx" | "dx" | "dx12" => Ok(Self::DirectX),
            "vulkan" | "vk" => Ok(Self::Vulkan),
            other => Err(format!(
                "unknown renderer {other:?} (expected auto, opengl, directx or vulkan)"
            )),
        }
    }
}

impl std::fmt::Display for Backend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Auto => "auto",
            Self::OpenGl => "OpenGL",
            Self::DirectX => "DirectX 12",
            Self::Vulkan => "Vulkan",
        })
    }
}

/// Everything the session needs to know.
#[derive(Debug, Clone)]
pub struct Config {
    /// Key that opens (or closes) the window.
    pub hotkey: Hotkey,
    /// What the hotkey does.
    pub action: Action,
    /// Graphics API for the window.
    pub backend: Backend,
    /// Where the window appears: inside the program's own window, as its own
    /// window, or whichever works.
    pub window: Mode,
    /// Show the window as soon as the library is loaded, instead of waiting for
    /// the first hotkey press.
    pub show_on_start: bool,
    /// Let the window's close button end the window thread instead of only
    /// hiding the window.
    pub exit_on_close: bool,
    /// Custom cleanup database, replacing the built-in one.
    pub database_path: Option<PathBuf>,
    /// Custom registry database (Windows only).
    pub registry_database_path: Option<PathBuf>,
    /// Disable the built-in custom cleanings.
    pub disable_custom: bool,
    /// Minimum time between two hotkey activations.
    pub debounce: Duration,
    /// File the log is appended to.
    pub log: Option<PathBuf>,
    /// Open the audio device for the UI sounds. Off by default: the host
    /// program may already be using it, and a library should not grab it.
    pub audio: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hotkey: Hotkey::default(),
            action: Action::Toggle,
            backend: Backend::Auto,
            window: Mode::Auto,
            show_on_start: false,
            exit_on_close: false,
            database_path: None,
            registry_database_path: None,
            disable_custom: false,
            debounce: Duration::from_millis(400),
            log: None,
            audio: false,
        }
    }
}

impl Config {
    /// Reads `CROSS_CLEANER_*` from the environment, falling back to the defaults
    /// for anything unset.
    ///
    /// A malformed value is reported and then ignored: the cleaner still has to
    /// come up, and a typo in an environment variable must not turn into a silent
    /// no-op.
    pub fn from_env() -> Self {
        let mut config = Self::default();

        if let Some(spec) = env_value("CROSS_CLEANER_HOTKEY") {
            match Hotkey::parse(&spec) {
                Ok(hotkey) => config.hotkey = hotkey,
                Err(e) => log::warn(&format!("ignoring CROSS_CLEANER_HOTKEY: {e}")),
            }
        }
        if let Some(spec) = env_value("CROSS_CLEANER_ACTION") {
            match spec.trim().to_lowercase().as_str() {
                "toggle" => config.action = Action::Toggle,
                "show" => config.action = Action::Show,
                "clean" | "quick" => config.action = Action::Clean,
                other => log::warn(&format!(
                    "ignoring CROSS_CLEANER_ACTION={other:?} (expected toggle, show or clean)"
                )),
            }
        }
        if let Some(spec) = env_value("CROSS_CLEANER_RENDERER") {
            match spec.parse() {
                Ok(backend) => config.backend = backend,
                Err(e) => log::warn(&format!("ignoring CROSS_CLEANER_RENDERER: {e}")),
            }
        }
        if let Some(spec) = env_value("CROSS_CLEANER_WINDOW") {
            match spec.parse() {
                Ok(window) => config.window = window,
                Err(e) => log::warn(&format!("ignoring CROSS_CLEANER_WINDOW: {e}")),
            }
        }
        if let Some(spec) = env_value("CROSS_CLEANER_SHOW_ON_START") {
            config.show_on_start = parse_bool(&spec);
        }
        if let Some(spec) = env_value("CROSS_CLEANER_EXIT_ON_CLOSE") {
            config.exit_on_close = parse_bool(&spec);
        }
        if let Some(spec) = env_value("CROSS_CLEANER_DATABASE") {
            config.database_path = Some(PathBuf::from(spec));
        }
        if let Some(spec) = env_value("CROSS_CLEANER_REGISTRY_DATABASE") {
            config.registry_database_path = Some(PathBuf::from(spec));
        }
        if let Some(spec) = env_value("CROSS_CLEANER_DISABLE_CUSTOM") {
            config.disable_custom = parse_bool(&spec);
        }
        if let Some(spec) = env_value("CROSS_CLEANER_DEBOUNCE_MS") {
            match spec.trim().parse::<u64>() {
                Ok(ms) => config.debounce = Duration::from_millis(ms),
                Err(e) => log::warn(&format!("ignoring CROSS_CLEANER_DEBOUNCE_MS: {e}")),
            }
        }
        if let Some(spec) = env_value("CROSS_CLEANER_LOG") {
            config.log = Some(PathBuf::from(spec));
        }
        if let Some(spec) = env_value("CROSS_CLEANER_AUDIO") {
            config.audio = parse_bool(&spec);
        }
        config
    }

    /// Sets up logging before anything else is reported, so the first line of
    /// the log is on disk too.
    pub fn configure_logging(&self) {
        if let Some(path) = &self.log {
            log::enable(path);
        }
    }
}

/// A set variable (empty values count as unset, matching winit's behaviour for
/// display variables).
fn env_value(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|value| !value.is_empty())
}

fn parse_bool(spec: &str) -> bool {
    matches!(
        spec.trim().to_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_matches_the_documented_behaviour() {
        let config = Config::default();
        assert_eq!(config.hotkey, Hotkey::default());
        assert_eq!(config.hotkey.to_string(), "delete");
        assert_eq!(config.action, Action::Toggle);
        assert_eq!(config.backend, Backend::Auto);
        assert_eq!(config.window, Mode::Auto);
        assert!(!config.show_on_start);
        assert!(!config.exit_on_close);
        assert_eq!(config.debounce, Duration::from_millis(400));
    }

    #[test]
    fn backend_names() {
        assert_eq!("auto".parse::<Backend>().unwrap(), Backend::Auto);
        assert_eq!("OpenGL".parse::<Backend>().unwrap(), Backend::OpenGl);
        assert_eq!("dx12".parse::<Backend>().unwrap(), Backend::DirectX);
        assert_eq!("vulkan".parse::<Backend>().unwrap(), Backend::Vulkan);
        assert!("opengles".parse::<Backend>().is_err());
    }

    #[test]
    fn bool_parsing() {
        for yes in ["1", "true", "YES", "on"] {
            assert!(parse_bool(yes), "{yes} should parse as true");
        }
        for no in ["0", "false", "no", "off", ""] {
            assert!(!parse_bool(no), "{no} should parse as false");
        }
    }
}