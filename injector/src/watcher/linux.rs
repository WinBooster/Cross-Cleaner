//! Linux implementation: the kernel input devices.
//!
//! There is no portable "watch a key globally" API on Linux, so the watcher reads
//! `/dev/input/event*` the way a window manager does: every input device is
//! opened and polled, and key events are matched without ever grabbing the
//! device, which is what keeps the host program's own Delete presses working.
//!
//! Reading the devices needs permission: either root, or membership of the
//! `input` group (`sudo usermod -aG input "$USER"`, then log out and back in).
//! Without it the devices cannot be opened, so the watcher keeps retrying and
//! keeps saying so.

use std::fs::File;
use std::io::Read;
use std::os::unix::io::AsRawFd;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::hotkey::Hotkey;
use crate::log;
use crate::watcher::WatcherError;

/// `EV_KEY` from `linux/input-event-codes.h`.
const EV_KEY: u16 = 0x01;
/// Value of a key-down / key-up event.
const KEY_PRESS: i32 = 1;
const KEY_RELEASE: i32 = 0;

/// Modifier keys tracked from the same event stream, so the hotkey can require
/// them without asking the X server or a window manager.
mod modifier {
    pub const LEFT_SHIFT: u16 = 42;
    pub const RIGHT_SHIFT: u16 = 54;
    pub const LEFT_CTRL: u16 = 29;
    pub const RIGHT_CTRL: u16 = 97;
    pub const LEFT_ALT: u16 = 56;
    pub const RIGHT_ALT: u16 = 100;

    pub const CTRL: u8 = 1 << 0;
    pub const ALT: u8 = 1 << 1;
    pub const SHIFT: u8 = 1 << 2;

    /// The modifier bit a key code belongs to. Left and right keys share one bit,
    /// so releasing one while the other is still down is not detected; that only
    /// matters for a hotkey that needs that modifier.
    pub fn bit_of(code: u16) -> Option<u8> {
        match code {
            LEFT_SHIFT | RIGHT_SHIFT => Some(SHIFT),
            LEFT_CTRL | RIGHT_CTRL => Some(CTRL),
            LEFT_ALT | RIGHT_ALT => Some(ALT),
            _ => None,
        }
    }
}

/// One opened device plus the modifier state of that keyboard.
struct Device {
    file: File,
    modifiers: u8,
}

/// State of the watcher: only touched from the watcher thread itself.
struct Watch {
    hotkey: Hotkey,
    debounce: Duration,
    on_press: Arc<dyn Fn() + Send + Sync>,
    /// Whether the hotkey key of *any* device is held. Kept outside the device list
    /// because the key can be released on a different device than it was pressed
    /// on (an external keyboard, for example).
    key_held: AtomicBool,
    /// Last activation, for the debounce.
    last_press: Mutex<Option<Instant>>,
}

impl Watch {
    fn modifiers_match(&self, modifiers: u8) -> bool {
        let want = modifier::CTRL * u8::from(self.hotkey.ctrl)
            | modifier::ALT * u8::from(self.hotkey.alt)
            | modifier::SHIFT * u8::from(self.hotkey.shift);
        modifiers & (modifier::CTRL | modifier::ALT | modifier::SHIFT) == want
    }

    fn activate(&self) {
        let mut last = self.last_press.lock().expect("debounce mutex poisoned");
        let now = Instant::now();
        if let Some(previous) = *last
            && now.duration_since(previous) < self.debounce
        {
            return;
        }
        *last = Some(now);
        drop(last);
        (self.on_press)();
    }
}

/// How often the device list is refreshed. Devices appear when a keyboard is
/// plugged in, and permissions may only be granted while we are running.
const RESCAN_INTERVAL: Duration = Duration::from_secs(2);
/// `poll` timeout, so the rescan above is reached even without any input.
const POLL_TIMEOUT: Duration = Duration::from_millis(500);
/// Rate limit for the "cannot read the keyboard" warning, so a missing group does
/// not fill the log.
const WARN_INTERVAL: Duration = Duration::from_secs(30);
/// Set while the library is being unloaded; the poll loop then ends.
static UNLOADING: AtomicBool = AtomicBool::new(false);

/// See [`crate::watcher::shutdown`].
pub fn shutdown() {
    UNLOADING.store(true, Ordering::SeqCst);
}

pub fn spawn(
    hotkey: Hotkey,
    debounce: Duration,
    on_press: Arc<dyn Fn() + Send + Sync>,
) -> Result<JoinHandle<()>, WatcherError> {
    std::thread::Builder::new()
        .name("cross-clean-hotkey".to_string())
        .spawn(move || {
            let watch = Watch {
                hotkey,
                debounce,
                on_press,
                key_held: AtomicBool::new(false),
                last_press: Mutex::new(None),
            };
            run(&watch);
        })
        .map_err(|e| WatcherError::Failed(format!("cannot spawn the watcher thread: {e}")))
}

/// Opens every readable input device and polls them until the process ends.
fn run(watch: &Watch) {
    let mut devices: Vec<Device> = Vec::new();
    let mut last_scan = Instant::now() - RESCAN_INTERVAL;
    let mut last_warn: Option<Instant> = None;

    log::info(&format!(
        "watching for {} on /dev/input (evdev) in pid {}",
        watch.hotkey,
        std::process::id()
    ));

    loop {
        if UNLOADING.load(Ordering::SeqCst) {
            log::info("key watcher stopped");
            return;
        }
        if last_scan.elapsed() >= RESCAN_INTERVAL {
            match open_devices() {
                Some(fresh) => {
                    if !fresh.is_empty() {
                        log::info(&format!("reading keyboard events from {} devices", fresh.len()));
                    }
                    devices = fresh;
                }
                None => {
                    if last_warn.is_none_or(|at| at.elapsed() >= WARN_INTERVAL) {
                        log::warn(&format!(
                            "cannot open /dev/input/event*: run as root or add the user to the \
                             \"input\" group, otherwise the {} hotkey stays inactive",
                            watch.hotkey
                        ));
                        last_warn = Some(Instant::now());
                    }
                    devices.clear();
                }
            }
            last_scan = Instant::now();
        }

        let mut fds: Vec<libc::pollfd> = devices
            .iter()
            .map(|device| libc::pollfd {
                fd: device.file.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            })
            .collect();

        // SAFETY: `fds` is a live, correctly sized slice of `pollfd`.
        let ready = unsafe {
            libc::poll(
                fds.as_mut_ptr(),
                fds.len() as libc::nfds_t,
                POLL_TIMEOUT.as_millis() as libc::c_int,
            )
        };
        if ready < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            log::error(&format!("poll failed: {error}"));
            std::thread::sleep(Duration::from_millis(500));
            continue;
        }

        for (index, entry) in fds.iter().enumerate() {
            if entry.revents == 0 {
                continue;
            }
            let Some(device) = devices.get_mut(index) else {
                continue;
            };
            read_events(device, watch);
        }
    }
}

/// Opens all readable event devices.
///
/// Returns `Some` with the devices it could open, or `None` when *nothing* is
/// readable, which is the permission problem. A partially readable setup (only
/// some devices granted) keeps what it got.
fn open_devices() -> Option<Vec<Device>> {
    let entries = std::fs::read_dir("/dev/input").ok()?;
    let mut devices = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if !is_event_node(&path) {
            continue;
        }
        let Ok(file) = File::open(&path) else {
            continue;
        };
        devices.push(Device {
            file,
            modifiers: 0,
        });
    }

    if devices.is_empty() {
        return None;
    }
    Some(devices)
}

/// `/dev/input/eventN`, without following the (many) other nodes in that dir.
fn is_event_node(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_prefix("event"))
        .is_some_and(|number| !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()))
}

/// Reads and interprets every complete event in the device's buffer.
fn read_events(device: &mut Device, watch: &Watch) {
    let size = std::mem::size_of::<libc::input_event>();
    let mut buffer = [0u8; 64];
    loop {
        let read = match device.file.read(&mut buffer) {
            Ok(0) | Err(_) => return,
            Ok(read) => read,
        };
        let mut offset = 0;
        while offset + size <= read {
            // SAFETY: `buffer` is `size` bytes long at this offset, and the kernel
            // writes whole `input_event` records into it.
            let event: libc::input_event =
                unsafe { std::ptr::read_unaligned(buffer.as_ptr().add(offset).cast()) };
            handle_event(&mut device.modifiers, event, watch);
            offset += size;
        }
        if read < size {
            return;
        }
    }
}

/// Matches one event against the hotkey.
fn handle_event(modifiers: &mut u8, event: libc::input_event, watch: &Watch) {
    if event.type_ != EV_KEY {
        return;
    }
    if event.code == watch.hotkey.key.linux_code() {
        match event.value {
            KEY_PRESS => {
                // Fire on the transition only: a held key repeats KEY_PRESS faster
                // than any debounce could keep up with.
                if watch.key_held.swap(true, Ordering::SeqCst) {
                    return;
                }
                if watch.modifiers_match(*modifiers) {
                    watch.activate();
                }
            }
            KEY_RELEASE => {
                watch.key_held.store(false, Ordering::SeqCst);
            }
            _ => {}
        }
        return;
    }
    // Track the modifiers as they are pressed and released. `value == 2` is a key
    // repeat, which leaves the state as it is.
    if let Some(bit) = modifier::bit_of(event.code) {
        match event.value {
            KEY_PRESS => *modifiers |= bit,
            KEY_RELEASE => *modifiers &= !bit,
            _ => {}
        }
    }
}