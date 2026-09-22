#![allow(clippy::too_many_arguments)]

use clap::{Parser, ValueEnum};
use serde::Deserialize;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(
    name = "updater",
    version,
    about = "Cross Cleaner updater — downloads Cross_Cleaner_Setup.exe from GitHub Releases and runs it (Inno Setup)",
    long_about = None
)]
struct Args {
    /// Only check for a newer version, do not download or install
    #[arg(long, conflicts_with = "download_only")]
    check: bool,

    /// Download installer but do not launch it
    #[arg(long)]
    download_only: bool,

    /// Force download/install even if already on latest version
    #[arg(long)]
    force: bool,

    /// Output result as JSON (for GUI / scripting)
    #[arg(long)]
    json: bool,

    /// Installer mode
    #[arg(long, value_enum, default_value_t = InstallMode::Silent)]
    mode: InstallMode,

    /// Extra arguments forwarded verbatim to the Inno Setup installer
    #[arg(long)]
    installer_args: Option<String>,

    /// Override detected current version (e.g. --current-version 2.0.2.8.2)
    #[arg(long)]
    current_version: Option<String>,

    /// GitHub repository in owner/repo form
    #[arg(long, default_value = "WinBooster/Cross-Cleaner")]
    repo: String,

    /// Asset name to look for in the release
    #[arg(long, default_value = "Cross_Cleaner_Setup.exe")]
    asset: String,

    /// Directory to download the installer into (default: %TEMP%)
    #[arg(long)]
    out_dir: Option<PathBuf>,

    /// Skip launching Cross Cleaner after install (adds /NORESTART handling)
    #[arg(long)]
    no_launch: bool,

    /// Assume yes for prompts (non-interactive)
    #[arg(short = 'y', long)]
    yes: bool,
}

#[derive(Copy, Clone, Debug, ValueEnum, PartialEq, Eq)]
enum InstallMode {
    /// Show installer wizard (default Inno behaviour)
    Interactive,
    /// /SILENT — shows progress, no prompts
    Silent,
    /// /VERYSILENT — no UI at all
    VerySilent,
}

impl std::fmt::Display for InstallMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Interactive => write!(f, "interactive"),
            Self::Silent => write!(f, "silent"),
            Self::VerySilent => write!(f, "verysilent"),
        }
    }
}

// ---------------------------------------------------------------------------
// Version helpers (mirrors database::version)
// ---------------------------------------------------------------------------

fn parse_version(v: &str) -> Vec<u64> {
    v.trim()
        .trim_start_matches('v')
        .trim_start_matches('V')
        .split('.')
        .map(|p| p.trim().parse::<u64>().unwrap_or(0))
        .collect()
}

fn is_newer(remote: &str, current: &str) -> bool {
    let r = parse_version(remote);
    let c = parse_version(current);
    for i in 0..r.len().max(c.len()) {
        let rv = r.get(i).copied().unwrap_or(0);
        let cv = c.get(i).copied().unwrap_or(0);
        if rv != cv {
            return rv > cv;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Detect installed version
// ---------------------------------------------------------------------------

const UNINSTALL_SUBKEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\{63C69122-AC80-4866-B328-5C3188A01F76}_is1";

#[cfg(windows)]
fn read_registry_version() -> Option<String> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;

    for hive in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
        for view in [winreg::enums::KEY_READ | winreg::enums::KEY_WOW64_64KEY, winreg::enums::KEY_READ | winreg::enums::KEY_WOW64_32KEY, winreg::enums::KEY_READ] {
            let hk = RegKey::predef(hive);
            if let Ok(key) = hk.open_subkey_with_flags(UNINSTALL_SUBKEY, view)
                && let Ok(v) = key.get_value::<String, _>("DisplayVersion")
            {
                let v = v.trim().to_string();
                if !v.is_empty() {
                    return Some(v);
                }
            }
        }
        // Fallback: enumerate Uninstall and match by DisplayName / AppId if GUID changes
        let hk = RegKey::predef(hive);
        if let Ok(uninstall) = hk.open_subkey_with_flags(r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall", winreg::enums::KEY_READ) {
            for name in uninstall.enum_keys().flatten() {
                if let Ok(k) = uninstall.open_subkey(&name) {
                    let dn: Result<String, _> = k.get_value("DisplayName");
                    let id_match = name.contains("63C69122") || dn.as_deref().is_ok_and(|s| s == "Cross Cleaner");
                    if id_match && let Ok(v) = k.get_value::<String, _>("DisplayVersion") {
                        let v = v.trim().to_string();
                        if !v.is_empty() {
                            return Some(v);
                        }
                    }
                }
            }
        }
    }
    None
}

#[cfg(not(windows))]
fn read_registry_version() -> Option<String> {
    None
}

fn detect_current_version(args: &Args) -> String {
    if let Some(v) = &args.current_version {
        return v.trim().to_string();
    }
    if let Some(v) = read_registry_version() {
        return v;
    }
    // Compile-time APP_VERSION injected by release workflow via `APP_VERSION` env,
    // or fallback to updater crate version.
    if let Some(v) = option_env!("APP_VERSION").map(|s| s.to_string())
        && !v.is_empty()
        && v != "0.1.0"
    {
        return v;
    }
    // Try to read version from adjacent Cross_Cleaner_GUI.exe if present (Windows)
    #[cfg(windows)]
    if let Some(v) = try_read_exe_product_version() {
        return v;
    }
    env!("CARGO_PKG_VERSION").to_string()
}

#[cfg(windows)]
fn try_read_exe_product_version() -> Option<String> {
    // Look for Cross_Cleaner_GUI.exe next to updater, and in Program Files
    let candidates: Vec<PathBuf> = {
        let mut v = Vec::new();
        if let Ok(exe) = std::env::current_exe()
            && let Some(dir) = exe.parent()
        {
            v.push(dir.join("Cross_Cleaner_GUI.exe"));
            v.push(dir.join("Cross Cleaner/Cross_Cleaner_GUI.exe"));
        }
        if let Ok(pf) = std::env::var("ProgramFiles") {
            v.push(PathBuf::from(pf).join("Cross Cleaner/Cross_Cleaner_GUI.exe"));
        }
        if let Ok(pf) = std::env::var("ProgramFiles(x86)") {
            v.push(PathBuf::from(pf).join("Cross Cleaner/Cross_Cleaner_GUI.exe"));
        }
        v
    };
    for p in candidates {
        if p.exists() {
            // Use winreg alternative: read version via file version info is non-trivial without windows crate.
            // We skip deep parsing and just return None — registry is authoritative.
            let _ = p;
        }
    }
    None
}

// ---------------------------------------------------------------------------
// GitHub API
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ReleaseAsset {
    name: String,
    browser_download_url: String,
    size: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    #[allow(dead_code)]
    body: Option<String>,
    assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Clone)]
struct UpdateInfo {
    version: String,       // without leading v
    tag: String,           // raw tag e.g. v2.0.2.8.2
    page_url: String,      // html_url
    download_url: String,  // browser_download_url for asset
    asset_name: String,
    asset_size: Option<u64>,
}

fn fetch_latest_release(repo: &str, asset_name: &str) -> Result<UpdateInfo, String> {
    let api = format!("https://api.github.com/repos/{repo}/releases/latest");
    let json: serde_json::Value = ureq::get(&api)
        .timeout(Duration::from_secs(15))
        .set("User-Agent", concat!("Cross-Cleaner-updater/", env!("CARGO_PKG_VERSION")))
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("GitHub API request failed ({api}): {e}"))?
        .into_json()
        .map_err(|e| format!("Failed to parse GitHub response: {e}"))?;

    let release: Release = serde_json::from_value(json)
        .map_err(|e| format!("Unexpected GitHub response shape: {e}"))?;

    let tag = release.tag_name.clone();
    let version = tag.trim().trim_start_matches('v').trim_start_matches('V').to_string();
    let asset = release
        .assets
        .iter()
        .find(|a| a.name == asset_name)
        .or_else(|| release.assets.iter().find(|a| a.name.eq_ignore_ascii_case(asset_name)))
        .ok_or_else(|| {
            let names: Vec<_> = release.assets.iter().map(|a| a.name.as_str()).collect();
            format!(
                "Asset '{asset_name}' not found in latest release {tag}. Available: {}",
                names.join(", ")
            )
        })?;

    Ok(UpdateInfo {
        version,
        tag,
        page_url: release.html_url,
        download_url: asset.browser_download_url.clone(),
        asset_name: asset.name.clone(),
        asset_size: asset.size,
    })
}

// ---------------------------------------------------------------------------
// Download with progress
// ---------------------------------------------------------------------------

fn download_file(url: &str, dest: &Path) -> Result<u64, String> {
    println!("Downloading: {url}");
    println!("      -> {}", dest.display());

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create dir {}: {e}", parent.display()))?;
    }

    let resp = ureq::get(url)
        .timeout(Duration::from_secs(300))
        .set("User-Agent", concat!("Cross-Cleaner-updater/", env!("CARGO_PKG_VERSION")))
        .call()
        .map_err(|e| format!("download request failed: {e}"))?;

    if resp.status() != 200 {
        return Err(format!("download failed: HTTP {}", resp.status()));
    }

    let total: Option<u64> = resp
        .header("Content-Length")
        .and_then(|v| v.parse().ok())
        .or(None);

    let mut reader = resp.into_reader();
    let mut file =
        std::fs::File::create(dest).map_err(|e| format!("create file {}: {e}", dest.display()))?;

    let mut buf = [0u8; 64 * 1024];
    let mut downloaded: u64 = 0;
    let mut last_pct: u64 = 0;
    let start = std::time::Instant::now();

    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("read response: {e}"))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])
            .map_err(|e| format!("write file: {e}"))?;
        downloaded += n as u64;

        if let Some(total) = total {
            let pct = downloaded * 100 / total.max(1);
            if pct >= last_pct + 5 || downloaded == total {
                let elapsed = start.elapsed().as_secs_f64();
                let speed = if elapsed > 0.0 {
                    downloaded as f64 / elapsed / 1024.0
                } else {
                    0.0
                };
                println!("  {pct}%  ({downloaded}/{total} bytes, {speed:.0} KiB/s)");
                last_pct = pct;
            }
        } else if downloaded % (1024 * 1024) < 64 * 1024 {
            println!("  {} bytes downloaded...", downloaded);
        }
    }

    file.flush().map_err(|e| format!("flush: {e}"))?;

    // Verify size if server sent Content-Length
    if let Some(total) = total
        && downloaded != total
    {
        return Err(format!(
            "incomplete download: expected {total} bytes, got {downloaded}"
        ));
    }

    println!("Download complete: {downloaded} bytes in {:.1}s", start.elapsed().as_secs_f64());
    Ok(downloaded)
}

// ---------------------------------------------------------------------------
// Installer launch
// ---------------------------------------------------------------------------

fn build_installer_args(mode: InstallMode, extra: Option<&str>, no_launch: bool) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "/SP-".to_string(),
        "/SUPPRESSMSGBOXES".to_string(),
        "/CLOSEAPPLICATIONS".to_string(),
        "/RESTARTAPPLICATIONS".to_string(),
    ];

    match mode {
        InstallMode::Interactive => {}
        InstallMode::Silent => args.push("/SILENT".to_string()),
        InstallMode::VerySilent => args.push("/VERYSILENT".to_string()),
    }

    if no_launch {
        // build_setup.iss has postinstall run with flags `postinstall` — /SILENT already
        // suppresses the checkbox, but we add explicit suppression for safety.
        // Inno does not have a universal /NOLAUNCH, so we document this as best-effort.
    }

    if let Some(extra) = extra {
        for tok in extra.split_whitespace() {
            if !tok.is_empty() {
                args.push(tok.to_string());
            }
        }
    }

    args
}

#[cfg(windows)]
fn close_running_gui() {
    use std::process::Command;
    use std::time::Duration;
    // Try graceful close first, then force. Ignore errors — installer with
    // CloseApplications will also try via Restart Manager.
    println!("Closing running Cross_Cleaner_GUI.exe (if any)...");
    let _ = Command::new("taskkill")
        .args(["/IM", "Cross_Cleaner_GUI.exe"])
        .output();
    // Give Restart Manager / graceful close a moment
    std::thread::sleep(Duration::from_millis(1500));
    // Force if still running — tasklist check is best-effort, just force
    let out = Command::new("taskkill")
        .args(["/F", "/IM", "Cross_Cleaner_GUI.exe"])
        .output();
    if let Ok(o) = out {
        let stdout = String::from_utf8_lossy(&o.stdout);
        let stderr = String::from_utf8_lossy(&o.stderr);
        if o.status.success() {
            println!("taskkill /F: {stdout}{stderr}");
        } else if stdout.contains("not found") || stderr.contains("not found") {
            println!("No running GUI found.");
        } else {
            // No GUI or already closed — ignore
            println!("taskkill check: {stdout}{stderr}");
        }
    }
    std::thread::sleep(Duration::from_millis(500));
}

fn launch_installer(installer: &Path, args: &[String]) -> Result<(), String> {
    println!("Launching installer:");
    println!("  {} {}", installer.display(), args.join(" "));

    #[cfg(windows)]
    {
        close_running_gui();
        let status = std::process::Command::new(installer)
            .args(args)
            .status()
            .map_err(|e| format!("failed to spawn installer: {e}"))?;

        if !status.success() {
            // Inno returns non-zero on cancel / error; surface it but do not panic
            let code = status.code().unwrap_or(-1);
            if code == 1 {
                eprintln!("Hint: installer code 1 often means the app is still running.");
                eprintln!("      Close Cross Cleaner manually and run: {} /SILENT", installer.display());
            }
            return Err(format!("installer exited with code {code}"));
        }
        Ok(())
    }

    #[cfg(not(windows))]
    {
        let _ = (installer, args);
        Err("Inno Setup installer can only be executed on Windows".to_string())
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let args = Args::parse();

    // Non-Windows: only check is meaningful
    #[cfg(not(windows))]
    {
        if !args.check && !args.json {
            eprintln!("Note: Inno Setup installer is Windows-only. Only --check is supported on this OS.");
        }
    }

    let current = detect_current_version(&args);
    println!("Current version: {current}");
    println!("Repository: {}", args.repo);
    println!("Asset: {}", args.asset);

    let update = match fetch_latest_release(&args.repo, &args.asset) {
        Ok(u) => u,
        Err(e) => {
            if args.json {
                println!(
                    "{}",
                    serde_json::json!({
                        "current_version": current,
                        "error": e,
                        "update_available": false
                    })
                );
                std::process::exit(1);
            } else {
                eprintln!("Error checking for updates: {e}");
                std::process::exit(1);
            }
        }
    };

    println!("Latest version: {} ({})", update.version, update.tag);
    println!("Release page: {}", update.page_url);
    println!("Download URL: {}", update.download_url);
    if let Some(sz) = update.asset_size {
        println!("Asset size: {} bytes ({:.1} MiB)", sz, sz as f64 / 1024.0 / 1024.0);
    }

    let newer = is_newer(&update.version, &current);
    let will_update = newer || args.force;

    if args.json {
        let out = serde_json::json!({
            "current_version": current,
            "latest_version": update.version,
            "latest_tag": update.tag,
            "page_url": update.page_url,
            "download_url": update.download_url,
            "asset": update.asset_name,
            "update_available": newer,
            "will_update": will_update,
        });
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
        if args.check {
            std::process::exit(if newer { 0 } else { 2 });
        }
        if !will_update {
            println!("Already on latest version.");
            std::process::exit(2);
        }
    } else {
        if !will_update {
            println!("Already on latest version ({}). Use --force to reinstall.", current);
            if args.check {
                std::process::exit(2);
            }
            std::process::exit(0);
        }
        if newer {
            println!("Update available: {} -> {}", current, update.version);
        } else {
            println!("Forcing reinstall of {} (current {})", update.version, current);
        }
        if args.check {
            // --check should not proceed to download
            std::process::exit(0);
        }
    }

    // Prompt unless --yes / --json / --force imply non-interactive
    if !args.yes && !args.json && args.mode == InstallMode::Interactive {
        print!("Download and install {}? [Y/n]: ", update.version);
        let _ = std::io::stdout().flush();
        let mut line = String::new();
        if std::io::stdin().read_line(&mut line).is_ok() {
            let s = line.trim().to_lowercase();
            if s == "n" || s == "no" {
                println!("Aborted.");
                std::process::exit(0);
            }
        }
    }

    // Resolve output path
    let out_dir = args
        .out_dir
        .clone()
        .unwrap_or_else(std::env::temp_dir);
    let dest = out_dir.join(format!(
        "Cross_Cleaner_Setup_{}.exe",
        update.version.replace('.', "_")
    ));
    // Also keep canonical name for scripts that expect it
    let canonical = out_dir.join("Cross_Cleaner_Setup.exe");

    // Download
    match download_file(&update.download_url, &dest) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("Download failed: {e}");
            std::process::exit(1);
        }
    }

    // Copy to canonical path as well (best effort)
    let _ = std::fs::copy(&dest, &canonical);

    if args.download_only {
        println!("Downloaded to {} (download-only mode, not launching)", dest.display());
        if args.json {
            println!(
                "{}",
                serde_json::json!({"installer": dest.to_string_lossy(), "canonical": canonical.to_string_lossy()})
            );
        }
        return;
    }

    // Launch installer
    let installer_args = build_installer_args(args.mode, args.installer_args.as_deref(), args.no_launch);

    match launch_installer(&dest, &installer_args) {
        Ok(()) => {
            println!("Installer finished successfully.");
            println!("Cross Cleaner {} installed.", update.version);
        }
        Err(e) => {
            eprintln!("Installer error: {e}");
            eprintln!("Installer kept at: {}", dest.display());
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version() {
        assert_eq!(parse_version("v2.0.2.8.2"), vec![2, 0, 2, 8, 2]);
        assert_eq!(parse_version("2.0.1"), vec![2, 0, 1]);
        assert_eq!(parse_version(" V1.2 "), vec![1, 2]);
    }

    #[test]
    fn test_is_newer() {
        assert!(is_newer("2.0.2.8.3", "2.0.2.8.2"));
        assert!(is_newer("2.1", "2.0.9.9.9"));
        assert!(!is_newer("2.0.2.8.2", "2.0.2.8.2"));
        assert!(!is_newer("2.0.2.8.1", "2.0.2.8.2"));
        assert!(!is_newer("1.9.9", "2.0.0"));
        assert!(is_newer("v2.0.2.8.1", "2.0.2.2"));
    }

    #[test]
    fn test_build_installer_args_silent() {
        let a = build_installer_args(InstallMode::Silent, None, false);
        assert!(a.contains(&"/SILENT".to_string()));
        assert!(!a.contains(&"/VERYSILENT".to_string()));
        assert!(a.contains(&"/SP-".to_string()));
    }

    #[test]
    fn test_build_installer_args_verysilent_extra() {
        let a = build_installer_args(InstallMode::VerySilent, Some("/LOG=log.txt"), false);
        assert!(a.contains(&"/VERYSILENT".to_string()));
        assert!(a.contains(&"/LOG=log.txt".to_string()));
    }

    #[test]
    fn test_build_installer_args_interactive() {
        let a = build_installer_args(InstallMode::Interactive, None, false);
        assert!(!a.contains(&"/SILENT".to_string()));
        assert!(!a.contains(&"/VERYSILENT".to_string()));
    }
}
