//! Developer tool around the injectable cleaner.
//!
//! It does not inject anything: loading the library into a program is the job of the
//! program that is being cleaned (or of any external loader — this library does not
//! need one of its own).
//!
//! * `--standalone`: run the cleaner window in this process, with the same hotkey
//!   and the same configuration as the loaded library. The quickest way to try a
//!   build.
//! * `--library`: load the library into this process with `LoadLibraryW`, which is
//!   what an external loader does, and keep it running for 30 seconds.
//! * `--embed-test`: open a plain window, load the library into this process and
//!   check that the cleaner really became a *child* of that window. The only way to
//!   check the embedded mode without a second program.
//! * `--preload` (Linux): start a program with the cleaner in `LD_PRELOAD`, which is
//!   how a `.so` gets into another program.

use std::path::{Path, PathBuf};

use clap::Parser;
use cross_cleaner_injector::{Action, Backend, Config, Hotkey, WindowMode};

#[derive(Parser, Debug)]
#[command(
    name = "cross-clean-injector",
    version,
    about = "Injectable Cross Cleaner",
    long_about = None
)]
struct Args {
    /// Run the cleaner in this process instead of loading the library somewhere.
    #[arg(long)]
    standalone: bool,

    /// Library to load into this process with `LoadLibraryW` (Windows).
    #[arg(long, value_name = "path")]
    library: Option<PathBuf>,

    /// Check the embedded mode: open a window, load the library into this process and
    /// report whether the cleaner ended up inside that window.
    #[arg(long)]
    embed_test: bool,

    /// Shared library to preload into the launched program (Linux).
    #[arg(long, value_name = "path")]
    preload: Option<PathBuf>,

    /// Hotkey that opens the window, e.g. `delete`, `ctrl+alt+f12`.
    #[arg(long, value_name = "hotkey")]
    hotkey: Option<String>,

    /// What the hotkey does: `toggle` (default), `show` or `clean`.
    #[arg(long, value_name = "action")]
    action: Option<String>,

    /// Where the window appears: `auto` (default, inside the program the library was
    /// loaded into), `embedded` or `window`.
    #[arg(long, value_name = "mode")]
    window: Option<String>,

    /// Graphics API for the window: `auto` (default), `opengl`, `directx`, `vulkan`.
    #[arg(long, value_name = "backend")]
    renderer: Option<String>,

    /// Show the window right away instead of waiting for the hotkey.
    #[arg(long)]
    show_on_start: bool,

    /// Let the window's close button quit instead of hiding the window.
    #[arg(long)]
    exit_on_close: bool,

    /// Write the log to this file.
    #[arg(long, value_name = "path")]
    log: Option<PathBuf>,

    /// Program to launch with `--preload`, followed by its own arguments.
    #[arg(last = true)]
    command: Vec<String>,
}

fn main() {
    let args = Args::parse();

    if args.embed_test {
        std::process::exit(embed_test());
    }
    if let Some(path) = &args.library {
        std::process::exit(self_load(path, &args));
    }
    if let Some(library) = &args.preload {
        preload(library, &args.command);
        return;
    }
    standalone(&args);
}

/// Runs the cleaner in this process: the same code path the library takes once it is
/// loaded, minus the loading.
fn standalone(args: &Args) {
    let mut config = Config::default();
    apply_options(&mut config, args);
    // In a program of our own, closing the window should end the program.
    config.exit_on_close = true;

    if !cross_cleaner_injector::start_with(config) {
        eprintln!("the cleaner session is already running");
        return;
    }
    // Returning from `main` ends the process, and with it the window thread, so a
    // program of our own waits for the window to be closed instead.
    cross_cleaner_injector::session::wait();
}

/// Copies the command line options into the configuration.
fn apply_options(config: &mut Config, args: &Args) {
    if let Some(spec) = &args.hotkey {
        match Hotkey::parse(spec) {
            Ok(hotkey) => config.hotkey = hotkey,
            Err(e) => fail(e),
        }
    }
    if let Some(spec) = &args.action {
        config.action = match spec.trim().to_lowercase().as_str() {
            "toggle" => Action::Toggle,
            "show" => Action::Show,
            "clean" | "quick" => Action::Clean,
            other => fail(format!(
                "unknown action {other:?} (expected toggle, show or clean)"
            )),
        };
    }
    if let Some(spec) = &args.window {
        match spec.parse::<WindowMode>() {
            Ok(window) => config.window = window,
            Err(e) => fail(e),
        }
    }
    if let Some(spec) = &args.renderer {
        match spec.parse::<Backend>() {
            Ok(backend) => config.backend = backend,
            Err(e) => fail(e),
        }
    }
    if args.show_on_start {
        config.show_on_start = true;
    }
    if args.exit_on_close {
        config.exit_on_close = true;
    }
    if let Some(path) = &args.log {
        config.log = Some(path.clone());
    }
}

fn fail(message: String) -> ! {
    eprintln!("{message}");
    std::process::exit(2);
}

/// Loads the library into this process with `LoadLibraryW`, exactly like an external
/// loader does, and keeps the process alive while it runs.
#[cfg(windows)]
fn self_load(path: &Path, args: &Args) -> i32 {
    use std::os::windows::ffi::OsStrExt;

    unsafe extern "system" {
        #[link_name = "LoadLibraryW"]
        fn load_library_w(lpfilename: *const u16) -> isize;
    }

    let mut config = Config::default();
    apply_options(&mut config, args);
    println!("loading {}", path.display());
    set_startup_environment(&config);

    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    // SAFETY: `wide` is NUL-terminated and outlives the call.
    let handle = unsafe { load_library_w(wide.as_ptr()) };
    if handle == 0 {
        println!(
            "LoadLibraryW failed, error {}",
            std::io::Error::last_os_error()
        );
        return 1;
    }
    println!("loaded, handle 0x{handle:x}; press the hotkey (waiting 30s)");
    std::thread::sleep(std::time::Duration::from_secs(30));
    println!("done");
    0
}

#[cfg(not(windows))]
fn self_load(_path: &Path, _args: &Args) -> i32 {
    eprintln!("--library is only implemented on Windows");
    2
}

/// Puts the configuration into the environment, because that is what the loaded
/// library reads.
#[cfg(windows)]
fn set_startup_environment(config: &Config) {
    // SAFETY: set before the library is loaded and before any thread of ours
    // exists, so nothing else can be reading the environment right now.
    unsafe {
        if let Some(log) = &config.log {
            std::env::set_var("CROSS_CLEANER_LOG", log);
        }
        if config.show_on_start {
            std::env::set_var("CROSS_CLEANER_SHOW_ON_START", "1");
        }
        if config.exit_on_close {
            std::env::set_var("CROSS_CLEANER_EXIT_ON_CLOSE", "1");
        }
        std::env::set_var("CROSS_CLEANER_WINDOW", config.window.to_string());
        std::env::set_var("CROSS_CLEANER_RENDERER", config.backend.to_string());
        std::env::set_var("CROSS_CLEANER_ACTION", config.action.to_string());
        std::env::set_var("CROSS_CLEANER_HOTKEY", config.hotkey.to_string());
    }
}

#[cfg(not(windows))]
fn set_startup_environment(_config: &Config) {}

// ============================================================================
// Embedded mode self test
// ============================================================================

/// Opens a plain window, loads the library into this process and reports whether the
/// cleaner ended up as a child window of it.
///
/// Loading the library is a single `LoadLibraryW`, and it is all the test does
/// itself: the window, the hotkey and the embedding are the library's code.
#[cfg(windows)]
fn embed_test() -> i32 {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;

    use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, EnumChildWindows, GetClientRect,
        PeekMessageW, RegisterClassW, TranslateMessage, CS_HREDRAW, CS_VREDRAW, PM_REMOVE,
        WS_OVERLAPPEDWINDOW, WS_VISIBLE, WINDOW_EX_STYLE,
    };
    use windows::core::{BOOL, PCWSTR};

    unsafe extern "system" {
        #[link_name = "LoadLibraryW"]
        fn load_library_w(lpfilename: *const u16) -> isize;
    }

    /// Wide `EmbedHost`, the class name of the test window.
    const CLASS_NAME: PCWSTR = PCWSTR::from_raw(
        [
            b'E' as u16, b'm' as u16, b'b' as u16, b'e' as u16, b'd' as u16, b'H' as u16, b'o' as u16,
            b's' as u16, b't' as u16, 0,
        ]
        .as_ptr(),
    );
    /// Wide `Cross Cleaner embed test`, the title of the test window.
    const TITLE: PCWSTR = PCWSTR::from_raw(
        [
            b'C' as u16, b'r' as u16, b'o' as u16, b's' as u16, b's' as u16, b' ' as u16, b'C' as u16,
            b'l' as u16, b'e' as u16, b'a' as u16, b'n' as u16, b'e' as u16, b'r' as u16, b' ' as u16,
            b'e' as u16, b'm' as u16, b'b' as u16, b'e' as u16, b'd' as u16, 0,
        ]
        .as_ptr(),
    );

    unsafe extern "system" fn window_proc(
        window: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        // SAFETY: the host window of a test needs no real handling.
        unsafe { DefWindowProcW(window, message, wparam, lparam) }
    }

    /// Collects the handles the enumeration visits. The vector is reached through
    /// `LPARAM`, which is how `EnumChildWindows` passes data along.
    unsafe extern "system" fn collect_children(child: HWND, parameter: LPARAM) -> BOOL {
        // SAFETY: `parameter` is the `LPARAM` passed below, a live `Vec`.
        let children = unsafe { &mut *(parameter.0 as *mut Vec<isize>) };
        children.push(child.0 as isize);
        BOOL(1)
    }

    // SAFETY: every window created here belongs to this process and lives only as
    // long as the test does.
    unsafe {
        let class = windows::Win32::UI::WindowsAndMessaging::WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            hInstance: HINSTANCE(GetModuleHandleW(PCWSTR::null()).unwrap_or_default().0),
            lpszClassName: CLASS_NAME,
            ..Default::default()
        };
        if RegisterClassW(&class) == 0 {
            println!("RegisterClassW failed");
            return 1;
        }

        let host = match CreateWindowExW(
            WINDOW_EX_STYLE(0),
            CLASS_NAME,
            TITLE,
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            120,
            120,
            900,
            700,
            None,
            None,
            None,
            None,
        ) {
            Ok(host) => host,
            Err(e) => {
                println!("CreateWindowExW failed: {e}");
                return 1;
            }
        };
        println!("host window 0x{:x} created", host.0 as isize);

        let library = match default_library() {
            Ok(path) => path,
            Err(e) => {
                println!("{e}");
                return 1;
            }
        };
        let log = std::env::temp_dir().join("cross-clean-embed-test.log");
        let _ = std::fs::remove_file(&log);
        set_startup_environment(&Config {
            show_on_start: true,
            log: Some(log.clone()),
            window: WindowMode::Auto,
            ..Default::default()
        });

        let wide: Vec<u16> = library
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        println!("loading {}", library.display());
        if load_library_w(wide.as_ptr()) == 0 {
            println!("LoadLibraryW failed: {}", std::io::Error::last_os_error());
            return 1;
        }

        // Pump messages until the cleaner has become a child of the host window.
        // `PeekMessageW`, not `GetMessageW`: the loop has to keep running while no
        // message arrives.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        let mut children: Vec<isize> = Vec::new();
        while std::time::Instant::now() < deadline {
            let mut message = std::mem::zeroed();
            while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            children.clear();
            let _ = EnumChildWindows(
                Some(host),
                Some(collect_children),
                LPARAM(&raw mut children as isize),
            );
            if !children.is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }

        println!("child windows of the host window: {}", children.len());
        for child in &children {
            let mut rect = Default::default();
            let _ = GetClientRect(HWND(*child as *mut c_void), &mut rect);
            println!(
                "  child 0x{child:x}: {}x{}",
                rect.right - rect.left,
                rect.bottom - rect.top
            );
        }
        println!(
            "result: {}",
            if children.is_empty() {
                "the cleaner is NOT inside the host window"
            } else {
                "the cleaner IS a child window of the host window"
            }
        );
        if let Ok(text) = std::fs::read_to_string(&log) {
            println!("--- {} ---", log.display());
            print!("{text}");
        }
        if children.is_empty() { 3 } else { 0 }
    }
}

#[cfg(not(windows))]
fn embed_test() -> i32 {
    eprintln!("--embed-test is only implemented on Windows");
    2
}

// ============================================================================
// Linux: LD_PRELOAD launcher
// ============================================================================

/// Loads the shared library into a program this tool starts itself.
#[cfg(target_os = "linux")]
fn preload(library: &Path, command: &[String]) {
    if command.is_empty() {
        eprintln!("--preload needs a program: --preload <lib.so> -- <program> [args...]");
        std::process::exit(2);
    }
    let library = std::fs::canonicalize(library).unwrap_or_else(|_| library.to_path_buf());
    let env = preload_env(&library.display().to_string());

    use std::os::unix::process::CommandExt;
    // SAFETY: `execve_with_env` replaces this process with the program, so there is
    // nothing left to return to.
    let error =
        unsafe { std::process::Command::new(&command[0]).execve_with_env(&command[0], &command, &env) };
    eprintln!("cannot start {}: {error}", command[0]);
    std::process::exit(1);
}

/// The environment for the launched program: the current one plus `LD_PRELOAD`.
#[cfg(target_os = "linux")]
fn preload_env(library: &str) -> Vec<std::ffi::OsString> {
    let existing = std::env::var_os("LD_PRELOAD").unwrap_or_default();
    let value = if existing.is_empty() {
        std::ffi::OsString::from(library)
    } else {
        format!("{library}:{}", existing.to_string_lossy())
    };
    let mut env: Vec<std::ffi::OsString> = std::env::vars_os().collect();
    env.retain(|(key, _)| key != "LD_PRELOAD");
    env.push((std::ffi::OsString::from("LD_PRELOAD"), value));
    env
}

#[cfg(not(target_os = "linux"))]
fn preload(library: &Path, command: &[String]) {
    eprintln!(
        "LD_PRELOAD is a Linux mechanism (library: {}); on this platform load the library \
         with any DLL loader",
        library.display()
    );
    if !command.is_empty() {
        eprintln!("a program to launch was given, but there is nothing to launch it with");
    }
    std::process::exit(2);
}

/// Library of the current build, next to this executable or one level up (which is
/// where `cargo run` leaves it).
fn default_library() -> Result<PathBuf, String> {
    let name = if cfg!(windows) {
        "cross_cleaner_injector.dll"
    } else {
        "libcross_cleaner_injector.so"
    };
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(executable) = std::env::current_exe()
        && let Some(directory) = executable.parent()
    {
        candidates.push(directory.join(name));
        if let Some(parent) = directory.parent() {
            candidates.push(parent.join(name));
        }
    }
    candidates.push(PathBuf::from(name));
    candidates
        .into_iter()
        .find(|path| path.is_file())
        .ok_or_else(|| format!("{name} not found next to cross-clean-injector, build it first"))
}