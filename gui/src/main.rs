#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use clap::{Parser, command};
use cleaner::clear_data;
#[cfg(windows)]
use database::registry_database;
#[cfg(windows)]
use database::structures::CleanerResult;
use database::structures::{CleanerData, Cleared};
use database::utils::get_file_size_string;
use database::{get_icon, get_version};
use eframe::egui;
use egui::IconData;
use image::ImageReader;
use notify_rust::Notification;
use std::cell::RefCell;
use std::collections::HashSet;
use std::io::Write;
use std::rc::Rc;
use std::sync::Arc;
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
    // INFO: 20px for 1 checkbox, 60px for button
    let height = (rows * 15) + 25;

    let size = egui::vec2(430.0, height as f32);
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
    let mut bytes_cleared = 0;
    let mut removed_files = 0;
    let mut removed_directories = 0;
    let mut cleared_programs = Vec::<Cleared>::with_capacity(database.len());

    // INFO: Check if LastActivity enabled
    // WARN: Windows only
    #[cfg(windows)]
    let has_last_activity = categories.contains(&"LastActivity".to_string());

    let mut tasks = Vec::with_capacity(database.len() + 1);

    // INFO: Clear LastActivity from Registry
    // WARN: Windiws only
    #[cfg(windows)]
    if has_last_activity {
        let progress_sender = progress_sender.clone();
        let task = task::spawn(async move {
            let _ = progress_sender.send("LastActivity".to_string()).await;
            let bytes_cleared = registry_database::clear_last_activity();
            CleanerResult {
                files: 0,
                folders: 0,
                bytes: bytes_cleared,
                working: true,
                path: String::new(),
                program: String::from("Registry"),
                category: String::from("LastActivity"),
            }
        });
        tasks.push(task);
    }

    let categories_set: HashSet<String> = categories.into_iter().collect();

    for data in database
        .iter()
        .filter(|&data| {
            categories_set.contains(&data.category) && !excluded_programs.contains(&data.program)
        })
        .cloned()
    {
        let data = Arc::new(data);
        let progress_sender = progress_sender.clone();
        let task = task::spawn(async move {
            let _ = progress_sender.send(data.path.to_string()).await;
            clear_data(&data)
        });
        tasks.push(task);
    }

    let total_tasks = tasks.len();
    let _ = progress_sender
        .send(format!("PROGRESS:0:{}", total_tasks))
        .await;

    for task in tasks {
        match task.await {
            Ok(result) => {
                current_task += 1;
                let _ = progress_sender
                    .send(format!("PROGRESS:{}:{}", current_task, total_tasks))
                    .await;
                if result.working {
                    bytes_cleared += result.bytes;
                    removed_files += result.files;
                    removed_directories += result.folders;

                    if let Some(cleared) = cleared_programs
                        .iter_mut()
                        .find(|c| c.program == result.program)
                    {
                        cleared.removed_bytes += result.bytes as u64;
                        cleared.removed_files += result.files as u64;
                        cleared.removed_directories += result.folders as u64;
                        if !cleared.affected_categories.contains(&result.category) {
                            cleared.affected_categories.push(result.category.clone());
                        }
                    } else {
                        let cleared = Cleared {
                            program: result.program.clone(),
                            removed_bytes: result.bytes as u64,
                            removed_files: result.files as u64,
                            removed_directories: result.folders as u64,
                            affected_categories: vec![result.category.clone()],
                        };
                        cleared_programs.push(cleared);
                    }
                }
            }
            Err(_) => {
                eprintln!("Error waiting for task completion");
            }
        }
    }

    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(get_icon()).unwrap();
    let icon_path = temp_file.path().to_str().unwrap();

    let notification_body = format!(
        "Removed: {}\nFiles: {}\nDirs: {}",
        get_file_size_string(bytes_cleared),
        removed_files,
        removed_directories
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
        bytes_cleared,
        removed_files,
        removed_directories,
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
    pub excluded_programs: HashSet<String>,

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
            excluded_programs: HashSet::new(),

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
                    let total_width = column_widths.iter().sum::<f32>() + 100.0;

                    // Динамический расчет высоты на основе количества программ
                    let num_programs = cleared.len();
                    let row_height = 20.0; // Высота одной строки
                    let header_height = 100.0; // Заголовок с результатами + заголовки таблицы
                    let min_scroll_height = 100.0; // Минимальная высота для прокрутки
                    let max_scroll_height = 400.0; // Максимальная высота для прокрутки

                    // Расчет необходимой высоты для всех строк
                    let content_height = num_programs as f32 * row_height;
                    let scroll_height =
                        content_height.min(max_scroll_height).max(min_scroll_height);
                    let total_height = header_height + scroll_height + 50.0;

                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::Vec2::new(
                        total_width,
                        total_height,
                    )));

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
                            .max_height(scroll_height)
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
                // Resize window for program selection
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::Vec2::new(
                    400.0, 520.0,
                )));

                ui.heading("Select Programs to Clean");
                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("Search:");
                    let search_response = ui.text_edit_singleline(&mut self.search_query);
                    if search_response.changed() {
                        self.search_query = self.search_query.to_lowercase();
                    }
                });

                ui.separator();

                egui::ScrollArea::vertical()
                    .max_height(400.0)
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
