use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;
use eframe::egui;
use egui::Context;
use indicatif::{ProgressBar, ProgressStyle};
use notify_rust::Notification;
use tabled::Table;
use tokio::sync::mpsc;
use tokio::task;
use cleaner::clear_data;
use database::{get_winbooster_version, registry_database};
use database::structures::{CleanerData, CleanerResult, Cleared};
use database::utils::get_file_size_string;

#[tokio::main]
async fn main() -> eframe::Result {
    env_logger::init();

    let options = eframe::NativeOptions {
        run_and_return: true,
        #[cfg(windows)]
        viewport: egui::ViewportBuilder::default().with_inner_size([430.0, 150.0]),
        #[cfg(unix)]
        viewport: egui::ViewportBuilder::default().with_inner_size([430.0, 125.0]),
        ..Default::default()
    };

    eframe::run_native(
        &*("WinBooster Definitive Edition GUI v".to_owned() + &*get_winbooster_version()),
        options.clone(),
        Box::new(|_cc| {
            _cc.egui_ctx.set_visuals(egui::Visuals::dark());
            Ok(Box::new(MyApp::new()))
        }),
    )
}

async fn work(
    ctx: Context,
    disabled_programs: Vec<&str>,
    categories: Vec<String>,
    database: Vec<CleanerData>,
    progress_sender: mpsc::Sender<String>,
) {
    let sty = ProgressStyle::with_template(
        "[{elapsed_precise}] {prefix:.bold.dim} {spinner:.green}\n[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} [{msg}]",
    ).unwrap().progress_chars("##-").tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ");

    let mut bytes_cleared = 0;
    let mut removed_files = 0;
    let mut removed_directories = 0;
    let mut cleared_programs: HashSet<String> = HashSet::new();

    let pb = ProgressBar::new(0);
    pb.set_style(sty.clone());
    pb.set_prefix("Clearing");
    let has_last_activity = categories.contains(&"LastActivity".to_string());

    let mut tasks = Vec::new();

    if has_last_activity {
        let progress_bar = Arc::new(pb.clone());
        let task = task::spawn(async move {
            progress_bar.set_message("LastActivity");
            #[cfg(windows)]
            registry_database::clear_last_activity();
            progress_bar.inc(1);
            CleanerResult {
                files: 0,
                folders: 0,
                bytes: 0,
                working: false,
                path: String::new(),
                program: String::new(),
            }
        });
        tasks.push(task);
    }

    let disabled_programs_set: HashSet<&str> = disabled_programs.into_iter().collect();
    let categories_set: HashSet<String> = categories.into_iter().collect();

    for data in database
        .into_iter()
        .filter(|data| categories_set.contains(&data.category))
    {
        if disabled_programs_set.contains(data.program.as_str()) {
            continue;
        }

        let data = Arc::new(data);
        let progress_bar = Arc::new(pb.clone());
        let task = task::spawn(async move {
            progress_bar.set_message(format!("{}", data.path));
            let result = clear_data(&data);
            progress_bar.inc(1);
            result
        });
        tasks.push(task);
    }

    pb.set_length(tasks.len() as u64);

    for task in tasks {
        match task.await {
            Ok(result) => {
                removed_files += result.files;
                removed_directories += result.folders;
                bytes_cleared += result.bytes;
                if result.working {
                    cleared_programs.insert(result.program);
                }
            },
            Err(_) => {
                eprintln!("Error waiting for task completion");
            }
        }
    }

    pb.set_message("done");
    pb.finish();

    println!("Cleared programs:");
    let table = Table::new(cleared_programs.into_iter()).to_string();
    println!("{}", table);
    println!("Removed: {}", get_file_size_string(bytes_cleared));
    println!("Removed files: {}", removed_files);
    println!("Removed directories: {}", removed_directories);

    let _ = Notification::new()
        .summary("WinBooster GUI")
        .body(&*("Removed: ".to_owned() + &*get_file_size_string(bytes_cleared) + "\nFiles: " + &*removed_files.to_string()))
        .show();
}

struct MyApp {
    pub(crate) checked_boxes: Vec<(Rc<RefCell<bool>>, String)>,
    pub(crate) selected_options: Vec<String>,
    pub(crate) task_handle: Option<tokio::task::JoinHandle<()>>,
    pub(crate) progress_message: String, // Сообщение о прогрессе
    pub(crate) progress_receiver: Option<mpsc::Receiver<String>>, // Канал для получения сообщений о прогрессе
}

impl MyApp {
    pub(crate) fn new() -> Self {
        let database: Vec<CleanerData> = database::cleaner_database::get_database();

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
            selected_options: vec![],
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

                if ui.add_sized([available_width, 25.0], egui::Button::new("Clear")).clicked() {
                    let selected_options: Vec<String> = self.checked_boxes.iter()
                        .filter(|(checkbox, _)| *checkbox.borrow())
                        .map(|(_, label)| label.clone())
                        .collect();

                    let database: Vec<CleanerData> = database::cleaner_database::get_database();

                    let (progress_sender, progress_receiver) = mpsc::channel(32);
                    self.progress_receiver = Some(progress_receiver);

                    let ctx = ctx.clone();
                    let handle = tokio::spawn(work(ctx, vec![], selected_options, database, progress_sender));
                    self.task_handle = Some(handle);

                    for (checkbox, _) in &self.checked_boxes {
                        *checkbox.borrow_mut() = false;
                    }
                }
            }
        });
    }
}