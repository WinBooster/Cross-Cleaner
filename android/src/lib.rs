//! Android entry via cargo-apk / cargo-apk2.
//! `gui` is library now, desktop launch moved to `desktop` crate.
//! This crate builds as cdylib and is loaded by NativeActivity.
//!
//! Responsive categories grid:
//! - desktop: 3 columns (see `gui::category_columns`)
//! - android landscape (width > height): 2 columns
//! - android portrait: 1 column

use database::cleaner_database::CleanerDatabase;
use database::get_version;
use database::structures::CustomCleaner;
use database::version::check_new_version;
use eframe::egui;
use gui::app::MyApp;
use std::sync::Arc;

#[cfg(target_os = "android")]
use winit::platform::android::activity::AndroidApp;

#[cfg(target_os = "android")]
fn ensure_manage_external_storage() {
    use jni::JavaVM;
    use jni::objects::{JObject, JValue};

    let ctx = ndk_context::android_context();
    let vm = match unsafe { JavaVM::from_raw(ctx.vm().cast()) } {
        Ok(vm) => vm,
        Err(err) => {
            eprintln!("[perms] Failed to get JavaVM: {err:?}");
            return;
        }
    };
    let mut env = match vm.attach_current_thread() {
        Ok(e) => e,
        Err(err) => {
            eprintln!("[perms] JNI attach failed: {err:?}");
            return;
        }
    };
    eprintln!("[perms] JNI attached ok");

    let already = (|| -> jni::errors::Result<bool> {
        let class = env.find_class("android/os/Environment")?;
        let res = env.call_static_method(class, "isExternalStorageManager", "()Z", &[])?;
        Ok(res.z()?)
    })()
    .unwrap_or(false);
    eprintln!("[perms] isExternalStorageManager = {already}");

    if already {
        return;
    }

    let run = (|| -> jni::errors::Result<()> {
        let activity = unsafe { JObject::from_raw(ctx.context().cast()) };

        // package:com.winbooster.crosscleaner
        let package_name = env
            .call_method(&activity, "getPackageName", "()Ljava/lang/String;", &[])?
            .l()?;

        let uri_class = env.find_class("android/net/Uri")?;
        let scheme = env.new_string("package")?;
        let null = JObject::null();
        let uri = env
            .call_static_method(
                &uri_class,
                "fromParts",
                "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)Landroid/net/Uri;",
                &[
                    JValue::Object(&scheme),
                    JValue::Object(&package_name),
                    JValue::Object(&null),
                ],
            )?
            .l()?;

        let settings_class = env.find_class("android/provider/Settings")?;
        let action = env
            .get_static_field(
                &settings_class,
                "ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION",
                "Ljava/lang/String;",
            )?
            .l()?;

        let intent_class = env.find_class("android/content/Intent")?;
        let intent = env.new_object(
            &intent_class,
            "(Ljava/lang/String;Landroid/net/Uri;)V",
            &[JValue::Object(&action), JValue::Object(&uri)],
        )?;

        let flag = env
            .get_static_field(&intent_class, "FLAG_ACTIVITY_NEW_TASK", "I")?
            .i()?;
        env.call_method(
            &intent,
            "addFlags",
            "(I)Landroid/content/Intent;",
            &[JValue::Int(flag)],
        )?;

        // === ГЛАВНОЕ: открыть экран настроек ===
        let res = env.call_method(
            &activity,
            "startActivity",
            "(Landroid/content/Intent;)V",
            &[JValue::Object(&intent)],
        );
        match res {
            Ok(_) => eprintln!("[perms] per-app settings intent sent (no exception)"),
            Err(e) => {
                eprintln!("[perms] per-app intent failed: {e:?}, trying global intent");
                // Fallback: общий список «Все файлы» без привязки к пакету
                let global_action = env
                    .get_static_field(
                        &settings_class,
                        "ACTION_MANAGE_ALL_FILES_ACCESS_PERMISSION",
                        "Ljava/lang/String;",
                    )?
                    .l()?;
                let intent2 = env.new_object(
                    &intent_class,
                    "(Ljava/lang/String;)V",
                    &[JValue::Object(&global_action)],
                )?;
                env.call_method(
                    &intent2,
                    "addFlags",
                    "(I)Landroid/content/Intent;",
                    &[JValue::Int(flag)],
                )?;
                env.call_method(
                    &activity,
                    "startActivity",
                    "(Landroid/content/Intent;)V",
                    &[JValue::Object(&intent2)],
                )?;
                eprintln!("[perms] global settings intent sent");
            }
        }
        Ok(())
    })();

    if let Err(e) = run {
        eprintln!("[perms] Failed to request MANAGE_EXTERNAL_STORAGE: {e:?}");
    }
}

/// Shared initialization: registers cleaners, builds database, creates MyApp.
fn create_myapp() -> MyApp {
    // Register custom cleaners (as desktop does)
    cleaner::custom_cleaners::register_all();
    let custom_database: Arc<[CustomCleaner]> =
        Arc::from(database::custom_cleaners::get_custom_cleaners());
    let database = CleanerDatabase::default_source();
    // Registry is Windows-only, not used on Android
    #[cfg(windows)]
    {
        // not reached on android
        let _ = custom_database;
        let _ = database;
        unreachable!()
    }
    #[cfg(not(windows))]
    {
        MyApp::from_database(database, custom_database)
    }
}

#[cfg(target_os = "android")]
struct AndroidWrapper {
    app: MyApp,
}

#[cfg(target_os = "android")]
impl eframe::App for AndroidWrapper {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        // System back on Android: winit maps AKEYCODE_BACK to Escape / close_requested.
        let ctx = ui.ctx().clone();
        let back_via_key = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let back_via_close = ctx.input(|i| i.viewport().close_requested());
        if back_via_key || back_via_close {
            if self.app.go_back() {
                // Cancel the pending close if we consumed the back as navigation.
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            }
        }
        self.app.ui(ui, frame);
    }
}

/// Common eframe run helper used both on Android and for `cargo run -p android` on desktop (for testing).
fn run_eframe(event_loop: winit::event_loop::EventLoop<eframe::UserEvent>) -> eframe::Result {
    let app_title = format!("Cross Cleaner v{}", get_version());
    // Android uses fullscreen wgpu; no need for fixed size or glow fallback.
    let icon = gui::icons::load_icon_from_ico_bytes(database::ICON_BYTES).ok();
    let viewport = {
        let vp = egui::ViewportBuilder::default().with_decorations(true);
        if let Some(ic) = icon {
            vp.with_icon(ic)
        } else {
            vp
        }
    };
    let options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };

    gui::sounds::init();

    let app_for_closure = {
        let mut a = create_myapp();
        let (tx, rx) = std::sync::mpsc::channel();
        a.update_receiver = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(check_new_version());
        });
        a
    };

    let mut native_app = eframe::create_native(
        &app_title,
        options,
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            #[cfg(target_os = "android")]
            {
                Ok(Box::new(AndroidWrapper {
                    app: app_for_closure,
                }) as Box<dyn eframe::App>)
            }
            #[cfg(not(target_os = "android"))]
            {
                Ok(Box::new(app_for_closure) as Box<dyn eframe::App>)
            }
        }),
        &event_loop,
    );
    event_loop
        .run_app(&mut native_app)
        .map_err(eframe::Error::WinitEventLoop)
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    #[cfg(target_os = "android")]
    ensure_manage_external_storage();

    // Android entry point called by NativeActivity.
    // Build event loop that is tied to the AndroidApp.
    use winit::event_loop::EventLoop;
    use winit::platform::android::EventLoopBuilderExtAndroid;

    // Tokio runtime for `MyApp` (which spawns async cleaning tasks)
    // Keep runtime alive for the whole app lifetime.
    // Use a dedicated thread for tokio, similar to desktop's #[tokio::main]
    // Instead we create a runtime and block on running eframe.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");

    rt.block_on(async {
        // EventLoop must be created on the main thread with android app
        let event_loop: EventLoop<eframe::UserEvent> =
            EventLoop::<eframe::UserEvent>::with_user_event()
                .with_android_app(app)
                .build()
                .expect("android event loop");
        // run is blocking, but we are inside tokio async context – use spawn_blocking?
        // eframe's run will block current thread, which is okay because we are on main thread.
        // We need to run without blocking tokio executor, so use blocking thread.
        // However `event_loop.run_app` is !Send and must run on this thread, so we run directly.
        // To avoid blocking tokio, we run it in a blocking way but tokio's block_on will just wait.
        let _ = run_eframe(event_loop);
    });
}

// For testing on desktop (`cargo run -p android`) we provide a regular main-like entry.
// When compiled for non-android (e.g. windows host), `cargo run -p android` will just run this.
#[cfg(not(target_os = "android"))]
#[allow(dead_code)]
fn main_like() -> eframe::Result {
    use winit::event_loop::EventLoop;
    let event_loop = EventLoop::<eframe::UserEvent>::with_user_event()
        .build()
        .expect("event loop");
    // Need tokio runtime as MyApp uses tokio::spawn for cleaning
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    rt.block_on(async {
        let _ = run_eframe(event_loop);
        Ok(())
    })
}

// Public helper for `cargo run` on host (not used by apk)
#[cfg(not(target_os = "android"))]
pub fn run_on_host() -> eframe::Result {
    main_like()
}

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }

    #[test]
    fn test_category_columns_desktop() {
        // On host (not android) category_columns should return 3
        let ctx = egui::Context::default();
        // screen_rect on default context is zero, fallback still returns 3 on non-android
        assert_eq!(gui::category_columns(&ctx), 3);
    }

    #[test]
    fn test_category_rows() {
        assert_eq!(gui::category_rows(7, 3), 3);
        assert_eq!(gui::category_rows(7, 2), 4);
        assert_eq!(gui::category_rows(7, 1), 7);
        assert_eq!(gui::category_rows(0, 3), 0);
    }
}
