// ============================================================================
// Self-update worker
// ============================================================================
// Runs on its own thread and owns everything a frontend must not do: downloading
// the release binary and replacing the currently running executable with it.
//
// Each frontend only sends [`appcore::updater::UpdaterCommand`]s and renders the
// shared [`appcore::updater::UpdateStage`], so the UI crates stay free of any
// `self-replace` dependency — which is what keeps the Android build (and the
// terminal app on a machine without a matching asset) working.

use std::sync::mpsc::Receiver;
use std::time::Duration;

use appcore::updater::{UpdateStage, UpdateState, UpdaterCommand};
use database::version::{NewRelease, download_asset};

/// Serves update commands until the frontend closes the channel.
///
/// A failing update is reported back through `state` instead of panicking, so
/// the app keeps running and the user can retry or fall back to the release
/// page.
pub fn run(commands: Receiver<UpdaterCommand>, state: UpdateState) {
    while let Ok(command) = commands.recv() {
        match command {
            UpdaterCommand::Install(release) => {
                if let Err(error) = install(&release, &state) {
                    eprintln!("[updater] {error}");
                    appcore::updater::publish(
                        &state,
                        UpdateStage::Failed {
                            version: release.version.clone(),
                            error,
                        },
                    );
                }
            }
            UpdaterCommand::Restart => restart(&state),
        }
    }
}

/// Downloads `release` and replaces the running executable with it.
fn install(release: &NewRelease, state: &UpdateState) -> Result<(), String> {
    let url = release
        .asset_url
        .as_deref()
        .ok_or("This release has no binary for your platform")?;
    let version = release.version.clone();

    let file = download_asset(url, release.asset_size, |done, total| {
        // Progress ticks arrive once per 64 KiB; `publish` swallows the
        // duplicates, so the UI only repaints when the stage really changes.
        appcore::updater::publish(
            state,
            UpdateStage::Downloading {
                version: version.clone(),
                done,
                total,
            },
        );
    })?;

    appcore::updater::publish(
        state,
        UpdateStage::Installing {
            version: version.clone(),
        },
    );

    // On Windows the running image is renamed aside and the new binary is put
    // in its place before this returns, so the file on disk already holds the
    // new version once the call succeeded.
    self_replace::self_replace(&file).map_err(|e| describe_install_error(release, &version, e))?;

    // `self_replace` copied the file, so the download is ours to clean up.
    let _ = std::fs::remove_file(&file);

    appcore::updater::publish(
        state,
        UpdateStage::Installed {
            version: version.clone(),
        },
    );
    Ok(())
}

/// Turns a `self_replace` failure into something the user can act on.
///
/// The usual cause is a machine-wide install: replacing an executable needs
/// write access to its own folder, which `C:\Program Files` does not grant to
/// a normal user.
fn describe_install_error(release: &NewRelease, version: &str, error: std::io::Error) -> String {
    eprintln!("[updater] Failed to install v{version}: {error}");
    if error.kind() == std::io::ErrorKind::PermissionDenied {
        return format!(
            "Access denied while writing the update. Cross Cleaner is installed in a \
             protected folder — run it as administrator, or download v{version} from {}.",
            release.url
        );
    }
    format!("Could not install v{version}: {error}")
}

/// Starts a second copy of the app and then quits, so the version that keeps
/// running is the one that was just installed.
///
/// A failed relaunch is reported to the UI and not only to stderr: the update
/// is installed at this point, so the user has to be told that closing and
/// reopening the app is enough.
fn restart(state: &UpdateState) {
    let version = appcore::updater::current(state)
        .version()
        .unwrap_or("?")
        .to_string();
    if let Err(error) = relaunch() {
        eprintln!("[updater] {error}");
        appcore::updater::publish(
            state,
            UpdateStage::RestartFailed {
                version,
                error: format!("{error} Close and reopen Cross Cleaner to use the new version."),
            },
        );
        return;
    }
    // Quitting hands the executable back to `self_replace`, which only removes
    // the old image once this process is gone. Waiting keeps that helper from
    // pulling the file handle out from under the process just started; the
    // terminal (or window) goes away right after.
    std::thread::sleep(Duration::from_millis(300));
    // `exit` rather than returning: the frontend's event loop is still running
    // and would otherwise draw another frame over the new process.
    std::process::exit(0);
}

/// Starts another instance of the app with the original command line, so a
/// launch with e.g. `--database-path` keeps using the same database.
fn relaunch() -> Result<(), String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("Could not locate the executable to restart: {e}"))?;
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();

    match spawn(&exe, &args) {
        Ok(()) => Ok(()),
        Err(error) => {
            // Release builds may ask for administrator rights (see `desktop`'s
            // build script), so a non-elevated instance cannot start the new
            // binary directly. `runas` makes Windows show the usual UAC prompt
            // instead of failing outright.
            #[cfg(windows)]
            if error.raw_os_error() == Some(ERROR_ELEVATION_REQUIRED) {
                eprintln!("[updater] Restart needs elevation, asking Windows to elevate");
                return shell_execute_elevated(&exe, &args);
            }
            Err(format!("Failed to restart Cross Cleaner: {error}"))
        }
    }
}

/// `ERROR_ELEVATION_REQUIRED`: the target executable is marked to run as
/// administrator while this process is not.
#[cfg(windows)]
const ERROR_ELEVATION_REQUIRED: i32 = 740;

/// Starts `exe` directly.
fn spawn(exe: &std::path::Path, args: &[std::ffi::OsString]) -> std::io::Result<()> {
    std::process::Command::new(exe)
        .args(args)
        .spawn()
        .map(|_| ())
}

/// Starts `exe` through the shell with the `runas` verb, which raises the UAC
/// prompt when the current process is not elevated. Fails when the user
/// declines the prompt or the shell refuses.
#[cfg(windows)]
fn shell_execute_elevated(
    exe: &std::path::Path,
    args: &[std::ffi::OsString],
) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use windows::core::PCWSTR;

    fn wide(value: &std::ffi::OsStr) -> Vec<u16> {
        value.encode_wide().chain(std::iter::once(0)).collect()
    }

    let verb = wide(std::ffi::OsStr::new("runas"));
    let file = wide(exe.as_os_str());
    // ShellExecuteW takes the arguments as one command line, so they have to
    // be quoted the way the C runtime would quote them.
    let parameters = args
        .iter()
        .map(|arg| format!("\"{}\"", arg.to_string_lossy()))
        .collect::<Vec<_>>()
        .join(" ");
    let parameters = wide(std::ffi::OsStr::new(&parameters));

    // SAFETY: every pointer refers to a NUL-terminated `Vec<u16>` that
    // outlives the call, and `parameters` holds the terminator even when there
    // are no arguments.
    let result = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(verb.as_ptr()),
            PCWSTR(file.as_ptr()),
            PCWSTR(parameters.as_ptr()),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    // ShellExecuteW returns a fake HINSTANCE: anything <= 32 is an error code.
    if (result.0 as isize) <= 32 {
        return Err(
            "Failed to restart Cross Cleaner: the elevation request was declined".to_string(),
        );
    }
    Ok(())
}
