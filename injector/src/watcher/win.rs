//! Windows implementation: a low-level keyboard hook.
//!
//! `WH_KEYBOARD_LL` is the only hook that sees keys before the foreground
//! application gets them, which is exactly what is needed when the cleaner runs
//! inside somebody else's window. The hook is installed on the watcher thread
//! and that thread pumps messages, because Windows only dispatches a low-level
//! hook on a thread that owns a message queue.
//!
//! The key is never swallowed: the callback always returns the result of
//! `CallNextHookEx`, so the host program keeps seeing its own Delete presses.

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::thread::JoinHandle;

use windows::Win32::Foundation::{HINSTANCE, HMODULE, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::{
    GetModuleHandleExW, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_CONTROL, VK_MENU, VK_SHIFT};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, HHOOK, KBDLLHOOKSTRUCT, MSG, PeekMessageW,
    SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, HC_ACTION, PM_NOREMOVE, WM_KEYDOWN,
    WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP, WH_KEYBOARD_LL,
};
use windows::core::PCWSTR;

use crate::hotkey::Hotkey;
use crate::log;
use crate::watcher::WatcherError;

/// Handle of the loaded library, remembered from `DllMain` so the hook can name
/// its own module instead of relying on the caller's address space.
static MODULE: AtomicIsize = AtomicIsize::new(0);

/// Remembers the module handle `DllMain` received.
pub fn set_module_handle(instance: HINSTANCE) {
    MODULE.store(instance.0 as isize, Ordering::Relaxed);
}

/// State shared between the message pump and the hook callback.
struct Watch {
    hotkey: Hotkey,
    debounce_ms: u64,
    on_press: Arc<dyn Fn() + Send + Sync>,
    /// `millis()` of the last activation, for the debounce.
    last_press: AtomicU64,
    /// Whether the hotkey key is currently held, so the auto-repeat stream does
    /// not toggle the window over and over.
    key_held: AtomicBool,
}

/// The single installed hook. Windows has one message loop per thread, so a
/// process cannot run two independent low-level keyboard hooks for this library
/// anyway.
static WATCH: OnceLock<&'static Watch> = OnceLock::new();
/// Handle of the installed hook, needed to pass it to `CallNextHookEx`.
static HOOK_HANDLE: AtomicIsize = AtomicIsize::new(0);
/// Set while the library is being unloaded: the callback then does nothing.
static UNLOADING: AtomicBool = AtomicBool::new(false);

/// See [`crate::watcher::shutdown`].
pub fn shutdown() {
    UNLOADING.store(true, Ordering::SeqCst);
}

impl Watch {
    /// Milliseconds since the unix epoch; the hook callback cannot use `Instant`
    /// (it would need a thread-local) and sub-second precision is not needed
    /// anyway.
    fn millis() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    /// True when the modifiers currently held match the ones the hotkey wants.
    fn modifiers_match(&self) -> bool {
        let held = |key: u16| unsafe { GetAsyncKeyState(key as i32) < 0 };
        held(VK_CONTROL.0) == self.hotkey.ctrl
            && held(VK_MENU.0) == self.hotkey.alt
            && held(VK_SHIFT.0) == self.hotkey.shift
    }
}

pub fn spawn(
    hotkey: Hotkey,
    debounce: std::time::Duration,
    on_press: Arc<dyn Fn() + Send + Sync>,
) -> Result<JoinHandle<()>, WatcherError> {
    let watch: &'static Watch = Box::leak(Box::new(Watch {
        hotkey,
        debounce_ms: debounce.as_millis() as u64,
        on_press,
        last_press: AtomicU64::new(0),
        key_held: AtomicBool::new(false),
    }));
    if WATCH.set(watch).is_err() {
        return Err(WatcherError::Failed(
            "a key watcher is already running in this process".to_string(),
        ));
    }

    // The hook callback runs on this thread, which needs its own message queue
    // before the first `GetMessageW`.
    let mut message: MSG = unsafe { std::mem::zeroed() };
    let _ = unsafe { PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE) };

    let Some(module) = module_handle() else {
        return Err(WatcherError::Failed(
            "cannot determine the DLL module handle for the keyboard hook".to_string(),
        ));
    };
    let hook = unsafe {
        SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(hook_proc),
            Some(HINSTANCE(module.0)),
            0,
        )
    }
    .map_err(|e| WatcherError::Failed(format!("SetWindowsHookExW failed: {e}")))?;
    HOOK_HANDLE.store(hook.0 as isize, Ordering::Relaxed);

    log::info(&format!(
        "watching for {hotkey} (WH_KEYBOARD_LL) in pid {}",
        std::process::id()
    ));

    // `HHOOK` wraps a raw pointer, which is not `Send`: the watcher thread gets
    // the address and rebuilds the handle.
    let hook_address = hook.0 as isize;

    Ok(std::thread::Builder::new()
        .name("cross-clean-hotkey".to_string())
        .spawn(move || pump(hook_address))
        .map_err(|e| WatcherError::Failed(format!("cannot spawn the watcher thread: {e}")))?)
}

/// Message loop of the watcher thread.
fn pump(hook: isize) {
    let hook = HHOOK(hook as *mut c_void);
    let mut message: MSG = unsafe { std::mem::zeroed() };
    unsafe {
        // `GetMessageW` returns FALSE for both WM_QUIT and an error; either way
        // there is nothing left to pump.
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        let _ = UnhookWindowsHookEx(hook);
    }
    log::info("keyboard hook removed");
}

/// The low-level keyboard hook.
unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let handle = HHOOK(HOOK_HANDLE.load(Ordering::Relaxed) as *mut c_void);
    // Never swallow the event: the foreground application must keep working.
    let pass_on = || unsafe { CallNextHookEx(Some(handle), code, wparam, lparam) };

    if code != HC_ACTION as i32 || UNLOADING.load(Ordering::Relaxed) {
        return pass_on();
    }
    let Some(watch) = WATCH.get() else {
        return pass_on();
    };
    let message = wparam.0 as u32;
    let event = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
    if event.vkCode != watch.hotkey.key.windows_vk() {
        return pass_on();
    }

    match message {
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            // Fire on the press transition only: a held key repeats WM_KEYDOWN
            // faster than any debounce could keep up with.
            if watch.key_held.swap(true, Ordering::Relaxed) {
                return pass_on();
            }
            if !watch.modifiers_match() {
                return pass_on();
            }
            let now = Watch::millis();
            let last = watch.last_press.load(Ordering::Relaxed);
            if last != 0 && now.saturating_sub(last) < watch.debounce_ms {
                return pass_on();
            }
            watch.last_press.store(now, Ordering::Relaxed);
            (watch.on_press)();
        }
        WM_KEYUP | WM_SYSKEYUP => {
            watch.key_held.store(false, Ordering::Relaxed);
        }
        _ => {}
    }
    pass_on()
}

/// Module that owns this library, for the hook handle.
fn module_handle() -> Option<HMODULE> {
    let cached = MODULE.load(Ordering::Relaxed);
    if cached != 0 {
        return Some(HMODULE(cached as *mut c_void));
    }
    // Not loaded through `DllMain` (the CLI embeds the library through the
    // rlib): ask the loader which module the callback lives in.
    let address: unsafe extern "system" fn(i32, WPARAM, LPARAM) -> LRESULT = hook_proc;
    let mut module = HMODULE(std::ptr::null_mut());
    unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
            PCWSTR(address as *const u16),
            &mut module,
        )
    }
    .ok()?;
    Some(module)
}