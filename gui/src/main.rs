#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

// PERFORMANCE: Use mimalloc for blazing fast memory allocation
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use clap::{ArgAction, Parser};
use cleaner::clear_data;
#[cfg(windows)]
use database::registry_database::clear_registry;
#[cfg(windows)]
use database::structures::CleanerDataRegistry;
use database::structures::{CleanerData, CleanerResult, Cleared, CustomCleaner};
use database::utils::get_file_size_string;
use database::{get_icon, get_version};
use eframe::egui;
use egui::IconData;
use futures::stream::{FuturesUnordered, StreamExt};
use image::ImageReader;
use notify_rust::Notification;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::io::Write;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use tempfile::NamedTempFile;
use tokio::sync::mpsc;

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
    let icon_bytes = get_icon();
    let icon = load_icon_from_bytes(icon_bytes).expect("Failed to load icon");

    let args = Args::parse();

    // INFO: Register all built-in custom cleanings (functions in cleaner::custom_cleaners)
    cleaner::custom_cleaners::register_all();

    let custom_database: Arc<[CustomCleaner]> = if args.disable_custom {
        Arc::from(Vec::new())
    } else {
        Arc::from(database::custom_cleaners::get_custom_cleaners())
    };

    let database: Vec<CleanerData> = if let Some(db_path) = &args.database_path {
        match database::cleaner_database::get_database_from_file(db_path) {
            Ok(db) => db,
            Err(e) => {
                eprintln!("Failed to load database from file: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        database::cleaner_database::get_default_database().to_vec()
    };

    #[cfg(windows)]
    let registry_database: Vec<CleanerDataRegistry> = {
        if let Some(db_path) = &args.registry_database_path {
            match database::registry_database::get_database_from_file(db_path) {
                Ok(db) => db,
                Err(e) => {
                    eprintln!("Failed to load database from file: {}", e);
                    std::process::exit(1);
                }
            }
        } else {
            database::registry_database::get_default_database().to_vec()
        }
    };
    #[cfg(windows)]
    let app = MyApp::from_database(
        Arc::from(database),
        Arc::from(registry_database),
        custom_database,
    );
    #[cfg(not(windows))]
    let app = MyApp::from_database(Arc::from(database), custom_database);
    let checkbox_count = app.categories.len();
    let rows = checkbox_count.div_ceil(3);
    // INFO: 20px for 1 checkbox, 45px for button, 32px for custom title bar
    let height = (rows * 20) + 45 + TITLE_BAR_HEIGHT as usize;

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

/// Height of the custom title bar (in points).
const TITLE_BAR_HEIGHT: f32 = 32.0;

// Width reserved for the close & minimize buttons (drag area excludes them
// so a single click always reaches the buttons instead of starting a drag).
const TITLE_BAR_BUTTONS_WIDTH: f32 = 96.0;

/// Custom window title bar: drag-to-move, minimize and close buttons.
/// Optionally shows a back button (returns whether it was clicked).
fn title_bar(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    title: &str,
    icon_texture: Option<&egui::TextureHandle>,
    show_back: bool,
) -> bool {
    let back_clicked = RefCell::new(false);
    let panel_frame = egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(8, 0))
        .fill(ui.visuals().window_fill);

    let title_bar = egui::Panel::top("custom_title_bar")
        .exact_size(TITLE_BAR_HEIGHT)
        .resizable(false)
        .frame(panel_frame)
        .show(ui, |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Windows order: close rightmost, minimize to its left.
                // Glyphs are painted manually (default egui font has no ✕/— glyphs).
                let close = title_bar_button(ui);
                if close.clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                paint_close_glyph(ui, close.rect);
                let minimize = title_bar_button(ui);
                if minimize.clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                }
                paint_minimize_glyph(ui, minimize.rect);
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    if show_back {
                        let back = title_bar_button(ui);
                        if back.clicked() {
                            *back_clicked.borrow_mut() = true;
                        }
                        paint_back_glyph(ui, back.rect);
                        ui.add_space(4.0);
                    }
                    ui.add_space(4.0);
                    if let Some(tex) = icon_texture {
                        let icon = egui::Image::from_texture(egui::load::SizedTexture::new(
                            tex.id(),
                            tex.size_vec2(),
                        ))
                        .fit_to_exact_size(egui::vec2(16.0, 16.0));
                        ui.add_sized(egui::vec2(16.0, 16.0), icon);
                    }
                    ui.strong(title);
                });
            });
        });
    let back_clicked = back_clicked.into_inner();

    // Drag area: whole bar except the button zone on the right, so buttons
    // get a single click instead of the drag overlay swallowing it.
    let bar_rect = title_bar.response.rect;
    let left_reserved = if show_back { 44.0 } else { 0.0 };
    let drag_rect = egui::Rect::from_min_max(
        egui::pos2(
            (bar_rect.min.x + left_reserved).min(bar_rect.max.x),
            bar_rect.min.y,
        ),
        egui::pos2(
            (bar_rect.max.x - TITLE_BAR_BUTTONS_WIDTH).max(bar_rect.min.x),
            bar_rect.max.y,
        ),
    );
    let drag_response = ui.interact(
        drag_rect,
        ui.id().with("title_bar_drag"),
        egui::Sense::click_and_drag(),
    );
    if drag_response.drag_started() {
        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }

    back_clicked
}

/// A flat click area in the title bar (hover highlight, no frame).
fn title_bar_button(ui: &mut egui::Ui) -> egui::Response {
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(40.0, TITLE_BAR_HEIGHT), egui::Sense::click());
    if resp.hovered() || resp.is_pointer_button_down_on() {
        let fill = if resp.is_pointer_button_down_on() {
            ui.visuals().widgets.active.bg_fill
        } else {
            ui.visuals().widgets.hovered.bg_fill
        };
        ui.painter().rect_filled(rect, 0.0, fill);
    }
    resp
}

fn paint_close_glyph(ui: &egui::Ui, rect: egui::Rect) {
    let c = rect.center();
    let h = 5.0;
    let stroke = egui::Stroke::new(1.5, ui.visuals().text_color());
    ui.painter().line_segment(
        [egui::pos2(c.x - h, c.y - h), egui::pos2(c.x + h, c.y + h)],
        stroke,
    );
    ui.painter().line_segment(
        [egui::pos2(c.x - h, c.y + h), egui::pos2(c.x + h, c.y - h)],
        stroke,
    );
}

fn paint_minimize_glyph(ui: &egui::Ui, rect: egui::Rect) {
    let c = rect.center();
    let stroke = egui::Stroke::new(1.5, ui.visuals().text_color());
    ui.painter().line_segment(
        [egui::pos2(c.x - 5.0, c.y), egui::pos2(c.x + 5.0, c.y)],
        stroke,
    );
}

fn paint_back_glyph(ui: &egui::Ui, rect: egui::Rect) {
    let c = rect.center();
    let stroke = egui::Stroke::new(1.5, ui.visuals().text_color());
    ui.painter().line_segment(
        [egui::pos2(c.x + 2.5, c.y - 5.0), egui::pos2(c.x - 2.5, c.y)],
        stroke,
    );
    ui.painter().line_segment(
        [egui::pos2(c.x - 2.5, c.y), egui::pos2(c.x + 2.5, c.y + 5.0)],
        stroke,
    );
}

/// Decodes the application icon into an egui texture (original colors).
fn load_icon_color_image() -> egui::ColorImage {
    let img = ImageReader::new(std::io::Cursor::new(get_icon()))
        .with_guessed_format()
        .expect("app icon format")
        .decode()
        .expect("decode app icon")
        .to_rgba8();
    let (w, h) = (img.width() as usize, img.height() as usize);
    egui::ColorImage {
        size: [w, h],
        source_size: egui::Vec2::new(w as f32, h as f32),
        pixels: img
            .pixels()
            .map(|p| egui::Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
            .collect(),
    }
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
    selected_map: HashMap<String, HashSet<String>>,
    progress_sender: mpsc::Sender<String>,
    database: &[CleanerData],
    custom_database: &[CustomCleaner],
    #[cfg(windows)] registry_database: &[CleanerDataRegistry],
    excluded_programs: HashSet<String>,
) -> (u64, u64, u64, Vec<Cleared>) {
    let mut current_task = 0;

    // ASYNC without threads: pure FuturesUnordered
    let mut bytes_cleared: u64 = 0;
    let mut removed_files: u64 = 0;
    let mut removed_directories: u64 = 0;
    let mut cleared_programs = Vec::<Cleared>::with_capacity(database.len());

    // C: limit to 16 concurrent cleaners
    let sem = Arc::new(tokio::sync::Semaphore::new(16));
    let mut futures: FuturesUnordered<
        Pin<Box<dyn Future<Output = database::structures::CleanerResult> + Send>>,
    > = FuturesUnordered::new();

    // INFO: Clear LastActivity from Registry
    // WARN: Windows only - показываем что СЕЙЧАС чистится
    #[cfg(windows)]
    {
        for data in registry_database.iter() {
            let eff = effective_sub(&data.class, &data.sub_category);
            if let Some(subs) = selected_map.get(&data.category) {
                if subs.contains(&eff) && !excluded_programs.contains(&data.program) {
                    let data = data.clone();
                    let sender = progress_sender.clone();
                    let name_msg = data.program.clone();
                    let sem = sem.clone();
                    futures.push(Box::pin(async move {
                        let _p = sem.acquire_owned().await.unwrap();
                        let _ = sender.send(format!("Cleaning: {}", name_msg)).await;
                        clear_registry(&data)
                    }));
                }
            }
        }
    }

    // INFO: Run built-in custom cleanings (functions defined in cleaner::custom_cleaners)
    for data in custom_database.iter() {
        let eff = effective_sub("", &data.sub_category);
        if let Some(subs) = selected_map.get(&data.category) {
            if subs.contains(&eff) && !excluded_programs.contains(&data.program) {
                let data = data.clone();
                let sender = progress_sender.clone();
                let name_msg = data.id.clone();
                let sem = sem.clone();
                futures.push(Box::pin(async move {
                    let _p = sem.acquire_owned().await.unwrap();
                    let _ = sender.send(format!("Cleaning: {}", name_msg)).await;
                    tokio::task::spawn_blocking(move || {
                        database::custom_cleaners::run_custom_cleaner(&data)
                    })
                    .await
                    .unwrap_or_else(|_| CleanerResult {
                        files: 0,
                        folders: 0,
                        bytes: 0,
                        working: false,
                        path: String::new(),
                        program: String::new(),
                        category: String::new(),
                        sub_category: String::new(),
                    })
                }));
            }
        }
    }

    for data in database.iter() {
        let eff = effective_sub(&data.class, &data.sub_category);
        if let Some(subs) = selected_map.get(&data.category) {
            if subs.contains(&eff) && !excluded_programs.contains(&data.program) {
                let data = data.clone();
                let sender = progress_sender.clone();
                let path_msg = data.program.clone();
                let sem = sem.clone();
                futures.push(Box::pin(async move {
                    let _p = sem.acquire_owned().await.unwrap();
                    let _ = sender.send(format!("Cleaning: {}", path_msg)).await;
                    clear_data(&data).await
                }));
            }
        }
    }

    let total_tasks = futures.len();
    let _ = progress_sender
        .send(format!("PROGRESS:0:{}:0", total_tasks))
        .await;

    while let Some(result) = futures.next().await {
        current_task += 1;

        if result.working {
            bytes_cleared += result.bytes;
            removed_files += result.files;
            removed_directories += result.folders;

            if let Some(cleared) = cleared_programs
                .iter_mut()
                .find(|c| c.program == result.program)
            {
                cleared.removed_bytes += result.bytes;
                cleared.removed_files += result.files;
                cleared.removed_directories += result.folders;
                if !cleared.affected_categories.contains(&result.category) {
                    cleared.affected_categories.push(result.category);
                }
            } else {
                cleared_programs.push(Cleared {
                    program: result.program,
                    removed_bytes: result.bytes,
                    removed_files: result.files,
                    removed_directories: result.folders,
                    affected_categories: vec![result.category],
                });
            }
        }

        // Отправляем только прогресс и очищенные байты, путь уже отправлен перед очисткой
        let _ = progress_sender
            .send(format!(
                "PROGRESS:{}:{}:{}",
                current_task, total_tasks, bytes_cleared
            ))
            .await;
    }

    let bytes_cleared_val = bytes_cleared;
    let removed_files_val = removed_files;
    let removed_directories_val = removed_directories;

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

#[derive(Clone, Debug)]
struct CategoryState {
    name: String,
    subs: Vec<String>,
    has_empty: bool,
    selected: HashSet<String>,
}

impl CategoryState {
    fn is_unchecked(&self) -> bool {
        self.selected.is_empty()
    }

    fn is_checked(&self) -> bool {
        if self.subs.is_empty() && !self.has_empty {
            !self.selected.is_empty()
        } else if self.has_empty {
            // fully checked = all subs + empty selected
            self.selected.len() == self.subs.len() + 1 && self.selected.contains("")
        } else {
            !self.subs.is_empty() && self.selected.len() == self.subs.len()
        }
    }
    fn is_indeterminate(&self) -> bool {
        if self.subs.is_empty() && !self.has_empty {
            false
        } else {
            !self.selected.is_empty() && !self.is_checked()
        }
    }
}

fn effective_sub(_class: &str, sub_category: &str) -> String {
    sub_category.to_string()
}

/// Tristate checkbox with square (filled rect) for indeterminate state.
/// Returns response and whether state changed via click.
fn tristate_checkbox(
    ui: &mut egui::Ui,
    checked: bool,
    indeterminate: bool,
    text: &str,
) -> (egui::Response, bool) {
    // Use a mutable dummy bool for Checkbox widget (it will toggle on click)
    let mut dummy = checked;
    // We don't rely on Checkbox's indeterminate painting (hline); we will paint square ourselves.
    // So pass false to avoid double paint, and we handle visual manually.
    let mut response = ui.add(egui::Checkbox::new(&mut dummy, text));
    // If indeterminate, we need to paint square overlay manually
    if indeterminate && ui.is_rect_visible(response.rect) {
        // Calculate icon rect similar to Checkbox impl
        let icon_width = ui.spacing().icon_width;
        let rect = response.rect;
        // icon is at left side, centered vertically
        let icon_rect = egui::Rect::from_min_size(
            egui::pos2(rect.min.x, rect.center().y - icon_width / 2.0),
            egui::vec2(icon_width, icon_width),
        );
        // small inner square (shrink)
        let small_rect = icon_rect.shrink(4.0);
        let visuals = ui.style().interact(&response);
        // Use bg_fill for outer, but for indeterminate we fill inner square with fg color
        // Mimic checkbox bg
        ui.painter()
            .rect_filled(small_rect, 1.0, visuals.fg_stroke.color);
        // Also need to erase the hline that Checkbox didn't draw (we passed false, so no hline)
        // So nothing else
    } else if indeterminate {
        // still need to ensure checkbox appears indeterminate visually even if not visible yet
        // nothing
    }
    let clicked = response.clicked();
    // When clicked, dummy has been toggled (!checked) but for indeterminate we want custom toggle handling outside
    // Return whether clicked
    response.mark_changed();
    (response, clicked)
}

struct MyApp {
    pub categories: Vec<CategoryState>,
    /// legacy alias kept for tests compat: returns categories length etc.
    /// We keep checked_boxes as deprecated view for tests via method, but store as categories.
    /// For internal compat we also expose checked_boxes as computed (not stored). However test expects field.
    /// So we add a helper method and keep field via Deref? Instead keep both via getter.
    pub task_handle: Option<tokio::task::JoinHandle<(u64, u64, u64, Vec<Cleared>)>>,
    pub progress_message: String,
    pub progress_receiver: Option<mpsc::Receiver<String>>,
    pub cleared_data: Option<(u64, u64, u64, Vec<Cleared>)>,
    pub show_results: bool,
    pub current_task: usize,
    pub total_tasks: usize,
    pub cleaned_bytes: u64,
    pub progress_start: Option<std::time::Instant>,

    pub show_program_selection: bool,
    pub program_checkboxes: Vec<(Rc<RefCell<bool>>, String)>,
    pub search_query: String,
    pub search_query_visible: String,
    pub excluded_programs: HashSet<String>,
    pub results_window_resized: bool,

    pub result_sender: Option<mpsc::Sender<(u64, u64, u64, Vec<Cleared>)>>,
    pub result_receiver: Option<mpsc::Receiver<(u64, u64, u64, Vec<Cleared>)>>,

    pub database: Arc<[CleanerData]>,
    pub custom_database: Arc<[CustomCleaner]>,
    #[cfg(windows)]
    pub regisry_database: Arc<[CleanerDataRegistry]>,
    pub menu_texture: Option<egui::TextureHandle>,
    pub icon_texture: Option<egui::TextureHandle>,
}

// Embedded menu image bytes (required to be embedded)
const MENU_BYTES: &[u8] = include_bytes!("../assets/menu.png");

fn load_menu_color_image() -> egui::ColorImage {
    // White with original alpha; actual color applied at draw time via tint()
    let img = ImageReader::new(std::io::Cursor::new(MENU_BYTES))
        .with_guessed_format()
        .expect("menu png format")
        .decode()
        .expect("decode menu")
        .to_rgba8();
    let (w, h) = (img.width() as usize, img.height() as usize);
    let mut pixels = Vec::with_capacity(w * h);
    for p in img.pixels() {
        let a = p[3];
        if a < 10 {
            pixels.push(egui::Color32::TRANSPARENT);
        } else {
            pixels.push(egui::Color32::from_rgba_unmultiplied(255, 255, 255, a));
        }
    }
    egui::ColorImage {
        size: [w, h],
        source_size: egui::Vec2::new(w as f32, h as f32),
        pixels,
    }
}

impl MyApp {
    // Helper for legacy test code: checked_boxes view
    #[allow(dead_code)]
    pub fn checked_boxes(&self) -> Vec<(Rc<RefCell<bool>>, String)> {
        self.categories
            .iter()
            .map(|c| {
                let b = c.is_checked();
                (Rc::new(RefCell::new(b)), c.name.clone())
            })
            .collect()
    }

    #[cfg(windows)]
    pub(crate) fn from_database(
        database: Arc<[CleanerData]>,
        reg_database: Arc<[CleanerDataRegistry]>,
        custom_database: Arc<[CustomCleaner]>,
    ) -> Self {
        let mut cat_to_subs: HashMap<String, HashSet<String>> = HashMap::new();
        let mut cat_has_empty: HashMap<String, bool> = HashMap::new();
        // Ensure all categories appear even if no sub_category
        for data in database.iter() {
            cat_to_subs.entry(data.category.clone()).or_default();
            cat_has_empty.entry(data.category.clone()).or_insert(false);
            let sub = effective_sub(&data.class, &data.sub_category);
            if !sub.is_empty() {
                cat_to_subs.get_mut(&data.category).unwrap().insert(sub);
            } else {
                *cat_has_empty.get_mut(&data.category).unwrap() = true;
            }
        }
        for data in custom_database.iter() {
            cat_to_subs.entry(data.category.clone()).or_default();
            cat_has_empty.entry(data.category.clone()).or_insert(false);
            let sub = effective_sub("", &data.sub_category);
            if !sub.is_empty() {
                cat_to_subs.get_mut(&data.category).unwrap().insert(sub);
            } else {
                *cat_has_empty.get_mut(&data.category).unwrap() = true;
            }
        }
        for data in reg_database.iter() {
            if data.category.is_empty() {
                continue;
            }
            cat_to_subs.entry(data.category.clone()).or_default();
            cat_has_empty.entry(data.category.clone()).or_insert(false);
            let sub = effective_sub(&data.class, &data.sub_category);
            if !sub.is_empty() {
                cat_to_subs.get_mut(&data.category).unwrap().insert(sub);
            } else {
                *cat_has_empty.get_mut(&data.category).unwrap() = true;
            }
        }
        let mut options: Vec<String> = cat_to_subs.keys().cloned().collect();

        let priority = |s: &str| match s {
            "Cache" => 0,
            "Logs" => 1,
            "Crashes" => 2,
            "Documentation" => 3,
            "Backups" => 4,
            "LastActivity" => 5,
            _ => 6,
        };

        options.sort_by(|a, b| {
            let a_prio = priority(a);
            let b_prio = priority(b);

            if a_prio == b_prio {
                a.cmp(b)
            } else {
                a_prio.cmp(&b_prio)
            }
        });

        let mut categories = vec![];
        for opt in options {
            let mut subs: Vec<String> = cat_to_subs
                .remove(&opt)
                .unwrap_or_default()
                .into_iter()
                .collect();
            subs.sort();
            let has_empty = cat_has_empty.remove(&opt).unwrap_or(false);
            categories.push(CategoryState {
                name: opt,
                subs,
                has_empty,
                selected: HashSet::new(),
            });
        }

        let (result_sender, result_receiver) = mpsc::channel(1);

        Self {
            database,
            custom_database,
            #[cfg(windows)]
            regisry_database: reg_database,
            categories,
            task_handle: None,
            progress_message: String::new(),
            progress_receiver: None,
            cleared_data: None,
            show_results: false,
            current_task: 0,
            total_tasks: 0,
            cleaned_bytes: 0,
            progress_start: None,

            show_program_selection: false,
            program_checkboxes: vec![],
            search_query: String::new(),
            search_query_visible: String::new(),
            excluded_programs: HashSet::new(),
            results_window_resized: false,

            result_sender: Some(result_sender),
            result_receiver: Some(result_receiver),
            menu_texture: None,
            icon_texture: None,
        }
    }

    #[cfg(not(windows))]
    pub(crate) fn from_database(
        database: Arc<[CleanerData]>,
        custom_database: Arc<[CustomCleaner]>,
    ) -> Self {
        let mut cat_to_subs: HashMap<String, HashSet<String>> = HashMap::new();
        let mut cat_has_empty: HashMap<String, bool> = HashMap::new();
        for data in database.iter() {
            cat_to_subs.entry(data.category.clone()).or_default();
            cat_has_empty.entry(data.category.clone()).or_insert(false);
            let sub = effective_sub(&data.class, &data.sub_category);
            if !sub.is_empty() {
                cat_to_subs.get_mut(&data.category).unwrap().insert(sub);
            } else {
                *cat_has_empty.get_mut(&data.category).unwrap() = true;
            }
        }
        for data in custom_database.iter() {
            cat_to_subs.entry(data.category.clone()).or_default();
            cat_has_empty.entry(data.category.clone()).or_insert(false);
            let sub = effective_sub("", &data.sub_category);
            if !sub.is_empty() {
                cat_to_subs.get_mut(&data.category).unwrap().insert(sub);
            } else {
                *cat_has_empty.get_mut(&data.category).unwrap() = true;
            }
        }

        let mut options: Vec<String> = cat_to_subs.keys().cloned().collect();

        let priority = |s: &str| match s {
            "Cache" => 0,
            "Logs" => 1,
            "Crashes" => 2,
            "Documentation" => 3,
            "Backups" => 4,
            "LastActivity" => 5,
            _ => 6,
        };

        options.sort_by(|a, b| {
            let a_prio = priority(a);
            let b_prio = priority(b);

            if a_prio == b_prio {
                a.cmp(b)
            } else {
                a_prio.cmp(&b_prio)
            }
        });

        let mut categories = vec![];
        for opt in options {
            let mut subs: Vec<String> = cat_to_subs
                .remove(&opt)
                .unwrap_or_default()
                .into_iter()
                .collect();
            subs.sort();
            let has_empty = cat_has_empty.remove(&opt).unwrap_or(false);
            categories.push(CategoryState {
                name: opt,
                subs,
                has_empty,
                selected: HashSet::new(),
            });
        }

        let (result_sender, result_receiver) = mpsc::channel(1);

        Self {
            database,
            custom_database,
            categories,
            task_handle: None,
            progress_message: String::new(),
            progress_receiver: None,
            cleared_data: None,
            show_results: false,
            current_task: 0,
            total_tasks: 0,
            cleaned_bytes: 0,
            progress_start: None,

            show_program_selection: false,
            program_checkboxes: vec![],
            search_query: String::new(),
            search_query_visible: String::new(),
            excluded_programs: HashSet::new(),
            results_window_resized: false,

            result_sender: Some(result_sender),
            result_receiver: Some(result_receiver),
            menu_texture: None,
            icon_texture: None,
        }
    }

    fn selected_map(&self) -> HashMap<String, HashSet<String>> {
        let mut map = HashMap::new();
        for cat in &self.categories {
            if !cat.selected.is_empty() {
                map.insert(cat.name.clone(), cat.selected.clone());
            }
        }
        map
    }

    fn has_selection(&self) -> bool {
        self.categories.iter().any(|c| !c.selected.is_empty())
    }
}

impl eframe::App for MyApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let focused = ctx.input(|i| i.viewport().focused.unwrap_or(false));
        if focused {
            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("focused_window_border"),
            ));
            let r = ctx.viewport_rect();
            let t = 2.0;
            let c = egui::Color32::from_rgb(0, 120, 215);
            painter.rect_filled(
                egui::Rect::from_min_max(r.min, egui::pos2(r.max.x, r.min.y + t)),
                0.0,
                c,
            );
            painter.rect_filled(
                egui::Rect::from_min_max(
                    egui::pos2(r.min.x, r.max.y - t - 2.0),
                    egui::pos2(r.max.x, r.max.y - 2.0),
                ),
                0.0,
                c,
            );
            painter.rect_filled(
                egui::Rect::from_min_max(r.min, egui::pos2(r.min.x + t, r.max.y)),
                0.0,
                c,
            );
            painter.rect_filled(
                egui::Rect::from_min_max(
                    egui::pos2(r.max.x - t, r.min.y),
                    egui::pos2(r.max.x, r.max.y),
                ),
                0.0,
                c,
            );
        }
        if let Some(receiver) = &mut self.progress_receiver {
            if let Ok(message) = receiver.try_recv() {
                if message.starts_with("PROGRESS:") {
                    let parts: Vec<&str> = message.split(':').collect();
                    if parts.len() == 4 {
                        self.current_task = parts[1].parse().unwrap_or(0);
                        self.total_tasks = parts[2].parse().unwrap_or(0);
                        self.cleaned_bytes = parts[3].parse().unwrap_or(0);
                        if self.progress_start.is_none() {
                            self.progress_start = Some(std::time::Instant::now());
                        }
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
                self.result_receiver = None; // Consume the result once
                ctx.request_repaint();
            }
        }

        if let Some(handle) = &mut self.task_handle {
            if handle.is_finished() {
                let handle = self.task_handle.take().unwrap();
                if let Some(sender) = self.result_sender.take() {
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
        }

        let title = format!("Cross Cleaner GUI v{}", get_version());
        if self.icon_texture.is_none() {
            self.icon_texture = Some(ctx.load_texture(
                "app_icon",
                load_icon_color_image(),
                egui::TextureOptions::LINEAR,
            ));
        }
        let show_back =
            (self.show_results && self.cleared_data.is_some()) || self.show_program_selection;
        let back_clicked = title_bar(ui, &ctx, &title, self.icon_texture.as_ref(), show_back);
        if back_clicked {
            if self.show_program_selection {
                self.show_program_selection = false;
            } else {
                self.show_results = false;
                self.cleared_data = None;
                self.results_window_resized = false;
            }
        }
        let inner_margin = 8;
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .inner_margin(egui::Margin::same(inner_margin))
                    .fill(ui.visuals().panel_fill),
            )
            .show(ui, |ui| {
                if self.task_handle.is_some() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::Vec2::new(
                        470.0,
                        100.0 + TITLE_BAR_HEIGHT,
                    )));
                    // Панель даёт 8px, текст добирает 12px от краёв экрана
                    ui.vertical(|ui| {
                        ui.add_space(4.0);
                        // Слева сверху: имя программы, которая сейчас чистится
                        let program = self
                            .progress_message
                            .strip_prefix("Cleaning: ")
                            .unwrap_or(&self.progress_message)
                            .to_string();
                        if !program.is_empty() {
                            ui.horizontal(|ui| {
                                ui.add_space(4.0);
                                ui.strong(&program);
                            });
                        }
                        ui.add_space(4.0);

                        if self.total_tasks > 0 {
                            let progress = self.current_task as f32 / self.total_tasks as f32;
                            // Прогресс-бар: 8px от краёв экрана
                            ui.add_sized(
                                [ui.available_width(), 20.0],
                                egui::ProgressBar::new(progress)
                                    .show_percentage()
                                    .animate(true),
                            );
                            ui.add_space(4.0);

                            let eta = self.progress_start.and_then(|start| {
                                if self.current_task == 0 || self.current_task >= self.total_tasks {
                                    None
                                } else {
                                    let elapsed = start.elapsed().as_secs_f64();
                                    let per_task = elapsed / self.current_task as f64;
                                    let remaining =
                                        per_task * (self.total_tasks - self.current_task) as f64;
                                    let mins = (remaining / 60.0).floor() as u64;
                                    let secs = (remaining % 60.0).round() as u64;
                                    if mins > 0 {
                                        Some(format!("~{}m {:02}s", mins, secs))
                                    } else {
                                        Some(format!("~{}s", secs))
                                    }
                                }
                            });

                            ui.horizontal(|ui| {
                                ui.add_space(4.0);
                                // Слева снизу: сколько очистилось
                                ui.label(get_file_size_string(self.cleaned_bytes));
                                // Справа снизу: оставшееся время
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Min),
                                    |ui| {
                                        if let Some(eta) = eta {
                                            ui.label(eta);
                                        }
                                        ui.add_space(4.0);
                                    },
                                );
                            });
                        } else {
                            ui.spinner();
                        }
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
                            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(
                                egui::Vec2::new(total_width, total_height + TITLE_BAR_HEIGHT),
                            ));
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
                                            ui.style_mut().spacing.item_spacing =
                                                egui::vec2(0.0, 0.0);

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
                    let scroll_height =
                        content_height.min(max_scroll_height).max(min_scroll_height);
                    let window_height = base_height + scroll_height;

                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::Vec2::new(
                        500.0,
                        window_height + TITLE_BAR_HEIGHT,
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
                            .add_sized([available_width, 25.0], egui::Button::new("Start Cleaning"))
                            .clicked()
                        {
                            let selected_map = self.selected_map();

                            self.excluded_programs.clear();
                            for (checkbox, program) in &self.program_checkboxes {
                                if !*checkbox.borrow() {
                                    self.excluded_programs.insert(program.clone());
                                }
                            }

                            let (progress_sender, progress_receiver) = mpsc::channel(32);
                            self.progress_receiver = Some(progress_receiver);
                            let (result_sender, result_receiver) = mpsc::channel(1);
                            self.result_sender = Some(result_sender);
                            self.result_receiver = Some(result_receiver);
                            self.current_task = 0;
                            self.total_tasks = 0;
                            self.cleaned_bytes = 0;
                            self.progress_start = None;
                            self.results_window_resized = false;

                            let database = Arc::clone(&self.database);
                            let custom_database = Arc::clone(&self.custom_database);
                            #[cfg(windows)]
                            let reg_database = Arc::clone(&self.regisry_database);
                            let excluded_programs = self.excluded_programs.clone();
                            let handle = tokio::spawn(async move {
                                work(
                                    selected_map,
                                    progress_sender,
                                    &database,
                                    &custom_database,
                                    #[cfg(windows)]
                                    &reg_database,
                                    excluded_programs,
                                )
                                .await
                            });
                            self.task_handle = Some(handle);

                            self.show_program_selection = false;
                            // clear selection
                            for cat in &mut self.categories {
                                cat.selected.clear();
                            }
                        }
                    });
                } else {
                    // Calculate dynamic window height based on number of categories
                    let num_categories = self.categories.len();
                    let rows = (num_categories + 2) / 3; // Round up division by 3 (3 columns)
                    let row_height = 20.0; // Approximate height per row
                    let base_height = 45.0; // Space for heading, margins, and button
                    let dynamic_height = base_height + (rows as f32 * row_height);
                    let window_height = dynamic_height.max(20.0).min(500.0); // Clamp between 200 and 500

                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::Vec2::new(
                        470.0,
                        window_height + TITLE_BAR_HEIGHT,
                    )));

                    if self.menu_texture.is_none() {
                        self.menu_texture = Some(ctx.load_texture(
                            "menu",
                            load_menu_color_image(),
                            egui::TextureOptions::LINEAR,
                        ));
                    }
                    let menu_tex = self.menu_texture.clone().unwrap();

                    ui.columns(3, |columns| {
                        for (idx, cat) in self.categories.iter_mut().enumerate() {
                            let column_index = idx % 3;
                            let is_checked = cat.is_checked();
                            let is_indet = cat.is_indeterminate();

                            columns[column_index].horizontal(|ui| {
                                // Tristate checkbox with square for indeterminate
                                let (resp, clicked) =
                                    tristate_checkbox(ui, is_checked, is_indet, &cat.name.clone());
                                if clicked {
                                    if is_checked || is_indet {
                                        cat.selected.clear();
                                    } else {
                                        cat.selected = cat.subs.iter().cloned().collect();
                                        if cat.has_empty {
                                            cat.selected.insert(String::new());
                                        }
                                        if cat.subs.is_empty() && !cat.has_empty {
                                            cat.selected.insert(String::new());
                                        }
                                    }
                                }
                                // menu image only if sub-categories exist (embedded menu.png)
                                if !cat.subs.is_empty() {
                                    let menu_image =
                                        egui::Image::from_texture(egui::load::SizedTexture::new(
                                            menu_tex.id(),
                                            menu_tex.size_vec2(),
                                        ))
                                        .fit_to_exact_size(egui::vec2(16.0, 16.0))
                                        .tint(ui.visuals().text_color())
                                        .sense(egui::Sense::click());
                                    let menu_resp =
                                        ui.add_sized(egui::vec2(16.0, 16.0), menu_image);

                                    // Popup with sub_category checkboxes - shifted to right-bottom corner of image so it doesn't cover the button
                                    let frame = egui::Frame::popup(ui.style());
                                    egui::Popup::menu(&menu_resp)
                                        .close_behavior(
                                            egui::PopupCloseBehavior::CloseOnClickOutside,
                                        )
                                        .frame(frame)
                                        .show(|ui| {
                                            ui.set_min_width(200.0);
                                            egui::ScrollArea::vertical().max_height(300.0).show(
                                                ui,
                                                |ui| {
                                                    for sub in cat.subs.clone() {
                                                        let mut is_sel =
                                                            cat.selected.contains(&sub);
                                                        if ui.checkbox(&mut is_sel, &sub).changed()
                                                        {
                                                            if is_sel {
                                                                cat.selected.insert(sub.clone());
                                                            } else {
                                                                cat.selected.remove(&sub);
                                                            }
                                                        }
                                                    }
                                                    // Show Uncategorized for objects without sub_category, only if category has ≥1 real sub
                                                    if cat.has_empty {
                                                        let mut is_uncat =
                                                            cat.selected.contains("");
                                                        if ui
                                                            .checkbox(
                                                                &mut is_uncat,
                                                                "Uncategorized",
                                                            )
                                                            .changed()
                                                        {
                                                            if is_uncat {
                                                                cat.selected.insert(String::new());
                                                            } else {
                                                                cat.selected.remove(&String::new());
                                                            }
                                                        }
                                                    }
                                                },
                                            );
                                        });
                                }
                                let _ = resp;
                            });
                        }
                    });

                    let available_width = ui.available_width();

                    if ui
                        .add_sized([available_width, 25.0], egui::Button::new("Next"))
                        .clicked()
                    {
                        if self.has_selection() {
                            let selected_map = self.selected_map();
                            let mut programs: Vec<String> = Vec::new();
                            for data in self.database.iter() {
                                let eff = effective_sub(&data.class, &data.sub_category);
                                if let Some(subs) = selected_map.get(&data.category) {
                                    if subs.contains(&eff) && !programs.contains(&data.program) {
                                        programs.push(data.program.clone());
                                    }
                                }
                            }
                            for data in self.custom_database.iter() {
                                let eff = effective_sub("", &data.sub_category);
                                if let Some(subs) = selected_map.get(&data.category) {
                                    if subs.contains(&eff) && !programs.contains(&data.program) {
                                        programs.push(data.program.clone());
                                    }
                                }
                            }
                            #[cfg(windows)]
                            {
                                for data in self.regisry_database.iter() {
                                    let eff = effective_sub(&data.class, &data.sub_category);
                                    if let Some(subs) = selected_map.get(&data.category) {
                                        if subs.contains(&eff) && !programs.contains(&data.program)
                                        {
                                            programs.push(data.program.clone());
                                        }
                                    }
                                }
                            }
                            programs.sort();

                            self.program_checkboxes.clear();
                            for program in programs {
                                self.program_checkboxes
                                    .push((Rc::new(RefCell::new(true)), program));
                            }

                            self.show_program_selection = true;
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
