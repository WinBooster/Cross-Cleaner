#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

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

#[tokio::main]
async fn main() -> eframe::Result {
    let icon_bytes = get_icon();
    let icon = load_icon_from_bytes(icon_bytes).expect("Failed to load icon");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([430.0, 150.0])
            .with_icon(icon),
        ..Default::default()
    };

    eframe::run_native(
        &*("Cross Cleaner GUI v".to_owned() + &*get_version()),
        options,
        Box::new(|_cc| {
            _cc.egui_ctx.set_visuals(egui::Visuals::dark());
            Ok(Box::new(MyApp::new()))
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
    database: &Vec<CleanerData>,
    progress_sender: mpsc::Sender<String>,
) {
    let mut bytes_cleared = 0;
    let mut removed_files = 0;
    let mut removed_directories = 0;
    let mut cleared_programs: HashSet<String> = HashSet::new();

    #[cfg(windows)]
    let has_last_activity = categories.contains(&"LastActivity".to_string());

    let mut tasks = Vec::new();

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
        .to_vec()
        .into_iter()
        .filter(|data| categories_set.contains(&data.category))
    {
        let data = Arc::new(data);
        let progress_sender = progress_sender.clone();
        let task = task::spawn(async move {
            let _ = progress_sender.send(format!("{}", data.path)).await;
            let result = clear_data(&data);
            result
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

    let notification_result = Notification::new()
        .summary("Cross Cleaner GUI")
        .body(
            &*("Removed: ".to_owned()
                + &*get_file_size_string(bytes_cleared)
                + "\nFiles: "
                + &*removed_files.to_string()
                + "\nDirs: "
                + &*removed_directories.to_string()),
        )
        #[cfg(windows)]
        .app_id("com.crosscleaner.gui")
        .icon(icon_path)
        .show();

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
    pub(crate) fn new() -> Self {
        let database: &Vec<CleanerData> = database::cleaner_database::get_default_database();

        let mut options: Vec<String> = vec![];
        for data in database.iter() {
            if !options.contains(&data.category) {
                options.push(data.category.clone());
            }
        }

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

                    let handle = tokio::spawn(work(selected_options, database, progress_sender));
                    self.task_handle = Some(handle);

                    for (checkbox, _) in &self.checked_boxes {
                        *checkbox.borrow_mut() = false;
                    }
                }
            }
        });
    }
}
