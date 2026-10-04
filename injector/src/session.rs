//! The cleaner window, running on a thread of its own.
//!
//! The window cannot run on the thread the library was loaded into: that thread
//! belongs to the host program and has its own event loop (winit allows exactly
//! one per process, and every toolkit assumes it owns the main thread). So the
//! window gets its own thread, its own winit event loop created with
//! `any_thread`, and its own tokio runtime, which `MyApp` needs to spawn the
//! cleaning jobs.
//!
//! The window is created hidden and shown by the hotkey. It stays alive while
//! hidden: recreating a winit event loop after the first one exited is not
//! supported, so hiding is the only way to keep the hotkey working.
//!
//! When the window is going to be embedded into the host program (see
//! [`crate::embed`]) it is created hidden even when it is about to be shown: it is
//! shown only after it has been parented, so it never appears as a second window
//! on screen or as a second entry in the taskbar.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use database::cleaner_database::CleanerDatabase;
#[cfg(windows)]
use database::registry_database::RegistryDatabase;
use database::structures::CustomCleaner;
use gui::app::MyApp;
use gui::title_bar::TITLE_BAR_HEIGHT;

use crate::config::{Action, Config};
use crate::embed;
use crate::log;
use crate::renderer;

/// Something the hotkey thread asks the window thread to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    /// Start a cleaning run over every category.
    QuickClean,
}

/// Everything the window thread and the hotkey thread share.
#[derive(Default)]
pub struct Shared {
    /// The egui context, published by the window thread. It can be used from any
    /// thread, which is how the hotkey thread shows the window without having to
    /// be scheduled by the event loop.
    context: Mutex<Option<egui::Context>>,
    /// Whether the window is currently on screen.
    visible: AtomicBool,
    /// Native window handle (an `HWND` on Windows), published once the window
    /// exists so the hotkey thread can raise it and the embedding can use it.
    window_handle: AtomicIsize,
    /// Requests handed to the window thread.
    requests: Mutex<VecDeque<Request>>,
    /// Set once the window thread has been started: a winit event loop can only be
    /// created once per process.
    started: AtomicBool,
    /// Set when the window was created hidden because it was going to be embedded,
    /// so that it is shown as soon as that has happened.
    hidden_for_embedding: AtomicBool,
}

/// The process-wide session state.
static SHARED: OnceLock<Arc<Shared>> = OnceLock::new();

/// Signalled when the window thread is gone, for a host that has nothing else to
/// do and would otherwise let `main` end the process.
static WINDOW_ENDED: OnceLock<(Mutex<bool>, std::sync::Condvar)> = OnceLock::new();

fn window_ended() -> &'static (Mutex<bool>, std::sync::Condvar) {
    WINDOW_ENDED.get_or_init(|| (Mutex::new(false), std::sync::Condvar::new()))
}

/// Blocks until the cleaner window is closed for good.
///
/// Only useful for a program of our own: returning from `main` ends the process
/// together with the window thread, so a host that starts the session from `main`
/// has to wait here instead.
pub fn wait() {
    let (lock, signal) = window_ended();
    let mut ended = lock.lock().expect("session mutex poisoned");
    while !*ended {
        ended = signal.wait(ended).expect("session mutex poisoned");
    }
}

/// Configuration the window thread needs, kept out of [`Shared`] because it is
/// written once before the first window is created.
static CONFIG: OnceLock<Config> = OnceLock::new();

/// Starts the session: registers the cleaners and opens the window thread.
///
/// Returns `false` when the session is already running, which is what loading the
/// library a second time into the same program sees.
pub fn start(config: Config) -> bool {
    let shared = SHARED.get_or_init(|| Arc::new(Shared::default())).clone();
    if CONFIG.set(config.clone()).is_err() {
        log::info("the cleaner session is already running in this process");
        return false;
    }

    // INFO: the built-in cleaners register themselves in a global registry that is
    // read once, before the window is built. Registering twice is harmless: the
    // registry ignores ids it already knows.
    cleaner::custom_cleaners::register_all();

    // INFO: the UI sounds need an audio device, and the host program may already
    // own the only one it has. Opening it from a library is not worth the risk, so
    // it stays off unless it is asked for.
    if config.audio {
        gui::sounds::init();
    }

    start_window_thread(&shared);
    true
}

/// The shared session state, for the hotkey thread.
pub fn shared() -> Option<Arc<Shared>> {
    SHARED.get().cloned()
}

/// Spawns the window thread. Later calls only make sure a request queued in the
/// meantime is not lost: the window is created on that thread, and only once.
fn start_window_thread(shared: &Arc<Shared>) {
    if shared.started.swap(true, Ordering::SeqCst) {
        return;
    }
    let shared = Arc::clone(shared);
    if std::thread::Builder::new()
        .name("cross-clean-window".to_string())
        // Cleaning spawns many short-lived futures; 2 MiB of stack keeps the
        // window thread from paying for guard pages on every one of them.
        .stack_size(2 * 1024 * 1024)
        .spawn(move || {
            if let Err(e) = run_window(&shared) {
                log::error(&format!("the cleaner window stopped: {e}"));
            }
        })
        .is_err()
    {
        log::error("cannot spawn the window thread");
        signal_window_ended();
    }
}

/// Shows the window, creating the window thread first if needed.
pub fn show() {
    let Some(shared) = shared() else {
        return;
    };
    start_window_thread(&shared);
    set_visible(&shared, true);
}

/// Hides the window. Does nothing while no window exists yet.
pub fn hide() {
    let Some(shared) = shared() else {
        return;
    };
    set_visible(&shared, false);
}

/// Shows the window if it is hidden, hides it if it is shown.
pub fn toggle() {
    let Some(shared) = shared() else {
        return;
    };
    let visible = shared.visible.load(Ordering::SeqCst);
    start_window_thread(&shared);
    set_visible(&shared, !visible);
}

/// Performs the action the configuration asks for.
pub fn activate(action: Action) {
    // The window the hotkey was pressed in is the foreground window of this
    // process, so this is the last moment where it is known which window the
    // cleaner should become a part of.
    if let Some(shared) = shared() {
        let own = shared.window_handle.load(Ordering::SeqCst);
        embed::capture_host(own);
    }
    match action {
        Action::Toggle => toggle(),
        Action::Show => show(),
        Action::Clean => {
            show();
            request(Request::QuickClean);
        }
    }
}

/// Queues a request for the window thread.
pub fn request(request: Request) {
    let Some(shared) = shared() else {
        return;
    };
    start_window_thread(&shared);
    if let Ok(mut queue) = shared.requests.lock() {
        queue.push_back(request);
    }
    // `request_repaint` wakes the event loop up, so a queued request is picked up
    // even when the window is hidden and nothing else is happening.
    with_context(&shared, |context| context.request_repaint());
}

/// Shows or hides the window from any thread.
fn set_visible(shared: &Arc<Shared>, visible: bool) {
    let changed = shared.visible.swap(visible, Ordering::SeqCst) != visible;
    let context = with_context(shared, Clone::clone);
    let Some(context) = context else {
        // The window thread has not created the window yet; it reads the flag when
        // it does.
        return;
    };
    if changed {
        context.send_viewport_cmd(egui::ViewportCommand::Visible(visible));
        if visible {
            context.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            context.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
    }
    if visible {
        let _ = apply_window_mode(shared);
        raise_window(shared);
    }
    context.request_repaint();
}

/// Puts the window where the configuration asks for: inside the host program's
/// window, or on top of it. Returns `true` when it is embedded.
fn apply_window_mode(shared: &Arc<Shared>) -> bool {
    let window = shared.window_handle.load(Ordering::SeqCst);
    if window == 0 {
        // No window yet; the first frame attaches it.
        return false;
    }
    let wanted = match CONFIG.get().map(|config| config.window) {
        Some(embed::Mode::Window) => false,
        Some(embed::Mode::Embedded) => true,
        // `Auto`: inside the host window when there is one.
        Some(embed::Mode::Auto) | None => embed::host().is_some(),
    };
    let embedded = embed::attach(window, wanted);
    if embedded {
        // Centered on every show, so the cleaner does not sit where it was left the
        // last time the program was used.
        embed::layout(window, true);
    }
    embedded
}

/// Runs `f` with the egui context of the running window, if there is one.
fn with_context<T>(shared: &Arc<Shared>, f: impl FnOnce(&egui::Context) -> T) -> Option<T> {
    let slot = shared.context.lock().ok()?;
    let context = slot.as_ref()?;
    Some(f(context))
}

/// Brings the window in front of everything else.
///
/// Windows refuses `SetForegroundWindow` for a process that did not receive the
/// last input event, which is exactly our situation: the hotkey was pressed while
/// the host program had the focus. A short topmost bump is the documented way
/// around that, and it is undone right away. An embedded window needs none of this:
/// it is a child of the window the user is looking at.
#[cfg(windows)]
fn raise_window(shared: &Arc<Shared>) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        SetForegroundWindow, SetWindowPos, HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOMOVE, SWP_NOSIZE,
        SWP_SHOWWINDOW,
    };

    let raw = shared.window_handle.load(Ordering::SeqCst);
    if raw == 0 || embed::is_embedded() {
        // No window yet, or a child window: it cannot be brought in front.
        return;
    }
    // SAFETY: `raw` is the `HWND` of our own window, published by the window
    // thread, and every call below only changes that window.
    unsafe {
        let hwnd = HWND(raw as *mut std::ffi::c_void);
        SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
        )
        .ok();
        SetWindowPos(
            hwnd,
            Some(HWND_NOTOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
        )
        .ok();
        let _ = SetForegroundWindow(hwnd).ok();
    }
}

/// Nothing to do: on the other window systems a mapped window is placed on top by
/// the server itself.
#[cfg(not(windows))]
fn raise_window(_shared: &Arc<Shared>) {}

/// `MyApp` plus the plumbing the loaded build needs: it publishes the egui context
/// and the native window handle, embeds the window where it belongs, runs the
/// queued requests, and forwards everything else to the real app.
struct SessionApp {
    inner: MyApp,
    shared: Arc<Shared>,
}

impl eframe::App for SessionApp {
    /// Runs once per frame, before the UI.
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        publish_window_handle(&self.shared, frame);

        // The window exists now, so it can be moved into the host program's
        // window. Created visible it would have been a window of its own for a
        // frame: a taskbar entry of its own, and a flash on screen.
        if !embed::is_embedded() {
            let embedded = apply_window_mode(&self.shared);
            if self.shared.visible.load(Ordering::SeqCst) {
                // A window that was created hidden for embedding is shown here, so
                // it is only ever seen inside the host program.
                let was_hidden = self.shared.hidden_for_embedding.swap(false, Ordering::SeqCst);
                if embedded || !was_hidden {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.request_repaint();
                }
            }
        }

        if let Ok(mut slot) = self.shared.context.lock() {
            *slot = Some(ctx.clone());
        }
        self.run_requests(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.inner.ui(ui, frame);
    }
}

impl SessionApp {
    /// Applies everything the hotkey thread queued since the last frame.
    fn run_requests(&mut self, ctx: &egui::Context) {
        let Ok(mut queue) = self.shared.requests.lock() else {
            return;
        };
        if queue.is_empty() {
            return;
        }
        let requests: Vec<Request> = queue.drain(..).collect();
        drop(queue);

        for request in requests {
            match request {
                Request::QuickClean => match self.inner.quick_clean_all() {
                    true => log::info("quick clean started from the hotkey"),
                    false => log::info("quick clean skipped: already running, or nothing to clean"),
                },
            }
            ctx.request_repaint();
        }
    }
}

/// Publishes the native window handle so the hotkey thread can raise the window
/// and the embedding can use it. Done once: the handle never changes.
fn publish_window_handle(shared: &Arc<Shared>, frame: &eframe::Frame) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    if shared.window_handle.load(Ordering::SeqCst) != 0 {
        return;
    }
    let Ok(handle) = frame.window_handle() else {
        return;
    };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return;
    };
    shared
        .window_handle
        .store(handle.hwnd.get() as isize, Ordering::SeqCst);
}

/// Builds the app, with the databases it cleans from.
fn build_app(config: &Config) -> MyApp {
    let custom_database: Arc<[CustomCleaner]> = if config.disable_custom {
        Arc::from(Vec::new())
    } else {
        Arc::from(database::custom_cleaners::get_custom_cleaners())
    };

    // INFO: a broken custom database is reported instead of silently falling back
    // to the built-in one, which would clean different paths than asked.
    let database = match &config.database_path {
        Some(path) => {
            let database = CleanerDatabase::from_file(path);
            if let Err(e) = database.for_each(|_| {}) {
                log::error(&format!("cannot read {}: {e}", path.display()));
            }
            database
        }
        None => CleanerDatabase::default_source(),
    };

    #[cfg(windows)]
    let app = match &config.registry_database_path {
        Some(path) => {
            let registry_database = RegistryDatabase::from_file(path);
            if let Err(e) = registry_database.for_each(|_| {}) {
                log::error(&format!("cannot read {}: {e}", path.display()));
            }
            MyApp::from_database(database, registry_database, custom_database)
        }
        None => MyApp::from_database(database, RegistryDatabase::default_source(), custom_database),
    };
    #[cfg(not(windows))]
    let app = MyApp::from_database(database, custom_database);

    // INFO: a loaded build never self-updates: there is no executable of its own to
    // replace, and `MyApp` only offers an in-app update once a platform updater
    // worker is registered.
    app
}

/// Runs the event loop of the cleaner window. Blocks for the lifetime of the
/// session.
fn run_window(shared: &Arc<Shared>) -> Result<(), String> {
    let config = CONFIG
        .get()
        .cloned()
        .ok_or_else(|| "the session was not configured".to_string())?;

    // INFO: recovering the display variables writes to the environment, which must
    // not race with another thread reading it, so this happens before the tokio
    // workers and the event loop exist.
    #[cfg(target_os = "linux")]
    let backend = {
        let display = gui::display::detect();
        display.apply();
        display.backend
    };
    // Off Linux there is no backend to choose.
    #[cfg(not(target_os = "linux"))]
    let backend = ();

    // INFO: `MyApp` spawns the cleaning jobs with `tokio::spawn`, so the runtime
    // has to be *entered* for the whole event loop rather than blocked on: its
    // workers live on their own threads while this one pumps messages.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| {
            signal_window_ended();
            format!("cannot create the tokio runtime: {e}")
        })?;
    let _runtime = runtime.enter();

    // A window that will end up inside the host program is created hidden and
    // shown by `SessionApp::logic` right after it has been parented. Created
    // visible it would be a window of its own first, with a taskbar entry and a
    // flash on screen.
    let wanted_visible = shared.visible.load(Ordering::SeqCst) || config.show_on_start;
    let embed_planned = config.window != embed::Mode::Window && embed::host().is_some();
    let start_visible = wanted_visible && !embed_planned;
    shared.visible.store(wanted_visible, Ordering::SeqCst);
    shared
        .hidden_for_embedding
        .store(wanted_visible && embed_planned, Ordering::SeqCst);

    let choice = renderer::choose(config.backend);
    let app = build_app(&config);
    let options = native_options(&app, start_visible, &choice);
    let window_title = app.window_title.clone();

    let event_loop = match build_event_loop(backend) {
        Ok(event_loop) => event_loop,
        Err(e) => {
            signal_window_ended();
            return Err(e);
        }
    };
    let session = SessionApp {
        inner: app,
        shared: Arc::clone(shared),
    };
    let exit_on_close = config.exit_on_close;

    let native = eframe::create_native(
        &window_title,
        options,
        Box::new(move |creation| {
            creation.egui_ctx.set_visuals(egui::Visuals::dark());
            Ok(Box::new(session))
        }),
        &event_loop,
    );

    log::info(&format!(
        "cleaner window ready on thread {:?} ({}, {})",
        std::thread::current().name().unwrap_or("unnamed"),
        choice.name,
        if start_visible { "visible" } else { "hidden" },
    ));

    // The event loop is consumed by `run_app`, and `EframeWinitApplication` owns the
    // app creator closure, so the handler takes ownership of it rather than
    // borrowing it: that keeps the struct down to one lifetime.
    let mut handler = CloseGuard {
        inner: native,
        shared: Arc::clone(shared),
        exit_on_close,
    };
    let result = event_loop.run_app(&mut handler);
    signal_window_ended();
    result.map_err(|e| format!("event loop failed: {e}"))
}

/// Tells a waiting host program that the window is gone for good.
fn signal_window_ended() {
    let (lock, signal) = window_ended();
    if let Ok(mut ended) = lock.lock() {
        *ended = true;
        signal.notify_all();
    }
}

/// Keeps the window alive when the user closes it.
///
/// winit ends the event loop when its last window is closed, and a second event
/// loop cannot be created afterwards. Closing therefore hides the window (the usual
/// behaviour of an injected overlay) unless the configuration asks for a real quit.
struct CloseGuard<'app> {
    inner: eframe::EframeWinitApplication<'app>,
    shared: Arc<Shared>,
    exit_on_close: bool,
}

impl winit::application::ApplicationHandler<eframe::UserEvent> for CloseGuard<'_> {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.inner.resumed(event_loop);
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        // An embedded window cannot use the title bar's own drag, so the pointer is
        // tracked here instead (see [`crate::embed`]).
        let window = self.shared.window_handle.load(Ordering::SeqCst);
        if window != 0 {
            let (cursor, pressed) = match &event {
                winit::event::WindowEvent::CursorMoved { position, .. } => {
                    (Some((position.x, position.y)), None)
                }
                winit::event::WindowEvent::MouseInput {
                    state: winit::event::ElementState::Pressed,
                    button: winit::event::MouseButton::Left,
                    ..
                } => (None, Some(true)),
                winit::event::WindowEvent::MouseInput {
                    state: winit::event::ElementState::Released,
                    button: winit::event::MouseButton::Left,
                    ..
                } => (None, Some(false)),
                _ => (None, None),
            };
            if embed::pointer(window, cursor, pressed) {
                return;
            }
        }

        if matches!(event, winit::event::WindowEvent::CloseRequested) && !self.exit_on_close {
            // Hide instead of closing, so the hotkey keeps working.
            self.shared.visible.store(false, Ordering::SeqCst);
            with_context(&self.shared, |context| {
                context.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                context.request_repaint();
            });
            log::info("window closed, hiding it (the hotkey still opens it)");
            return;
        }
        self.inner.window_event(event_loop, window_id, event);
    }

    fn about_to_wait(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.inner.about_to_wait(event_loop);
    }

    fn new_events(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        cause: winit::event::StartCause,
    ) {
        self.inner.new_events(event_loop, cause);
    }

    fn user_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        event: eframe::UserEvent,
    ) {
        self.inner.user_event(event_loop, event);
    }

    fn device_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        self.inner.device_event(event_loop, device_id, event);
    }

    fn suspended(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.inner.suspended(event_loop);
    }

    fn exiting(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.inner.exiting(event_loop);
    }

    fn memory_warning(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.inner.memory_warning(event_loop);
    }
}

/// Window backend to force when creating the event loop. There is no choice to make
/// off Linux, where winit has a single display server.
#[cfg(target_os = "linux")]
type BackendChoice = Option<gui::display::Backend>;
#[cfg(not(target_os = "linux"))]
type BackendChoice = ();

/// Creates the event loop of the window thread.
///
/// `any_thread` is the whole point: the window does not run on the main thread of
/// the process, which is where the host program keeps its own loop.
#[cfg_attr(not(target_os = "linux"), allow(unused_variables))]
fn build_event_loop(
    backend: BackendChoice,
) -> Result<winit::event_loop::EventLoop<eframe::UserEvent>, String> {
    #[allow(unused_mut)]
    let mut builder = winit::event_loop::EventLoop::<eframe::UserEvent>::with_user_event();

    #[cfg(windows)]
    {
        use winit::platform::windows::EventLoopBuilderExtWindows;
        builder.with_any_thread(true);
    }
    #[cfg(target_os = "linux")]
    {
        use winit::platform::wayland::EventLoopBuilderExtWayland;
        use winit::platform::x11::EventLoopBuilderExtX11;

        // Both extension traits define `with_any_thread`, so it is called on the
        // trait instead of the builder to keep the call unambiguous.
        let wayland = match backend {
            Some(gui::display::Backend::Wayland) => true,
            Some(gui::display::Backend::X11) => false,
            // No compositor socket was announced, so X11 is the only option left,
            // and winit has to be told before it would pick it itself.
            None => std::env::var_os("WAYLAND_DISPLAY").is_some(),
        };
        if wayland {
            builder.with_wayland();
            EventLoopBuilderExtWayland::with_any_thread(&mut builder, true);
        } else {
            builder.with_x11();
            EventLoopBuilderExtX11::with_any_thread(&mut builder, true);
        }
    }

    builder
        .build()
        .map_err(|e| format!("cannot create the event loop: {e}"))
}

/// Window size and chrome of the cleaner window.
///
/// The window is undecorated on purpose: the app draws its own title bar, which is
/// what makes the window draggable (`ViewportCommand::StartDrag`, and for an
/// embedded window the pointer handling in [`crate::embed`]).
fn native_options(
    app: &MyApp,
    start_visible: bool,
    choice: &renderer::Choice,
) -> eframe::NativeOptions {
    let icon = gui::icons::load_icon_from_ico_bytes(database::ICON_BYTES).ok();

    // INFO: the window is not resizable and its height depends on how many
    // categories the databases contain, so the size is computed the same way the
    // desktop build computes it: 20px per row of categories, a 445px frame for the
    // heading and the button, and the custom title bar.
    let rows = app.categories.len().div_ceil(gui::CATEGORY_COLUMNS);
    let size = egui::vec2(570.0, (rows * 20 + 445) as f32 + TITLE_BAR_HEIGHT);

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size(size)
        .with_min_inner_size(size)
        .with_resizable(false)
        .with_maximize_button(false)
        .with_decorations(false)
        .with_visible(start_visible)
        .with_app_id("cross-cleaner-injector");
    if let Some(icon) = icon {
        viewport = viewport.with_icon(icon);
    }

    let mut options = eframe::NativeOptions {
        viewport,
        renderer: choice.renderer,
        ..Default::default()
    };
    renderer::apply(&mut options, choice);
    options
}