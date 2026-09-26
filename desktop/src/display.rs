// ============================================================================
// Window backend selection
// ============================================================================
// Winit allows exactly one event loop per process: every `build()` after the
// first returns `EventLoopError::RecreationAttempt`, no matter how the first
// one failed. The renderer fallback loop in `main` therefore cannot react to a
// failed event-loop creation, and the backend has to be decided *before* the
// event loop is built.
//
// On Linux that is mostly a `sudo` problem. `sudo`'s `env_reset` drops
// `WAYLAND_DISPLAY`, `XDG_RUNTIME_DIR` and `XAUTHORITY`, so winit sees no
// compositor, falls back to X11 and dies with "Authorization required, but no
// authorization protocol specified". `detect` puts the session variables back
// by looking for the compositor socket in the *invoking* user's runtime
// directory (`sudo` keeps `SUDO_UID`), and `Display::apply` writes them into
// the environment while this process is still single-threaded.

use std::path::{Path, PathBuf};

/// Window backend to force when creating the event loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub enum Backend {
    Wayland,
    X11,
}

impl std::fmt::Display for Backend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Wayland => "Wayland",
            Self::X11 => "X11",
        })
    }
}

/// Backend choice together with the environment variables winit has to see.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Display {
    /// `None` leaves the choice to winit, which prefers Wayland as soon as
    /// `WAYLAND_DISPLAY` is set.
    pub backend: Option<Backend>,
    /// Variables to set before the event loop is built.
    pub env: Vec<(String, String)>,
}

impl Display {
    /// Writes the recovered variables into the environment.
    pub fn apply(&self) {
        for (key, value) in &self.env {
            eprintln!("Display: setting {key}={value}");
            // SAFETY: `main` calls this before the tokio runtime is built, so
            // this is the only thread of the process and none can read the
            // environment while it is being modified.
            unsafe { std::env::set_var(key, value) };
        }
        if !self.env.is_empty()
            && let Some(backend) = self.backend
        {
            eprintln!("Display: forcing the {backend} backend");
        }
    }
}

/// Picks a backend and recovers the session variables the environment is
/// missing. Must run before the event loop is created (see module docs).
pub fn detect() -> Display {
    #[cfg(not(target_os = "linux"))]
    {
        Display::default()
    }
    #[cfg(target_os = "linux")]
    {
        // A compositor is already announced: winit picks Wayland by itself.
        if env_var("WAYLAND_DISPLAY").is_some() || env_var("WAYLAND_SOCKET").is_some() {
            return Display::default();
        }

        // No `WAYLAND_DISPLAY`: look for the compositor socket of the session
        // this process was started from, which is the one to attach to even
        // when running through `sudo`.
        for dir in runtime_dirs() {
            if let Some(socket) = wayland_socket(&dir) {
                return Display {
                    backend: Some(Backend::Wayland),
                    env: vec![
                        ("WAYLAND_DISPLAY".to_string(), socket.display().to_string()),
                        ("XDG_RUNTIME_DIR".to_string(), dir.display().to_string()),
                    ],
                };
            }
        }

        // Without a compositor socket X11 is the only option left, but it needs
        // the cookie of the session user, which `sudo` does not forward.
        if env_var("DISPLAY").is_some() {
            return Display {
                backend: Some(Backend::X11),
                env: xauthority(),
            };
        }

        Display::default()
    }
}

/// A set-and-non-empty variable. winit treats an empty value as unset.
#[cfg(target_os = "linux")]
fn env_var(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|value| !value.is_empty())
}

/// Runtime directories to search for a compositor socket, most specific first:
/// the invoking user's (`sudo` hides the session of the user who typed the
/// command from the environment), then this process' own.
#[cfg(target_os = "linux")]
fn runtime_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut add = |dir: PathBuf| {
        if dir.is_dir() && !dirs.contains(&dir) {
            dirs.push(dir);
        }
    };

    for uid in [session_uid(), current_uid()].into_iter().flatten() {
        add(PathBuf::from(format!("/run/user/{uid}")));
    }
    if let Some(dir) = env_var("XDG_RUNTIME_DIR").map(PathBuf::from) {
        add(dir);
    }
    dirs
}

/// Newest `wayland-N` socket in `dir`. Compositors hand out increasing
/// numbers, so the highest one belongs to the running session while the lower
/// ones are leftovers of sessions that ended.
#[cfg(target_os = "linux")]
fn wayland_socket(dir: &Path) -> Option<PathBuf> {
    use std::os::unix::fs::FileTypeExt;

    let mut sockets: Vec<(u32, PathBuf)> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let number: u32 = entry
                .file_name()
                .to_str()?
                .strip_prefix("wayland-")?
                .parse()
                .ok()?;
            if !entry.file_type().ok()?.is_socket() {
                return None;
            }
            Some((number, entry.path()))
        })
        .collect();
    sockets.sort_by_key(|(number, _)| *number);
    sockets.pop().map(|(_, path)| path)
}

/// UID of the user who invoked `sudo`, as reported by `sudo` itself. Root
/// itself is never a session to attach to.
#[cfg(target_os = "linux")]
fn session_uid() -> Option<u32> {
    let uid = env_var("SUDO_UID")?.parse().ok()?;
    (uid != 0).then_some(uid)
}

/// Real UID of this process.
#[cfg(target_os = "linux")]
fn current_uid() -> Option<u32> {
    parse_status_uid(&std::fs::read_to_string("/proc/self/status").ok()?)
}

#[cfg(target_os = "linux")]
fn parse_status_uid(status: &str) -> Option<u32> {
    let uid_line = status.lines().find_map(|line| line.strip_prefix("Uid:"))?;
    uid_line.split_whitespace().next()?.parse().ok()
}

/// Cookie file for the X server of the session user, for when `sudo` dropped
/// `XAUTHORITY`. Empty when there is nothing to recover.
#[cfg(target_os = "linux")]
fn xauthority() -> Vec<(String, String)> {
    if env_var("XAUTHORITY").is_some() {
        return Vec::new();
    }
    let Some(uid) = session_uid() else {
        return Vec::new();
    };

    let runtime_dir = PathBuf::from(format!("/run/user/{uid}"));
    let mut candidates = vec![
        runtime_dir.join("gdm/Xauthority"),
        runtime_dir.join("xorg/Xauthority"),
    ];
    if let Some(home) = home_of(uid) {
        candidates.push(home.join(".Xauthority"));
    }
    candidates
        .into_iter()
        .find(|path| path.is_file())
        .map(|path| vec![("XAUTHORITY".to_string(), path.display().to_string())])
        .unwrap_or_default()
}

/// Home directory of `uid`. Read from `/etc/passwd` to avoid pulling in libc
/// just for `getpwuid`.
#[cfg(target_os = "linux")]
fn home_of(uid: u32) -> Option<PathBuf> {
    parse_passwd_home(&std::fs::read_to_string("/etc/passwd").ok()?, uid)
}

#[cfg(target_os = "linux")]
fn parse_passwd_home(passwd: &str, uid: u32) -> Option<PathBuf> {
    // INFO: name:password:uid:gid:gecos:home:shell
    passwd.lines().find_map(|line| {
        let mut fields = line.split(':');
        let (_name, _password, entry_uid, _gid, _gecos, home) = (
            fields.next()?,
            fields.next()?,
            fields.next()?,
            fields.next()?,
            fields.next()?,
            fields.next()?,
        );
        (entry_uid.parse::<u32>().ok()? == uid).then(|| PathBuf::from(home))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_status_uid_returns_real_uid() {
        let status = "Name:\tapp\nUid:\t1000\t1000\t1000\t1000\nGid:\t100\t100\t100\t100\n";
        assert_eq!(parse_status_uid(status), Some(1000));
    }

    #[test]
    fn test_parse_status_uid_without_uid_line() {
        assert_eq!(
            parse_status_uid("Name:\tapp\nGid:\t100\t100\t100\t100\n"),
            None
        );
        assert_eq!(parse_status_uid(""), None);
    }

    #[test]
    fn test_parse_passwd_home_finds_entry() {
        let passwd =
            "root:x:0:0:root:/root:/bin/bash\nroman:x:1000:1000:Laptop:/home/roman:/bin/bash\n";
        assert_eq!(
            parse_passwd_home(passwd, 1000),
            Some(PathBuf::from("/home/roman"))
        );
        assert_eq!(parse_passwd_home(passwd, 0), Some(PathBuf::from("/root")));
        assert_eq!(parse_passwd_home(passwd, 1234), None);
        assert_eq!(parse_passwd_home("", 0), None);
    }

    #[test]
    #[cfg(unix)]
    fn test_wayland_socket_picks_highest_number() {
        let dir = tempfile::tempdir().unwrap();
        // INFO: keep the listeners alive so their socket files stay in place
        let _listeners: Vec<_> = ["wayland-0", "wayland-2", "wayland-10"]
            .iter()
            .map(|name| std::os::unix::net::UnixListener::bind(dir.path().join(name)).unwrap())
            .collect();
        // INFO: a lock file shares the prefix but is not a socket
        std::fs::write(dir.path().join("wayland-2.lock"), []).unwrap();
        std::fs::write(dir.path().join("wayland-bus"), []).unwrap();

        assert_eq!(
            wayland_socket(dir.path()),
            Some(dir.path().join("wayland-10"))
        );
    }

    #[test]
    fn test_wayland_socket_without_compositor() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(wayland_socket(dir.path()), None);
        assert_eq!(wayland_socket(&dir.path().join("missing")), None);
    }
}
