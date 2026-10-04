//! Putting the cleaner window *inside* the program's own window.
//!
//! Without this the window is a second, independent top-level window: it has its
//! own taskbar entry and sits on top of everything, which reads as "some other
//! program popped up". Parenting it to the program's own window (`SetParent`
//! with `WS_CHILD`) makes it a part of that window instead: it is clipped by it,
//! hidden when it is minimized, moves with it, and it is gone when the program
//! exits.
//!
//! Only Windows has child windows. On the other window systems the cleaner stays
//! a normal top-level window, which is what [`Mode::Window`] asks for anyway.
//!
//! # What a child window costs
//!
//! * It can only be moved inside its parent, and it is clipped by it.
//! * A program that draws its own surface (a game with a DirectX or Vulkan swap
//!   chain, for example) may paint over it. Those programs want [`Mode::Window`],
//!   where the cleaner stays on top of everything.
//! * When the program destroys its window, ours goes with it, which is why the
//!   hotkey cannot bring the window back after that.
//!
//! Dragging is handled here rather than by `ViewportCommand::StartDrag`, because
//! that one is a no-op for a child window: the mouse is tracked instead.

/// Where the cleaner window lives on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Inside the program's own window when one can be found, a normal window
    /// otherwise.
    Auto,
    /// Always a child of the program's own window.
    Embedded,
    /// Always an independent top-level window.
    Window,
}

impl std::str::FromStr for Mode {
    type Err = String;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        match name.trim().to_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "embedded" | "child" | "inside" => Ok(Self::Embedded),
            "window" | "free" => Ok(Self::Window),
            other => Err(format!(
                "unknown window mode {other:?} (expected auto, embedded or window)"
            )),
        }
    }
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Auto => "auto",
            Self::Embedded => "embedded",
            Self::Window => "window",
        })
    }
}

/// Height of the title bar, matching `gui::title_bar::TITLE_BAR_HEIGHT`: a press
/// above it drags the window.
const TITLE_BAR: f32 = 32.0;
/// Width reserved for the title bar buttons on the right, which stay clickable
/// while the rest of the bar drags the window.
const TITLE_BAR_BUTTONS: f32 = 162.0;

// ============================================================================
// Windows
// ============================================================================

#[cfg(windows)]
mod platform {
    use std::sync::atomic::{AtomicIsize, Ordering};
    use std::sync::{Mutex, OnceLock};

    use super::{log, TITLE_BAR, TITLE_BAR_BUTTONS};
    use windows::Win32::Foundation::{HWND, LPARAM, RECT};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetAncestor, GetClientRect, GetForegroundWindow, GetWindowLongPtrW,
        GetWindowRect, GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible, SetParent,
        SetWindowLongPtrW, SetWindowPos, GA_ROOT, GWL_STYLE, HWND_TOP, SWP_FRAMECHANGED,
        SWP_NOACTIVATE, SWP_NOZORDER, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, WS_CHILD, WS_POPUP,
    };
    use windows::core::BOOL;

    /// The program window the cleaner is a child of.
    static HOST: AtomicIsize = AtomicIsize::new(0);
    /// Our own window while it is a child window.
    static CHILD: AtomicIsize = AtomicIsize::new(0);
    /// Position of the window inside the host window, packed as `(x << 32) | y`.
    static ORIGIN: AtomicIsize = AtomicIsize::new(0);

    /// A drag in progress. Only touched from the window thread, so a mutex is
    /// enough.
    #[derive(Clone, Copy)]
    struct Drag {
        /// Where the button went down, in window coordinates.
        grab_x: f64,
        grab_y: f64,
        /// Position of the window at that moment.
        origin_x: i32,
        origin_y: i32,
    }

    static DRAG: OnceLock<Mutex<Option<Drag>>> = OnceLock::new();

    fn drag_slot() -> &'static Mutex<Option<Drag>> {
        DRAG.get_or_init(|| Mutex::new(None))
    }

    fn hwnd(value: isize) -> HWND {
        HWND(value as *mut std::ffi::c_void)
    }

    fn store_origin(x: i32, y: i32) {
        ORIGIN.store(((x as isize) << 32) | (y as u32 as isize), Ordering::SeqCst);
    }

    fn origin() -> (i32, i32) {
        let packed = ORIGIN.load(Ordering::SeqCst);
        ((packed >> 32) as i32, (packed & 0xffff_ffff) as i32)
    }

    /// Remembers the host window for the next [`attach`].
    ///
    /// Called while the hotkey is being pressed, when the window the user is
    /// looking at is the foreground window of *this* process - the host
    /// program's main window. The foreground window is only a hint though: the
    /// hotkey can also be pressed while another program has the focus, and the
    /// cleaner still belongs inside this one. So the visible top-level windows of
    /// this process are searched as a fallback, and the biggest one wins, which for
    /// every normal program is its main window.
    ///
    /// `own` is our own window handle, which must never become the host.
    pub fn capture_host(own: isize) {
        let foreground = unsafe { GetForegroundWindow() };
        if !foreground.0.is_null()
            && foreground.0 as isize != own
            && belongs_to_us(foreground)
            && is_usable_host(foreground)
        {
            HOST.store(foreground.0 as isize, Ordering::SeqCst);
            return;
        }
        if let Some(window) = largest_host_window(own) {
            HOST.store(window.0 as isize, Ordering::SeqCst);
        }
    }

    /// Forgets the host window, e.g. because the window was closed.
    pub fn forget_host() {
        HOST.store(0, Ordering::SeqCst);
    }

    /// The remembered host window, if it still exists.
    pub fn host() -> Option<isize> {
        let host = HOST.load(Ordering::SeqCst);
        if host == 0 {
            return None;
        }
        if !unsafe { IsWindow(Some(hwnd(host))).as_bool() } {
            HOST.store(0, Ordering::SeqCst);
            return None;
        }
        Some(host)
    }

    /// Whether the host window is minimized, in which case the cleaner (as its
    /// child) is hidden by the window manager anyway.
    pub fn host_minimized() -> bool {
        host().is_some_and(|host| unsafe { IsIconic(hwnd(host)).as_bool() })
    }

    /// Whether `window` belongs to this process.
    fn belongs_to_us(window: HWND) -> bool {
        let mut pid = 0;
        unsafe { GetWindowThreadProcessId(window, Some(&mut pid)) };
        pid == std::process::id()
    }

    /// A window that can host the cleaner: top-level, visible, and with a client
    /// area worth showing the cleaner in.
    fn is_usable_host(window: HWND) -> bool {
        if !unsafe { IsWindowVisible(window).as_bool() } {
            return false;
        }
        // `GA_ROOT` is the window itself only for a top-level window, which is
        // what we want: the cleaner must not be parented to another child.
        if unsafe { GetAncestor(window, GA_ROOT) } != window {
            return false;
        }
        client_size(window).is_some_and(|(width, height)| width > 200 && height > 200)
    }

    /// Client area of a window, or `None` when it cannot be read.
    fn client_size(window: HWND) -> Option<(i32, i32)> {
        let mut rect = RECT::default();
        unsafe { GetClientRect(window, &mut rect) }.ok()?;
        Some((rect.right - rect.left, rect.bottom - rect.top))
    }

    /// Size of a window in pixels. For a child window `GetWindowRect` reports
    /// screen coordinates, but the size is still the window's own.
    fn window_size(window: HWND) -> (i32, i32) {
        let mut rect = RECT::default();
        // SAFETY: `rect` is a live, correctly sized `RECT`.
        if unsafe { GetWindowRect(window, &mut rect) }.is_err() {
            return (0, 0);
        }
        (rect.right - rect.left, rect.bottom - rect.top)
    }

    /// Turns our window into a child of the host window, or gives it back its own
    /// top-level status.
    ///
    /// `window` is the cleaner's own window and has to exist already, which is
    /// why this runs from the first frame rather than from the hotkey.
    pub fn attach(window: isize, embedded: bool) -> bool {
        let host = if embedded { host() } else { None };
        match host {
            Some(host) if host != window => {
                let (child, parent) = (hwnd(window), hwnd(host));
                let style = unsafe { GetWindowLongPtrW(child, GWL_STYLE) };
                let wanted = (style & !(WS_POPUP.0 as isize)) | WS_CHILD.0 as isize;
                if style != wanted {
                    unsafe { SetWindowLongPtrW(child, GWL_STYLE, wanted) };
                }
                // SAFETY: both windows belong to this process.
                unsafe { SetParent(child, Some(parent)) }.ok();
                CHILD.store(window, Ordering::SeqCst);
                store_origin(0, 0);
                place(child, parent, None);
                // The style change only takes effect after the frame is
                // recomputed; the cleaner window is undecorated anyway, so only
                // the position has to be re-applied here.
                unsafe {
                    SetWindowPos(
                        child,
                        Some(HWND_TOP),
                        0,
                        0,
                        0,
                        0,
                        SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                    )
                }
                .ok();
                log::info(&format!("embedded into host window 0x{host:x}"));
                start_follower();
                true
            }
            _ => {
                detach();
                false
            }
        }
    }

    /// Gives the window back its own top-level status.
    pub fn detach() {
        let child = CHILD.swap(0, Ordering::SeqCst);
        if child == 0 {
            return;
        }
        if let Ok(mut slot) = drag_slot().lock() {
            *slot = None;
        }
        let child = hwnd(child);
        // SAFETY: the handle is a window of this process.
        unsafe {
            SetParent(child, None).ok();
            let style = GetWindowLongPtrW(child, GWL_STYLE);
            let wanted = (style & !WS_CHILD.0 as isize) | WS_POPUP.0 as isize;
            SetWindowLongPtrW(child, GWL_STYLE, wanted);
            SetWindowPos(
                child,
                Some(HWND_TOP),
                0,
                0,
                0,
                0,
                SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            )
            .ok();
        }
        log::info("window detached from the host program");
    }

    /// Whether our window currently is a child window.
    pub fn is_embedded() -> bool {
        CHILD.load(Ordering::SeqCst) != 0
    }

    /// Recomputes the position of the window inside its parent.
    ///
    /// `center` puts it in the middle of the host window, which is what showing
    /// the cleaner again should do. Without it the window keeps the offset it
    /// had, so dragging it inside the program sticks.
    pub fn layout(window: isize, center: bool) {
        if CHILD.load(Ordering::SeqCst) != window {
            return;
        }
        let Some(host) = host() else {
            return;
        };
        place(
            hwnd(window),
            hwnd(host),
            if center { None } else { Some(origin()) },
        );
    }

    /// Moves the window to `position`, or centers it, keeping it inside the host
    /// window's client area.
    fn place(window: HWND, host: HWND, position: Option<(i32, i32)>) {
        let Some((host_width, host_height)) = client_size(host) else {
            return;
        };
        let (width, height) = window_size(window);
        if width <= 0 || height <= 0 {
            return;
        }
        let width = width.min(host_width);
        let height = height.min(host_height);

        let (x, y) = match position {
            None => ((host_width - width) / 2, (host_height - height) / 2),
            Some(position) => position,
        };
        let x = x.clamp(0, (host_width - width).max(0));
        let y = y.clamp(0, (host_height - height).max(0));
        store_origin(x, y);

        // SAFETY: the window is ours and the parent belongs to this process.
        // A child window's coordinates are relative to the parent's client area.
        unsafe {
            SetWindowPos(
                window,
                Some(HWND_TOP),
                x,
                y,
                width,
                height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )
        }
        .ok();
    }

    /// Keeps the embedded window inside the host window.
    ///
    /// A child window keeps its position when the parent is moved, but it has to
    /// be clamped again when the parent is *resized*, and it is gone when the
    /// parent is destroyed. Both need watching, and winit has no event for either
    /// while the cleaner sits idle, so this is a small poll.
    fn start_follower() {
        static STARTED: OnceLock<()> = OnceLock::new();
        if STARTED.set(()).is_err() {
            return;
        }
        let _ = std::thread::Builder::new()
            .name("cross-clean-follow".to_string())
            .spawn(|| {
                use std::time::Duration;
                let mut size: Option<(i32, i32)> = None;
                loop {
                    std::thread::sleep(Duration::from_millis(200));
                    let child = CHILD.load(Ordering::SeqCst);
                    if child == 0 {
                        // Detached or never attached: nothing to follow.
                        return;
                    }
                    let Some(host) = host() else {
                        // The program closed its window, which took ours with it:
                        // go back to being a window of our own.
                        detach();
                        return;
                    };
                    let current = client_size(hwnd(host));
                    if current != size {
                        size = current;
                        layout(child, false);
                    }
                }
            });
    }

    /// Mouse handling for the embedded window.
    ///
    /// `cursor` is the position inside the window, `pressed` is `Some(true)` on a
    /// left button press and `Some(false)` on the release. Returns `true` when the
    /// event was used for dragging.
    pub fn pointer(window: isize, cursor: Option<(f64, f64)>, pressed: Option<bool>) -> bool {
        if CHILD.load(Ordering::SeqCst) != window {
            return false;
        }
        let Ok(mut slot) = drag_slot().lock() else {
            return false;
        };

        match pressed {
            Some(true) => {
                let Some((x, y)) = cursor else {
                    return false;
                };
                // Only the empty part of the title bar drags the window; the
                // buttons on the right stay clickable.
                let too_far_right =
                    x > window_size(hwnd(window)).0 as f64 - TITLE_BAR_BUTTONS as f64;
                if y > TITLE_BAR as f64 || too_far_right {
                    return false;
                }
                let (origin_x, origin_y) = origin();
                *slot = Some(Drag {
                    grab_x: x,
                    grab_y: y,
                    origin_x,
                    origin_y,
                });
                true
            }
            Some(false) => {
                if slot.is_some() {
                    *slot = None;
                    return true;
                }
                false
            }
            None => {
                let Some(drag) = *slot else {
                    return false;
                };
                let Some((x, y)) = cursor else {
                    return false;
                };
                if let Some(host) = host() {
                    place(
                        hwnd(window),
                        hwnd(host),
                        Some((
                            drag.origin_x + (x - drag.grab_x) as i32,
                            drag.origin_y + (y - drag.grab_y) as i32,
                        )),
                    );
                }
                true
            }
        }
    }

    /// State of the [`largest_host_window`] search, reached through `LPARAM`.
    struct Search {
        own: isize,
        best: Option<(HWND, i64)>,
    }

    /// Keeps the biggest usable window of this process.
    unsafe extern "system" fn pick_host(window: HWND, parameter: LPARAM) -> BOOL {
        // SAFETY: `parameter` is the `LPARAM` passed below, a live `Search`.
        let search = unsafe { &mut *(parameter.0 as *mut Search) };
        if window.0 as isize != search.own && belongs_to_us(window) && is_usable_host(window) {
            if let Some((width, height)) = client_size(window) {
                let area = i64::from(width) * i64::from(height);
                if search.best.is_none_or(|(_, best)| area > best) {
                    search.best = Some((window, area));
                }
            }
        }
        BOOL(1)
    }

    /// The biggest visible top-level window of this process that is not `own`.
    fn largest_host_window(own: isize) -> Option<HWND> {
        let mut search = Search { own, best: None };
        // SAFETY: the state is a live `Search` for the duration of the call.
        unsafe {
            let _ = EnumWindows(Some(pick_host), LPARAM(&raw mut search as isize));
        }
        search.best.map(|(window, _)| window)
    }
}

// ============================================================================
// Other platforms: the window stays a normal top-level window
// ============================================================================

#[cfg(not(windows))]
mod platform {
    pub fn capture_host(_own: isize) {}
    pub fn forget_host() {}
    pub fn host() -> Option<isize> {
        None
    }
    pub fn host_minimized() -> bool {
        false
    }
    pub fn attach(_window: isize, _embedded: bool) -> bool {
        false
    }
    pub fn detach() {}
    pub fn is_embedded() -> bool {
        false
    }
    pub fn layout(_window: isize, _center: bool) {}
    pub fn pointer(_window: isize, _cursor: Option<(f64, f64)>, _pressed: Option<bool>) -> bool {
        false
    }
}

/// Remembers which window of this program the cleaner belongs in. Called from
/// the hotkey, while the program still has the focus. `own` is our own window
/// handle, which must never become the host.
pub fn capture_host(own: isize) {
    platform::capture_host(own);
}

/// Forgets the host window.
pub fn forget_host() {
    platform::forget_host();
}

/// The host window handle, if one was captured and still exists.
pub fn host() -> Option<isize> {
    platform::host()
}

/// Whether the host window is minimized.
pub fn host_minimized() -> bool {
    platform::host_minimized()
}

/// Makes the cleaner window a child of the host window (`embedded` is true), or
/// gives it back its own top-level status.
pub fn attach(window: isize, embedded: bool) -> bool {
    platform::attach(window, embedded)
}

/// Gives the window back its own top-level status.
pub fn detach() {
    platform::detach();
}

/// Whether the window currently is a child window.
pub fn is_embedded() -> bool {
    platform::is_embedded()
}

/// Repositions the embedded window: centered on `true`, at its current offset on
/// `false`.
pub fn layout(window: isize, center: bool) {
    platform::layout(window, center);
}

/// Mouse handling for the embedded window. Returns `true` when the event was
/// used for dragging.
pub fn pointer(window: isize, cursor: Option<(f64, f64)>, pressed: Option<bool>) -> bool {
    platform::pointer(window, cursor, pressed)
}