//! Custom window chrome: title bar with drag-to-move, window control
//! buttons, GitHub/donate shortcuts and the 2px window outline.

use std::cell::RefCell;

use eframe::egui;

use crate::sounds;

/// Height of the custom title bar (in points).
pub const TITLE_BAR_HEIGHT: f32 = 32.0;

/// Project repository, opened by the GitHub icon in the title bar.
const GITHUB_URL: &str = "https://github.com/WinBooster/Cross-Cleaner";
const DONATE_URL: &str = "https://nowpayments.io/donation/neki_play";

// Width reserved for the close, minimize & GitHub buttons (drag area excludes
// them so a single click always reaches the buttons instead of starting a drag).
const TITLE_BAR_BUTTONS_WIDTH: f32 = 162.0;

/// Paints the 2px window outline used by both the main window and the
/// changelog viewport (blue when focused, text color otherwise).
pub fn paint_window_border(ctx: &egui::Context, id: &str, color: egui::Color32) {
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new(id),
    ));
    let r = ctx.viewport_rect();
    let t = 2.0;
    painter.rect_filled(
        egui::Rect::from_min_max(r.min, egui::pos2(r.max.x, r.min.y + t)),
        0.0,
        color,
    );
    painter.rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(r.min.x, r.max.y - t - 2.0),
            egui::pos2(r.max.x, r.max.y - 2.0),
        ),
        0.0,
        color,
    );
    painter.rect_filled(
        egui::Rect::from_min_max(r.min, egui::pos2(r.min.x + t, r.max.y)),
        0.0,
        color,
    );
    painter.rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(r.max.x - t, r.min.y),
            egui::pos2(r.max.x, r.max.y),
        ),
        0.0,
        color,
    );
}

/// Custom window title bar: drag-to-move, minimize and close buttons.
/// Optionally shows a back button or settings button (returns (back_clicked, settings_clicked)).
pub fn title_bar(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    title: &str,
    icon_texture: Option<&egui::TextureHandle>,
    show_back: bool,
    show_settings: bool,
    settings_texture: Option<&egui::TextureHandle>,
) -> (bool, bool) {
    let back_clicked = RefCell::new(false);
    let settings_clicked = RefCell::new(false);
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
                // Glyphs are painted manually (default egui font has no check/cross glyphs).
                let close = title_bar_button(ui);
                if close.clicked() {
                    sounds::click();
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                paint_close_glyph(ui, close.rect);
                let minimize = title_bar_button(ui);
                if minimize.clicked() {
                    sounds::click();
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                }
                paint_minimize_glyph(ui, minimize.rect);
                // Vertical separator between window controls and the GitHub button.
                // Spans the full title bar height: from the top window border
                // down to the bottom of the title bar.
                let (sep_rect, _sep) =
                    ui.allocate_exact_size(egui::vec2(1.0, TITLE_BAR_HEIGHT), egui::Sense::hover());
                ui.painter().line_segment(
                    [
                        egui::pos2(sep_rect.center().x, sep_rect.min.y),
                        egui::pos2(sep_rect.center().x, sep_rect.max.y),
                    ],
                    egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
                );
                // GitHub icon (U+E624 from the built-in emoji-icon-font)
                // that opens the project repository.
                let github = title_bar_button(ui);
                if github.clicked() {
                    sounds::click();
                    open_in_browser(GITHUB_URL);
                }
                paint_glyph(ui, github.rect, "\u{e624}");
                github.on_hover_text("GitHub repository");

                // GitHub icon (U+24 from the built-in emoji-icon-font)
                // that opens the project repository.
                let donations = title_bar_button(ui);
                if donations.clicked() {
                    sounds::click();
                    open_in_browser(DONATE_URL);
                }
                paint_glyph(ui, donations.rect, "\u{24}");
                donations.on_hover_text("Donations");
                let (sep_rect, _sep) =
                    ui.allocate_exact_size(egui::vec2(1.0, TITLE_BAR_HEIGHT), egui::Sense::hover());
                ui.painter().line_segment(
                    [
                        egui::pos2(sep_rect.center().x, sep_rect.min.y),
                        egui::pos2(sep_rect.center().x, sep_rect.max.y),
                    ],
                    egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
                );
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    if show_back {
                        let back = title_bar_button(ui);
                        if back.clicked() {
                            sounds::click();
                            *back_clicked.borrow_mut() = true;
                        }
                        paint_back_glyph(ui, back.rect);
                    } else if show_settings && let Some(tex) = settings_texture {
                        #[cfg(not(target_os = "android"))]
                        {
                            let settings = title_bar_button(ui);
                            if settings.clicked() {
                                sounds::click();
                                *settings_clicked.borrow_mut() = true;
                            }
                            let icon = egui::Image::from_texture(egui::load::SizedTexture::new(
                                tex.id(),
                                tex.size_vec2(),
                            ))
                            .fit_to_exact_size(egui::vec2(16.0, 16.0))
                            .tint(ui.visuals().text_color());
                            ui.put(
                                egui::Rect::from_center_size(
                                    settings.rect.center(),
                                    egui::vec2(16.0, 16.0),
                                ),
                                icon,
                            );
                            settings.on_hover_text("Settings");
                        }
                    }
                    ui.add_space(2.0);
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
    let settings_clicked = settings_clicked.into_inner();

    // Drag area: whole bar except the button zone on the right, so buttons
    // get a single click instead of the drag overlay swallowing it.
    let bar_rect = title_bar.response.rect;
    let left_reserved = if show_back || show_settings {
        44.0
    } else {
        0.0
    };
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

    (back_clicked, settings_clicked)
}

/// A flat click area in the title bar (hover highlight, no frame).
fn title_bar_button(ui: &mut egui::Ui) -> egui::Response {
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(30.0, TITLE_BAR_HEIGHT), egui::Sense::click());
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

fn paint_glyph(ui: &egui::Ui, rect: egui::Rect, glyph: &str) {
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        glyph,
        egui::FontId::proportional(14.0),
        ui.visuals().text_color(),
    );
}

/// Opens a URL in the system browser.
#[cfg(windows)]
pub(crate) fn open_in_browser(url: &str) {
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use windows::core::{PCWSTR, w};

    if !url.starts_with("https://") {
        eprintln!("Refusing to open non-HTTPS URL: {url}");
        return;
    }
    let wide_url: Vec<u16> = url.encode_utf16().chain(std::iter::once(0)).collect();
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(wide_url.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    if result.0 as isize <= 32 {
        eprintln!("Failed to open URL in browser: {url}");
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn open_in_browser(url: &str) {
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

#[cfg(target_os = "macos")]
pub(crate) fn open_in_browser(url: &str) {
    let _ = std::process::Command::new("open").arg(url).spawn();
}

#[cfg(any(
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
))]
pub(crate) fn open_in_browser(url: &str) {
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

#[cfg(target_os = "android")]
pub(crate) fn open_in_browser(url: &str) {
    if !url.starts_with("https://") {
        eprintln!("Refusing to open non-HTTPS URL: {url}");
        return;
    }
    use jni::objects::{JObject, JValue};
    use jni::JavaVM;

    let ctx = ndk_context::android_context();
    let vm = match unsafe { JavaVM::from_raw(ctx.vm().cast()) } {
        Ok(vm) => vm,
        Err(err) => {
            eprintln!("[browser] Failed to get JavaVM: {err:?}");
            return;
        }
    };
    let mut env = match vm.attach_current_thread() {
        Ok(e) => e,
        Err(err) => {
            eprintln!("[browser] JNI attach failed: {err:?}");
            return;
        }
    };

    let result: Result<(), jni::errors::Error> = (|| {
        let activity = unsafe { JObject::from_raw(ctx.context().cast()) };

        // Uri.parse(url)
        let uri_class = env.find_class("android/net/Uri")?;
        let jurl = env.new_string(url)?;
        let uri = env
            .call_static_method(
                &uri_class,
                "parse",
                "(Ljava/lang/String;)Landroid/net/Uri;",
                &[JValue::Object(&jurl)],
            )?
            .l()?;

        // new Intent(Intent.ACTION_VIEW, uri)
        let intent_class = env.find_class("android/content/Intent")?;
        let action_view = env
            .get_static_field(&intent_class, "ACTION_VIEW", "Ljava/lang/String;")?
            .l()?;
        let intent = env.new_object(
            &intent_class,
            "(Ljava/lang/String;Landroid/net/Uri;)V",
            &[JValue::Object(&action_view), JValue::Object(&uri)],
        )?;

        // FLAG_ACTIVITY_NEW_TASK
        let flag = env
            .get_static_field(&intent_class, "FLAG_ACTIVITY_NEW_TASK", "I")?
            .i()?;
        env.call_method(
            &intent,
            "addFlags",
            "(I)Landroid/content/Intent;",
            &[JValue::Int(flag)],
        )?;

        env.call_method(
            &activity,
            "startActivity",
            "(Landroid/content/Intent;)V",
            &[JValue::Object(&intent)],
        )?;

        // check exception
        if env.exception_check()? {
            env.exception_describe()?;
            env.exception_clear()?;
            return Err(jni::errors::Error::JavaException);
        }
        Ok(())
    })();

    if let Err(e) = result {
        eprintln!("[browser] Failed to open URL {url}: {e:?}");
        // best effort: try to clear exception if still pending
        if let Ok(mut env2) = vm.attach_current_thread() {
            let _ = env2.exception_clear();
        }
    }
}

#[cfg(not(any(
    target_os = "windows",
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly",
    target_os = "android"
)))]
pub(crate) fn open_in_browser(_url: &str) {}

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
