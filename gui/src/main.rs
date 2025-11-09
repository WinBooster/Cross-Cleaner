#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

// PERFORMANCE: Use mimalloc for blazing fast memory allocation
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use clap::Parser;
use cleaner::clear_data;
#[cfg(windows)]
use database::registry_database;
#[cfg(windows)]
use database::structures::{CleanerData, Cleared};
use database::utils::get_file_size_string;
use database::{get_icon, get_version};
use eframe::egui;
use egui::IconData;
use futures::stream::{FuturesUnordered, StreamExt};
use image::ImageReader;
use notify_rust::Notification;
use std::cell::RefCell;
use std::collections::HashSet;
use std::io::Write;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tempfile::NamedTempFile;
use tokio::sync::mpsc;
use tokio::task;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Specify a custom database file path.
    /// Example: --database-path=custom_database.json
    #[arg(long, value_name = "path")]
    database_path: Option<String>,
}

#[tokio::main]
async fn main() -> eframe::Result {
    let icon_bytes = get_icon();
    let icon = load_icon_from_bytes(icon_bytes).expect("Failed to load icon");

    let args = Args::parse();

    let database: Vec<CleanerData> = if let Some(db_path) = &args.database_path {
        match database::cleaner_database::get_database_from_file(db_path) {
            Ok(db) => db,
            Err(e) => {
                eprintln!("Failed to load database from file: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        database::cleaner_database::get_default_database().clone()
    };

    let app = MyApp::from_database(Arc::from(database));
    let checkbox_count = app.checked_boxes.len();
    let rows = checkbox_count.div_ceil(3);
    // INFO: 20px for 1 checkbox, 45px for button
    let height = (rows * 20) + 45;

    let size = egui::vec2(450.0, height as f32);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(size)
            .with_min_inner_size(size)
            .with_max_inner_size(size)
            .with_resizable(false)
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

fn load_icon_from_bytes(bytes: &[u8]) -> Result<Arc<IconData>, image::ImageError> {
    let img = ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()?
        .decode()?;

    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();

    Ok(Arc::new(IconData {
        rgba: rgba.into_raw(),
        width,
        height,
    }))
}

async fn work(
    categories: Vec<String>,
    progress_sender: mpsc::Sender<String>,
    database: &[CleanerData],
    excluded_programs: HashSet<String>,
) -> (u64, u64, u64, Vec<Cleared>) {
    let mut current_task = 0;

    // Use atomics for lock-free concurrent counting - BLAZING FAST!
    let bytes_cleared = AtomicU64::new(0);
    let removed_files = AtomicU64::new(0);
    let removed_directories = AtomicU64::new(0);

    // Pre-allocate with exact capacity
    let mut cleared_programs = Vec::<Cleared>::with_capacity(database.len());

    let mut tasks = Vec::with_capacity(database.len() + 1);

    // Pre-convert to HashSet for O(1) lookups instead of O(n) contains
    let categories_set: HashSet<String> = categories.into_iter().collect();

    // INFO: Clear LastActivity from Registry
    // WARN: Windows only
    #[cfg(windows)]
    {
        let registry_database = registry_database::get_default_database();
        for data in registry_database.iter() {
            if categories_set.contains(&data.category) && !excluded_programs.contains(&data.program)
            {
                let data = Arc::new(data.clone());
                let task =
                    task::spawn(async move { registry_database::clear_last_activity(&data) });
                tasks.push(task);
            }
        }
    }

    // Filter and spawn tasks - avoid cloning in filter closure
    for data in database.iter() {
        if categories_set.contains(&data.category) && !excluded_programs.contains(&data.program) {
            let data = Arc::new(data.clone());
            let task = task::spawn(async move { clear_data(&data) });
            tasks.push(task);
        }
    }

    let total_tasks = tasks.len();
    let _ = progress_sender
        .send(format!("PROGRESS:0:{}", total_tasks))
        .await;

    // Use FuturesUnordered to process tasks as they complete (smoother progress)
    let mut futures = tasks.into_iter().collect::<FuturesUnordered<_>>();

    // Batch progress updates every N items for better performance
    const PROGRESS_BATCH_SIZE: usize = 3;
    let mut pending_paths = Vec::with_capacity(PROGRESS_BATCH_SIZE);

    while let Some(task_result) = futures.next().await {
        match task_result {
            Ok(result) => {
                current_task += 1;

                if result.working {
                    // Atomic operations - no locks, blazing fast!
                    bytes_cleared.fetch_add(result.bytes, Ordering::Relaxed);
                    removed_files.fetch_add(result.files, Ordering::Relaxed);
                    removed_directories.fetch_add(result.folders, Ordering::Relaxed);

                    // Use binary search hint or direct lookup for large datasets
                    if let Some(cleared) = cleared_programs
                        .iter_mut()
                        .find(|c| c.program == result.program)
                    {
                        cleared.removed_bytes += result.bytes as u64;
                        cleared.removed_files += result.files as u64;
                        cleared.removed_directories += result.folders as u64;
                        if !cleared.affected_categories.contains(&result.category) {
                            cleared.affected_categories.push(result.category);
                        }
                    } else {
                        cleared_programs.push(Cleared {
                            program: result.program,
                            removed_bytes: result.bytes as u64,
                            removed_files: result.files as u64,
                            removed_directories: result.folders as u64,
                            affected_categories: vec![result.category],
                        });
                    }
                }

                // Batch progress updates
                pending_paths.push(result.path);
                if pending_paths.len() >= PROGRESS_BATCH_SIZE || futures.is_empty() {
                    // Send latest path
                    if let Some(last_path) = pending_paths.last() {
                        let _ = progress_sender.send(last_path.clone()).await;
                    }
                    // Send progress bar update
                    let _ = progress_sender
                        .send(format!("PROGRESS:{}:{}", current_task, total_tasks))
                        .await;
                    pending_paths.clear();
                }
            }
            Err(_) => {
                eprintln!("Error waiting for task completion");
                current_task += 1;
                if pending_paths.len() >= PROGRESS_BATCH_SIZE || futures.is_empty() {
                    let _ = progress_sender
                        .send(format!("PROGRESS:{}:{}", current_task, total_tasks))
                        .await;
                    pending_paths.clear();
                }
            }
        }
    }

    // Load atomic values once at the end
    let bytes_cleared_val = bytes_cleared.load(Ordering::Relaxed);
    let removed_files_val = removed_files.load(Ordering::Relaxed);
    let removed_directories_val = removed_directories.load(Ordering::Relaxed);

    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(get_icon()).unwrap();
    let icon_path = temp_file.path().to_str().unwrap();

    let notification_body = format!(
        "Removed: {}\nFiles: {}\nDirs: {}",
        get_file_size_string(bytes_cleared_val),
        removed_files_val,
        removed_directories_val
    );

    let mut notification = Notification::new();
    let notification = notification
        .summary("Cross Cleaner GUI")
        .body(&notification_body)
        .icon(icon_path);

    let notification_result = notification.show();

    temp_file.close().unwrap();
    if let Err(e) = notification_result {
        eprintln!("Failed to show notification: {:?}", e);
    }

    (
        bytes_cleared_val,
        removed_files_val,
        removed_directories_val,
        cleared_programs,
    )
}

struct MyApp {
    pub checked_boxes: Vec<(Rc<RefCell<bool>>, String)>,
    pub task_handle: Option<tokio::task::JoinHandle<(u64, u64, u64, Vec<Cleared>)>>,
    pub progress_message: String,
    pub progress_receiver: Option<mpsc::Receiver<String>>,
    pub cleared_data: Option<(u64, u64, u64, Vec<Cleared>)>,
    pub show_results: bool,
    pub current_task: usize,
    pub total_tasks: usize,

    pub show_program_selection: bool,
    pub program_checkboxes: Vec<(Rc<RefCell<bool>>, String)>,
    pub search_query: String,
    pub search_query_visible: String,
    pub excluded_programs: HashSet<String>,
    pub results_window_resized: bool,

    pub result_sender: Option<mpsc::Sender<(u64, u64, u64, Vec<Cleared>)>>,
    pub result_receiver: Option<mpsc::Receiver<(u64, u64, u64, Vec<Cleared>)>>,
    pub database: Arc<[CleanerData]>,
}

impl MyApp {
    pub(crate) fn from_database(database: Arc<[CleanerData]>) -> Self {
        let mut options: Vec<String> = Vec::with_capacity(database.len());
        for data in database.iter() {
            if !options.contains(&data.category) {
                options.push(data.category.clone());
            }
        }

        #[cfg(windows)]
        for data in registry_database::get_default_database().iter() {
            if !options.contains(&data.category) {
                options.push(data.category.clone());
            }
        }

        options.sort_by(|a, b| {
            let priority = |s: &str| match s {
                "Cache" => 0,
                "Logs" => 1,
                "Crashes" => 2,
                "Documentation" => 3,
                "Backups" => 4,
                "LastActivity" => 5,
                _ => 6,
            };

            let a_prio = priority(a);
            let b_prio = priority(b);

            if a_prio == b_prio {
                a.cmp(b)
            } else {
                a_prio.cmp(&b_prio)
            }
        });

        let mut checked_boxes = vec![];
        for option in options {
            checked_boxes.push((Rc::new(RefCell::new(false)), option));
        }

        let (result_sender, result_receiver) = mpsc::channel(1);

        Self {
            database,
            checked_boxes,
            task_handle: None,
            progress_message: String::new(),
            progress_receiver: None,
            cleared_data: None,
            show_results: false,
            current_task: 0,
            total_tasks: 0,

            show_program_selection: false,
            program_checkboxes: vec![],
            search_query: String::new(),
            search_query_visible: String::new(),
            excluded_programs: HashSet::new(),
            results_window_resized: false,

            result_sender: Some(result_sender),
            result_receiver: Some(result_receiver),
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(receiver) = &mut self.progress_receiver {
            if let Ok(message) = receiver.try_recv() {
                if message.starts_with("PROGRESS:") {
                    let parts: Vec<&str> = message.split(':').collect();
                    if parts.len() == 3 {
                        self.current_task = parts[1].parse().unwrap_or(0);
                        self.total_tasks = parts[2].parse().unwrap_or(0);
                    }
                } else {
                    self.progress_message = message;
                }
                ctx.request_repaint();
            }
        }

        if let Some(receiver) = &mut self.result_receiver {
            if let Ok(result) = receiver.try_recv() {
                self.cleared_data = Some(result);
                self.show_results = true;
                self.results_window_resized = false; // Reset flag for new results
                ctx.request_repaint();
            }
        }

        if let Some(handle) = &mut self.task_handle {
            if handle.is_finished() {
                let handle = self.task_handle.take().unwrap();
                let sender = self.result_sender.take().unwrap();
                tokio::spawn(async move {
                    match handle.await {
                        Ok(result) => {
                            let _ = sender.send(result).await;
                        }
                        Err(e) => eprintln!("Task failed: {:?}", e),
                    }
                });
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.task_handle.is_some() {
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::Vec2::new(
                    450.0, 120.0,
                )));
                ui.vertical_centered(|ui| {
                    ui.heading("Cleaning in progress...");
                    ui.add_space(10.0);

                    if self.total_tasks > 0 {
                        let progress = self.current_task as f32 / self.total_tasks as f32;
                        ui.add(
                            egui::ProgressBar::new(progress)
                                .show_percentage()
                                .animate(true),
                        );
                        ui.label(format!("Task {}/{}", self.current_task, self.total_tasks));
                    } else {
                        ui.spinner();
                    }

                    ui.add_space(10.0);
                    ui.label(&self.progress_message);
                });
                return;
            }

            if self.show_results {
                if let Some((bytes, files, dirs, cleared)) = &self.cleared_data {
                    ui.vertical_centered(|ui| {
                        ui.heading("Cleaning Results");
                        ui.heading(format!(
                            "Size: {}, Files: {}, Dirs: {}",
                            get_file_size_string(*bytes),
                            files,
                            dirs
                        ));
                    });
                    ui.separator();

                    // Фиксированные размеры для колонок
                    let column_widths = [150.0, 80.0, 80.0, 170.0];
                    let total_width = column_widths.iter().sum::<f32>() + 120.0;
                    let total_height = 500.0;

                    // Resize window only once when results are first shown
                    if !self.results_window_resized {
                        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::Vec2::new(
                            total_width,
                            total_height,
                        )));
                        self.results_window_resized = true;
                    }

                    // Общий контейнер для таблицы
                    ui.vertical(|ui| {
                        // Заголовки таблицы
                        ui.horizontal(|ui| {
                            ui.style_mut().spacing.item_spacing = egui::vec2(0.0, 0.0);

                            // Колонка Program
                            ui.add_sized(
                                egui::vec2(column_widths[0], 20.0),
                                egui::Label::new(egui::RichText::new("Program").heading()),
                            )
                            .on_hover_text("Program name");

                            // Колонка Size
                            ui.add_sized(
                                egui::vec2(column_widths[1], 20.0),
                                egui::Label::new(egui::RichText::new("Size").heading()),
                            )
                            .on_hover_text("Deleted data size");

                            // Колонка Files
                            ui.add_sized(
                                egui::vec2(column_widths[2], 20.0),
                                egui::Label::new(egui::RichText::new("Files").heading()),
                            )
                            .on_hover_text("Number of files");

                            // Колонка Dirs
                            ui.add_sized(
                                egui::vec2(column_widths[2], 20.0),
                                egui::Label::new(egui::RichText::new("Dirs").heading()),
                            )
                            .on_hover_text("Number of folders");

                            // Колонка Categories
                            ui.add_sized(
                                egui::vec2(column_widths[3], 20.0),
                                egui::Label::new(egui::RichText::new("Categories").heading()),
                            )
                            .on_hover_text("Data categories");
                        });
                        ui.separator();

                        // Прокручиваемое содержимое таблицы
                        egui::ScrollArea::vertical()
                            .max_height(total_height)
                            .show(ui, |ui| {
                                for cleared in cleared {
                                    ui.horizontal(|ui| {
                                        ui.style_mut().spacing.item_spacing = egui::vec2(0.0, 0.0);

                                        // Колонка Program
                                        ui.add_sized(
                                            egui::vec2(column_widths[0], 20.0),
                                            egui::Label::new(&cleared.program).truncate(),
                                        );

                                        // Колонка Size
                                        ui.add_sized(
                                            egui::vec2(column_widths[1], 20.0),
                                            egui::Label::new(get_file_size_string(
                                                cleared.removed_bytes,
                                            ))
                                            .truncate(),
                                        );

                                        // Колонка Files
                                        ui.add_sized(
                                            egui::vec2(column_widths[2], 20.0),
                                            egui::Label::new(cleared.removed_files.to_string())
                                                .truncate(),
                                        );

                                        // Колонка Dirs
                                        ui.add_sized(
                                            egui::vec2(column_widths[2], 20.0),
                                            egui::Label::new(
                                                cleared.removed_directories.to_string(),
                                            )
                                            .truncate(),
                                        );

                                        // Колонка Categories
                                        ui.add_sized(
                                            egui::vec2(column_widths[3], 20.0),
                                            egui::Label::new(
                                                cleared.affected_categories.join(", "),
                                            )
                                            .wrap(),
                                        );
                                    });
                                    ui.separator();
                                }
                            });
                    });
                    return;
                }
            }

            if self.show_program_selection {
                // Dynamic window sizing based on number of programs
                let num_programs = self.program_checkboxes.len();
                let rows = (num_programs + 1) / 2; // 2 columns
                let row_height = 20.0;
                let base_height = 120.0; // Heading, search, buttons, separators
                let min_scroll_height = 20.0;
                let max_scroll_height = 400.0;

                let content_height = rows as f32 * row_height;
                let scroll_height = content_height.min(max_scroll_height).max(min_scroll_height);
                let window_height = base_height + scroll_height;

                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::Vec2::new(
                    500.0,
                    window_height,
                )));

                ui.vertical_centered(|ui| {
                    ui.heading("Select Programs to Clean");
                });
                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("Search:");
                    let available_width = ui.available_width();
                    let search_response = ui.add_sized(
                        [available_width, 20.0],
                        egui::TextEdit::singleline(&mut self.search_query_visible),
                    );
                    if search_response.changed() {
                        self.search_query = self.search_query_visible.to_lowercase();
                    }
                });

                ui.separator();

                egui::ScrollArea::vertical()
                    .max_height(scroll_height)
                    .show(ui, |ui| {
                        ui.columns(2, |columns| {
                            let mut col_index = 0;
                            for (checkbox, program) in self.program_checkboxes.iter() {
                                if self.search_query.is_empty()
                                    || program.to_lowercase().contains(&self.search_query)
                                {
                                    let mut value = checkbox.borrow_mut();
                                    columns[col_index % 2].checkbox(&mut *value, program);
                                    col_index += 1;
                                }
                            }
                        });
                    });

                ui.separator();

                let available_width = ui.available_width();
                ui.horizontal(|ui| {
                    if ui
                        .add_sized(
                            [available_width / 2.0 - 5.0, 25.0],
                            egui::Button::new("Back"),
                        )
                        .clicked()
                    {
                        self.show_program_selection = false;
                        // Window will resize dynamically in the next frame based on category count
                    }

                    if ui
                        .add_sized(
                            [available_width / 2.0 - 5.0, 25.0],
                            egui::Button::new("Start Cleaning"),
                        )
                        .clicked()
                    {
                        let selected_categories: Vec<String> = self
                            .checked_boxes
                            .iter()
                            .filter(|(checkbox, _)| *checkbox.borrow())
                            .map(|(_, label)| label.clone())
                            .collect();

                        self.excluded_programs.clear();
                        for (checkbox, program) in &self.program_checkboxes {
                            if !*checkbox.borrow() {
                                self.excluded_programs.insert(program.clone());
                            }
                        }

                        let (progress_sender, progress_receiver) = mpsc::channel(32);
                        self.progress_receiver = Some(progress_receiver);
                        self.current_task = 0;
                        self.total_tasks = 0;
                        self.results_window_resized = false;

                        let database = Arc::clone(&self.database);
                        let excluded_programs = self.excluded_programs.clone();
                        let handle = tokio::spawn(async move {
                            work(
                                selected_categories,
                                progress_sender,
                                &database,
                                excluded_programs,
                            )
                            .await
                        });
                        self.task_handle = Some(handle);

                        self.show_program_selection = false;
                        // Window will resize dynamically in the next frame based on category count
                        for (checkbox, _) in &self.checked_boxes {
                            *checkbox.borrow_mut() = false;
                        }
                    }
                });
            } else {
                // Calculate dynamic window height based on number of categories
                let num_categories = self.checked_boxes.len();
                let rows = (num_categories + 2) / 3; // Round up division by 3 (3 columns)
                let row_height = 20.0; // Approximate height per row
                let base_height = 45.0; // Space for heading, margins, and button
                let dynamic_height = base_height + (rows as f32 * row_height);
                let window_height = dynamic_height.max(20.0).min(500.0); // Clamp between 200 and 500

                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::Vec2::new(
                    450.0,
                    window_height,
                )));

                ui.columns(3, |columns| {
                    for (i, (checkbox, label)) in self.checked_boxes.iter().enumerate() {
                        let column_index = i % 3;
                        let mut value = checkbox.borrow_mut();
                        columns[column_index].checkbox(&mut *value, label);
                    }
                });

                let available_width = ui.available_width();

                if ui
                    .add_sized([available_width, 25.0], egui::Button::new("Next"))
                    .clicked()
                {
                    let selected_categories: Vec<String> = self
                        .checked_boxes
                        .iter()
                        .filter(|(checkbox, _)| *checkbox.borrow())
                        .map(|(_, label)| label.clone())
                        .collect();

                    if !selected_categories.is_empty() {
                        let categories_set: HashSet<String> =
                            selected_categories.into_iter().collect();

                        let mut programs: Vec<String> = Vec::new();
                        for data in self.database.iter() {
                            if categories_set.contains(&data.category)
                                && !programs.contains(&data.program)
                            {
                                programs.push(data.program.clone());
                            }
                        }
                        #[cfg(windows)]
                        {
                            for data in registry_database::get_default_database() {
                                if categories_set.contains(&data.category)
                                    && !programs.contains(&data.program)
                                {
                                    programs.push(data.program.clone());
                                }
                            }
                        }
                        programs.sort();

                        self.program_checkboxes.clear();
                        for program in programs {
                            self.program_checkboxes
                                .push((Rc::new(RefCell::new(true)), program));
                        }

                        self.search_query.clear();
                        self.show_program_selection = true;
                        // Resize window will happen in the next frame when show_program_selection is true
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use database::structures::CleanerData;

    #[test]
    fn test_load_icon_from_bytes() {
        let icon_data = get_icon();
        let result = load_icon_from_bytes(icon_data);

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
                files_to_remove: vec![],
                directories_to_remove: vec![],
                remove_all_in_dir: false,
                remove_directory_after_clean: false,
                remove_directories: false,
                remove_files: false,
            },
        ];

        let app = MyApp::from_database(Arc::from(database.into_boxed_slice()));

        assert_eq!(app.checked_boxes.len(), 2, "Should have 2 categories");
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
                files_to_remove: vec![],
                directories_to_remove: vec![],
                remove_all_in_dir: false,
                remove_directory_after_clean: false,
                remove_directories: false,
                remove_files: false,
            },
        ];

        let app = MyApp::from_database(Arc::from(database.into_boxed_slice()));

        // Categories should be sorted with Cache first, then Logs, then Documentation
        assert_eq!(app.checked_boxes[0].1, "Cache", "First should be Cache");
        assert_eq!(app.checked_boxes[1].1, "Logs", "Second should be Logs");
        assert_eq!(
            app.checked_boxes[2].1, "Documentation",
            "Third should be Documentation"
        );
    }

    #[test]
    fn test_args_parsing() {
        // Test that Args structure can be created
        let args = Args {
            database_path: Some(String::from("test.json")),
        };

        assert_eq!(args.database_path, Some(String::from("test.json")));
    }

    #[test]
    fn test_myapp_initial_state() {
        let database: Vec<CleanerData> = vec![];
        let app = MyApp::from_database(Arc::from(database.into_boxed_slice()));

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
}
