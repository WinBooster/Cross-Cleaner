#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

// PERFORMANCE: Use mimalloc for blazing fast memory allocation
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

mod app;
mod categories;
mod cleaning;
mod icons;
mod notifications;
mod sounds;
mod taskbar;
mod title_bar;

use app::MyApp;
use clap::{ArgAction, Parser};
use database::get_version;
#[cfg(windows)]
use database::structures::CleanerDataRegistry;
use database::structures::{CleanerData, CustomCleaner};
use database::version::check_new_version;
use eframe::egui;
use icons::{ico_bytes_to_png_bytes, load_icon_from_bytes};
use std::sync::Arc;
use title_bar::TITLE_BAR_HEIGHT;

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

#[tokio::main]
async fn main() -> eframe::Result {
    let icon_bytes =
        ico_bytes_to_png_bytes(database::ICON_BYTES).expect("Failed to convert icon bytes to PNG");
    let icon = load_icon_from_bytes(&icon_bytes).expect("Failed to load icon");

    let args = Args::parse();

    // INFO: Initialize UI sounds (no-op without an audio device)
    sounds::init();

    // INFO: Register all built-in custom cleanings (functions in cleaner::custom_cleaners)
    cleaner::custom_cleaners::register_all();

    let custom_database: Arc<[CustomCleaner]> = if args.disable_custom {
        Arc::from(Vec::new())
    } else {
        Arc::from(database::custom_cleaners::get_custom_cleaners())
    };

    let database: Arc<[CleanerData]> = if let Some(db_path) = &args.database_path {
        match database::cleaner_database::get_database_from_file(db_path) {
            Ok(db) => db,
            Err(e) => {
                eprintln!("Failed to load database from file: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        database::cleaner_database::get_default_database()
    };

    #[cfg(windows)]
    let registry_database: Arc<[CleanerDataRegistry]> = {
        if let Some(db_path) = &args.registry_database_path {
            match database::registry_database::get_database_from_file(db_path) {
                Ok(db) => db,
                Err(e) => {
                    eprintln!("Failed to load database from file: {}", e);
                    std::process::exit(1);
                }
            }
        } else {
            database::registry_database::get_default_database()
        }
    };
    #[cfg(windows)]
    let app = MyApp::from_database(database, registry_database, custom_database);
    #[cfg(not(windows))]
    let app = MyApp::from_database(database, custom_database);
    let checkbox_count = app.categories.len();
    let rows = checkbox_count.div_ceil(3);
    // INFO: 20px for 1 checkbox, 45px for button, 32px for custom title bar
    let height = (rows * 20) + 45 + TITLE_BAR_HEIGHT as usize;

    // INFO: Check for a new version in the background
    let (update_sender, update_receiver) = std::sync::mpsc::channel();
    let mut app = app;
    app.update_receiver = Some(update_receiver);
    std::thread::spawn(move || {
        let _ = update_sender.send(check_new_version());
    });

    let size = egui::vec2(470.0, height as f32);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(size)
            .with_min_inner_size(size)
            .with_max_inner_size(size)
            .with_resizable(false)
            .with_maximize_button(false)
            .with_decorations(false)
            .with_icon(icon),
        ..Default::default()
    };

    eframe::run_native(
        &format!("Cross Cleaner GUI v{}", get_version()),
        options,
        Box::new(|_cc| {
            _cc.egui_ctx.set_visuals(egui::Visuals::dark());
            Ok(Box::new(app))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use categories::CategoryState;
    use database::structures::CleanerData;
    use std::collections::HashSet;

    #[test]
    fn test_load_icon_from_bytes() {
        let icon_data = database::ICON_BYTES;
        let result = load_icon_from_bytes(&icon_data[..]);

        assert!(result.is_ok(), "Icon should load successfully");
        let icon = result.unwrap();
        assert!(!icon.rgba.is_empty(), "Icon RGBA data should not be empty");
    }

    #[test]
    fn test_myapp_from_database() {
        let database: Vec<CleanerData> = vec![
            CleanerData {
                path: String::from("test/path1"),
                category: String::from("Cache"),
                program: String::from("TestApp1"),
                class: String::from("Application"),
                sub_category: String::from("Browser"),
                files_to_remove: vec![],
                directories_to_remove: vec![],
                remove_all_in_dir: false,
                remove_directory_after_clean: false,
                remove_directories: false,
                remove_files: false,
            },
            CleanerData {
                path: String::from("test/path2"),
                category: String::from("Logs"),
                program: String::from("TestApp2"),
                class: String::from("Application"),
                sub_category: String::from("System"),
                files_to_remove: vec![],
                directories_to_remove: vec![],
                remove_all_in_dir: false,
                remove_directory_after_clean: false,
                remove_directories: false,
                remove_files: false,
            },
        ];

        let registry_database: Vec<CleanerDataRegistry> = vec![CleanerDataRegistry {
            category: String::new(),
            program: String::new(),
            class: String::new(),
            sub_category: String::new(),
            remove_all_in_tree: false,
            remove_all_in_registry: false,
            path: String::new(),
            values_to_remove: vec![],
            keys_to_remove: vec![],
            remove_values: String::new(),
            remove_trees: String::new(),
        }];

        let app = MyApp::from_database(
            Arc::from(database.into_boxed_slice()),
            Arc::from(registry_database.into_boxed_slice()),
            Arc::from(Vec::new()),
        );

        assert_eq!(app.categories.len(), 2, "Should have 2 categories");
        assert!(
            app.task_handle.is_none(),
            "Task handle should be None initially"
        );
        assert!(!app.show_results, "Should not show results initially");
        assert_eq!(app.current_task, 0, "Current task should be 0");
        assert_eq!(app.total_tasks, 0, "Total tasks should be 0");
        assert!(
            !app.show_program_selection,
            "Should not show program selection initially"
        );
        assert!(
            app.program_checkboxes.is_empty(),
            "Program checkboxes should be empty"
        );
        assert!(
            app.excluded_programs.is_empty(),
            "Excluded programs should be empty"
        );
    }

    #[test]
    fn test_myapp_category_sorting() {
        let database: Vec<CleanerData> = vec![
            CleanerData {
                path: String::from("test1"),
                category: String::from("Documentation"),
                program: String::from("App1"),
                class: String::from("App"),
                sub_category: String::new(),
                files_to_remove: vec![],
                directories_to_remove: vec![],
                remove_all_in_dir: false,
                remove_directory_after_clean: false,
                remove_directories: false,
                remove_files: false,
            },
            CleanerData {
                path: String::from("test2"),
                category: String::from("Cache"),
                program: String::from("App2"),
                class: String::from("App"),
                sub_category: String::new(),
                files_to_remove: vec![],
                directories_to_remove: vec![],
                remove_all_in_dir: false,
                remove_directory_after_clean: false,
                remove_directories: false,
                remove_files: false,
            },
            CleanerData {
                path: String::from("test3"),
                category: String::from("Logs"),
                program: String::from("App3"),
                class: String::from("App"),
                sub_category: String::new(),
                files_to_remove: vec![],
                directories_to_remove: vec![],
                remove_all_in_dir: false,
                remove_directory_after_clean: false,
                remove_directories: false,
                remove_files: false,
            },
        ];

        let registry_database: Vec<CleanerDataRegistry> = vec![CleanerDataRegistry {
            category: String::new(),
            program: String::new(),
            class: String::new(),
            sub_category: String::new(),
            remove_all_in_tree: false,
            remove_all_in_registry: false,
            path: String::new(),
            values_to_remove: vec![],
            keys_to_remove: vec![],
            remove_values: String::new(),
            remove_trees: String::new(),
        }];

        let app = MyApp::from_database(
            Arc::from(database.into_boxed_slice()),
            Arc::from(registry_database.into_boxed_slice()),
            Arc::from(Vec::new()),
        );

        // Categories should be sorted with Cache first, then Logs, then Documentation
        assert_eq!(app.categories[0].name, "Cache", "First should be Cache");
        assert_eq!(app.categories[1].name, "Logs", "Second should be Logs");
        assert_eq!(
            app.categories[2].name, "Documentation",
            "Third should be Documentation"
        );
    }

    #[test]
    fn test_args_parsing() {
        // Test that Args structure can be created
        let args = Args {
            disable_custom: false,
            database_path: Some(String::from("test.json")),
            registry_database_path: Some(String::from("registry_test.json")),
        };

        assert_eq!(args.database_path, Some(String::from("test.json")));
        assert!(!args.disable_custom);
        assert_eq!(
            args.registry_database_path,
            Some(String::from("registry_test.json"))
        );
    }

    #[test]
    fn test_myapp_initial_state() {
        let database: Vec<CleanerData> = vec![];
        let registry_database: Vec<CleanerDataRegistry> = vec![];

        let app = MyApp::from_database(
            Arc::from(database.into_boxed_slice()),
            Arc::from(registry_database.into_boxed_slice()),
            Arc::from(Vec::new()),
        );

        assert!(
            app.progress_message.is_empty(),
            "Progress message should be empty"
        );
        assert!(app.search_query.is_empty(), "Search query should be empty");
        assert!(app.result_sender.is_some(), "Result sender should be Some");
        assert!(
            app.result_receiver.is_some(),
            "Result receiver should be Some"
        );
    }

    #[test]
    fn test_tristate_logic() {
        let mut cat = CategoryState {
            name: "Cache".to_string(),
            subs: vec!["A".to_string(), "B".to_string(), "C".to_string()],
            has_empty: false,
            selected: HashSet::new(),
        };
        assert!(cat.is_unchecked());
        assert!(!cat.is_checked());
        assert!(!cat.is_indeterminate());
        cat.selected.insert("A".to_string());
        assert!(cat.is_indeterminate());
        assert!(!cat.is_checked());
        cat.selected.insert("B".to_string());
        cat.selected.insert("C".to_string());
        assert!(cat.is_checked());
        assert!(!cat.is_indeterminate());
        cat.selected.clear();
        assert!(cat.is_unchecked());
    }

    #[test]
    fn test_subcategory_selection() {
        let database: Vec<CleanerData> = vec![
            CleanerData {
                path: String::from("p1"),
                category: String::from("Cache"),
                program: String::from("App1"),
                class: String::from("Browser"),
                sub_category: String::from("Browser"),
                files_to_remove: vec![],
                directories_to_remove: vec![],
                remove_all_in_dir: false,
                remove_directory_after_clean: false,
                remove_directories: false,
                remove_files: false,
            },
            CleanerData {
                path: String::from("p2"),
                category: String::from("Cache"),
                program: String::from("App2"),
                class: String::from("Game"),
                sub_category: String::from("Game"),
                files_to_remove: vec![],
                directories_to_remove: vec![],
                remove_all_in_dir: false,
                remove_directory_after_clean: false,
                remove_directories: false,
                remove_files: false,
            },
        ];
        let registry_database: Vec<CleanerDataRegistry> = vec![];
        let app = MyApp::from_database(
            Arc::from(database.into_boxed_slice()),
            Arc::from(registry_database.into_boxed_slice()),
            Arc::from(Vec::new()),
        );
        assert_eq!(app.categories.len(), 1);
        assert_eq!(app.categories[0].subs.len(), 2);
        assert!(app.categories[0].subs.contains(&"Browser".to_string()));
        assert!(app.categories[0].subs.contains(&"Game".to_string()));
    }
}
