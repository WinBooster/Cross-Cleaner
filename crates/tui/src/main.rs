//! Cross Cleaner, terminal edition.
//!
//! Mirrors the window frontend: the same categories, the same program list, the
//! same cleaning run — rendered with ratatui instead of egui.
//!
//! ```text
//! cargo run -p tui
//! cargo run -p tui -- --database-path=custom_database.json
//! ```

use std::io::{self, Stdout};
use std::sync::Arc;
use std::time::Duration;

use appcore::app::AppState;
use clap::{ArgAction, Parser};
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use database::cleaner_database::CleanerDatabase;
#[cfg(windows)]
use database::registry_database::RegistryDatabase;
use database::structures::CustomCleaner;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tui::TuiApp;

/// How long the loop waits for a key before redrawing anyway. Cleaning pushes
/// progress through a channel, so the app must repaint on its own while a run
/// is in flight.
const TICK: Duration = Duration::from_millis(100);

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Disable all built-in custom cleanings.
    /// Example: --disable-custom=true
    #[arg(long, value_name = "bool", default_value_t = false, action = ArgAction::Set)]
    disable_custom: bool,

    /// Specify a custom database file path.
    /// Example: --database-path=custom_database.json
    #[arg(long, value_name = "path")]
    database_path: Option<String>,

    /// Specify a custom registry database file path.
    /// Example: --registry-database-path=custom_database.json
    #[cfg(windows)]
    #[arg(long, value_name = "registry_path")]
    registry_database_path: Option<String>,
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    // INFO: the custom cleaners are registered in a global registry, so this
    // has to happen before the database is assembled.
    cleaner::custom_cleaners::register_all();

    let custom_database: Arc<[CustomCleaner]> = if args.disable_custom {
        Arc::from(Vec::new())
    } else {
        Arc::from(database::custom_cleaners::get_custom_cleaners())
    };

    // INFO: validate a user-supplied database early; the entries themselves are
    // streamed on demand (see CleanerDatabase::for_each).
    let database = match &args.database_path {
        Some(path) => {
            let database = CleanerDatabase::from_file(path);
            if let Err(e) = database.for_each(|_| {}) {
                eprintln!("Failed to load database from file: {e}");
                std::process::exit(1);
            }
            database
        }
        None => CleanerDatabase::default_source(),
    };

    #[cfg(windows)]
    let state = match &args.registry_database_path {
        Some(path) => {
            let registry = RegistryDatabase::from_file(path);
            if let Err(e) = registry.for_each(|_| {}) {
                eprintln!("Failed to load database from file: {e}");
                std::process::exit(1);
            }
            AppState::from_database(database, registry, custom_database)
        }
        None => AppState::from_database(
            database,
            RegistryDatabase::default_source(),
            custom_database,
        ),
    };

    #[cfg(not(windows))]
    let state = AppState::from_database(database, custom_database);

    // INFO: `AppState` spawns cleaning jobs with `tokio::spawn`, so the runtime
    // has to outlive the UI loop. `block_on` keeps it alive for the whole run.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    let mut app = TuiApp::new(state);
    // Registers the worker that can replace this executable with the release
    // binary; without it the dialog only offers the release page.
    app.start_self_update();
    runtime.block_on(run(app))
}

/// Owns the terminal setup, so a panic or an early error still restores it.
struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let terminal = Terminal::new(CrosstermBackend::new(stdout))?;
        Ok(Self { terminal })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        // Best effort: the process is about to exit, so failures here only
        // affect the appearance of the shell prompt afterwards.
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

async fn run(mut app: TuiApp) -> io::Result<()> {
    let mut guard = TerminalGuard::enter()?;
    let result = event_loop(&mut guard.terminal, &mut app);
    drop(guard);
    result
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut TuiApp,
) -> io::Result<()> {
    loop {
        // Drain the cleaning channels and the version check before drawing, so
        // the frame already shows the newest state.
        app.tick();
        terminal.draw(|frame| app.render(frame))?;

        if event::poll(TICK)? {
            match event::read()? {
                // Key repeats are already filtered by crossterm on Windows;
                // filtering the kind keeps a kitty-protocol release from
                // firing a binding twice.
                Event::Key(key) if key.kind != KeyEventKind::Release => {
                    app.on_event(Event::Key(key))
                }
                other => app.on_event(other),
            }
            if app.should_quit {
                return Ok(());
            }
        }
    }
}
