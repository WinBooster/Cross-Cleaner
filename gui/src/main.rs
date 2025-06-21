#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use clap::{Parser, command};
use cleaner::clear_data;
#[cfg(windows)]
use database::registry_database;
use database::structures::CleanerData;
#[cfg(windows)]
use database::structures::CleanerResult;
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

    let app = MyApp::from_database(&database);
    let checkbox_count = app.checked_boxes.len();
    let rows = checkbox_count.div_ceil(3);
    // INFO: 20px for 1 checkbox, 60px for button
    let height = (rows * 20) + 45;

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
) {
    let mut bytes_cleared = 0;
    let mut removed_files = 0;
    let mut removed_directories = 0;
    let mut cleared_programs: HashSet<String> = HashSet::with_capacity(database.len());

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
        .filter(|&data| categories_set.contains(&data.category))
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

    for task in tasks {
        match task.await {
            Ok(result) => {
                removed_files += result.files;
                removed_directories += result.folders;
                bytes_cleared += result.bytes;
                if result.working {
                    cleared_programs.insert(result.program);
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
        "Removed: {}, Files: {}, Dirs: {}",
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
}

struct MyApp {
    pub(crate) checked_boxes: Vec<(Rc<RefCell<bool>>, String)>,
    pub(crate) task_handle: Option<tokio::task::JoinHandle<()>>,
    pub(crate) progress_message: String,
    pub(crate) progress_receiver: Option<mpsc::Receiver<String>>,
}

impl MyApp {
    pub(crate) fn from_database(database: &[CleanerData]) -> Self {
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

        Self {
            checked_boxes,
            task_handle: None,
            progress_message: String::new(),
            progress_receiver: None,
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(receiver) = &mut self.progress_receiver {
            if let Ok(message) = receiver.try_recv() {
                self.progress_message = message;
                ctx.request_repaint();
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.columns(3, |columns| {
                for (i, (checkbox, label)) in self.checked_boxes.iter().enumerate() {
                    let column_index = i % 3;
                    let mut value = checkbox.borrow_mut();
                    columns[column_index].checkbox(&mut *value, label);
                }
            });

            if let Some(handle) = &self.task_handle {
                ui.label(&self.progress_message);
                if handle.is_finished() {
                    self.task_handle = None;
                }
            }

            if self.task_handle.is_none() {
                let available_width = ui.available_width();

                if ui
                    .add_sized([available_width, 25.0], egui::Button::new("Clear"))
                    .clicked()
                {
                    let selected_options: Vec<String> = self
                        .checked_boxes
                        .iter()
                        .filter(|(checkbox, _)| *checkbox.borrow())
                        .map(|(_, label)| label.clone())
                        .collect();

                    let database: &Vec<CleanerData> =
                        database::cleaner_database::get_default_database();

                    let (progress_sender, progress_receiver) = mpsc::channel(32);
                    self.progress_receiver = Some(progress_receiver);

                    let handle = tokio::spawn(work(selected_options, progress_sender, database));
                    self.task_handle = Some(handle);

                    for (checkbox, _) in &self.checked_boxes {
                        *checkbox.borrow_mut() = false;
                    }
                }
            }
        });
    }
}
