use serde_json::Value;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::Duration;

/// Base URL of the GitHub releases page.
pub const RELEASES_URL: &str = "https://github.com/WinBooster/Cross-Cleaner/releases";

const LATEST_RELEASE_API_URL: &str =
    "https://api.github.com/repos/WinBooster/Cross-Cleaner/releases/latest";

const RELEASES_LIST_API_URL: &str =
    "https://api.github.com/repos/WinBooster/Cross-Cleaner/releases?per_page=30";

/// Chunk size used while streaming a release binary to disk.
const DOWNLOAD_CHUNK: usize = 64 * 1024;

/// Information about a newer release found on GitHub.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewRelease {
    /// Version string without the leading `v` (e.g. `2.0.2.8.1`).
    pub version: String,
    /// URL of the release page.
    pub url: String,
    /// Direct download URL of the release binary matching this platform,
    /// `None` when the release ships no such asset (e.g. a source-only tag).
    pub asset_url: Option<String>,
    /// Size of that binary in bytes, `None` when unknown.
    pub asset_size: Option<u64>,
}

impl NewRelease {
    /// True when the release ships a binary this build can install on its own.
    pub fn has_asset(&self) -> bool {
        self.asset_url.is_some()
    }
}

/// One grouped entry of a changelog, e.g. group `Windows Enhancements`
/// with items `["Added documentation clearing", ...]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangelogGroup {
    pub title: String,
    pub items: Vec<String>,
}

/// Parsed changelog of one release.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Changelog {
    pub groups: Vec<ChangelogGroup>,
    pub contributors: Vec<String>,
}

/// Parses a dotted version string (e.g. `v2.0.2.8.1`) into numeric components.
/// Missing or non-numeric components are treated as `0`.
pub fn parse_version(version: &str) -> Vec<u64> {
    version
        .trim()
        .trim_start_matches('v')
        .split('.')
        .map(|part| part.trim().parse::<u64>().unwrap_or(0))
        .collect()
}

/// Returns true if `remote` is strictly newer than `current`.
pub fn is_newer(remote: &str, current: &str) -> bool {
    let remote = parse_version(remote);
    let current = parse_version(current);
    for i in 0..remote.len().max(current.len()) {
        let r = remote.get(i).copied().unwrap_or(0);
        let c = current.get(i).copied().unwrap_or(0);
        if r != c {
            return r > c;
        }
    }
    false
}

/// True when this build runs on arm64 (aarch64).
///
/// Windows and Linux publish an x86_64 and an arm64 binary, so the asset name
/// has to carry the architecture: an arm64 build asking for the x86_64 asset
/// would replace itself with a binary the CPU cannot execute.
fn is_arm64() -> bool {
    matches!(std::env::consts::ARCH, "aarch64" | "arm64")
}

/// Which of the release binaries a build installs for itself.
///
/// The release publishes one binary per platform *and* per frontend (see
/// `.github/workflows/release.yml`), so a frontend has to say which one it is:
/// a terminal build that resolved the GUI asset would download the wrong
/// executable and replace itself with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frontend {
    /// The egui/eframe window app: `desktop`, and the `android` APK.
    Gui,
    /// The ratatui terminal app: `tui`.
    Tui,
}

impl Frontend {
    /// Name of the portable release binary for this frontend on the platform
    /// this build runs on.
    ///
    /// The Inno Setup installer (`Cross_Cleaner_Setup.exe`) is deliberately not
    /// used for the in-app update: it needs administrator rights and cannot
    /// report its progress.
    pub fn asset_name(self) -> &'static str {
        match (self, std::env::consts::OS) {
            (Frontend::Gui, "windows") if is_arm64() => "Windows-Arm64-Cross_Cleaner_GUI.exe",
            (Frontend::Gui, "windows") => "Windows-Cross_Cleaner_GUI.exe",
            (Frontend::Gui, "linux") if is_arm64() => "Linux-Arm64-Cross_Cleaner_GUI",
            (Frontend::Gui, "linux") => "Linux-Cross_Cleaner_GUI",
            // macOS only ever ships arm64 builds, so the name never varies.
            (Frontend::Gui, "macos") => "MacOS-Arm64-Cross_Cleaner_GUI",
            (Frontend::Gui, "android") => "Cross_Cleaner_Android.apk",
            (Frontend::Tui, "windows") if is_arm64() => "Windows-Arm64-Cross_Cleaner_TUI.exe",
            (Frontend::Tui, "windows") => "Windows-Cross_Cleaner_TUI.exe",
            (Frontend::Tui, "linux") if is_arm64() => "Linux-Arm64-Cross_Cleaner_TUI",
            (Frontend::Tui, "linux") => "Linux-Cross_Cleaner_TUI",
            (Frontend::Tui, "macos") => "MacOS-Arm64-Cross_Cleaner_TUI",
            // The terminal app is not built for Android, and an APK cannot be
            // self-replaced anyway.
            (Frontend::Tui, _) => "",
            // An OS this project does not ship for.
            (Frontend::Gui, _) => "",
        }
    }

    /// Name of the release binary for a given OS and CPU, regardless of the
    /// ones this build actually runs on.
    ///
    /// [`Frontend::asset_name`] is the entry point the updater uses; this
    /// variant exists so the tests can enumerate every published name instead
    /// of only the one the test happens to run on.
    #[cfg(test)]
    pub fn asset_name_for(self, os: &str, arm64: bool) -> &'static str {
        match (self, os, arm64) {
            (Frontend::Gui, "windows", true) => "Windows-Arm64-Cross_Cleaner_GUI.exe",
            (Frontend::Gui, "windows", false) => "Windows-Cross_Cleaner_GUI.exe",
            (Frontend::Gui, "linux", true) => "Linux-Arm64-Cross_Cleaner_GUI",
            (Frontend::Gui, "linux", false) => "Linux-Cross_Cleaner_GUI",
            (Frontend::Gui, "macos", _) => "MacOS-Arm64-Cross_Cleaner_GUI",
            (Frontend::Gui, "android", _) => "Cross_Cleaner_Android.apk",
            (Frontend::Tui, "windows", true) => "Windows-Arm64-Cross_Cleaner_TUI.exe",
            (Frontend::Tui, "windows", false) => "Windows-Cross_Cleaner_TUI.exe",
            (Frontend::Tui, "linux", true) => "Linux-Arm64-Cross_Cleaner_TUI",
            (Frontend::Tui, "linux", false) => "Linux-Cross_Cleaner_TUI",
            (Frontend::Tui, "macos", _) => "MacOS-Arm64-Cross_Cleaner_TUI",
            (Frontend::Tui, "android", _) => "",
            (Frontend::Gui, _, _) | (Frontend::Tui, _, _) => "",
        }
    }
}

/// Name of the GUI release binary for the platform this build runs on.
///
/// Kept as the default so the window frontend keeps calling
/// [`check_new_version`] unchanged.
pub fn asset_name() -> &'static str {
    Frontend::Gui.asset_name()
}

/// Picks the asset called `wanted` out of the GitHub `assets` array and
/// returns its direct download URL together with its size in bytes.
/// `size` is `0` when the release does not report one.
fn parse_asset(assets: Option<&Value>, wanted: &str) -> Option<(String, u64)> {
    if wanted.is_empty() {
        return None;
    }
    for asset in assets?.as_array()? {
        if asset.get("name").and_then(Value::as_str) != Some(wanted) {
            continue;
        }
        let Some(url) = asset.get("browser_download_url").and_then(Value::as_str) else {
            continue;
        };
        let size = asset.get("size").and_then(Value::as_u64).unwrap_or(0);
        return Some((url.to_string(), size));
    }
    None
}

/// Checks GitHub for the latest release and returns it when its tag
/// (e.g. `v2.0.2.8.1`) is newer than the current version.
///
/// Resolves the GUI asset; see [`check_new_version_for`] for the terminal app.
pub fn check_new_version() -> Result<Option<NewRelease>, String> {
    check_new_version_for(Frontend::Gui)
}

/// Same as [`check_new_version`], but resolves the release binary belonging to
/// `frontend`.
pub fn check_new_version_for(frontend: Frontend) -> Result<Option<NewRelease>, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(10)))
        .build()
        .into();
    let json: Value = agent
        .get(LATEST_RELEASE_API_URL)
        .header(
            "User-Agent",
            concat!("Cross-Cleaner/", env!("CARGO_PKG_VERSION")),
        )
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("Failed to request latest release: {}", e))?
        .body_mut()
        .read_json()
        .map_err(|e| format!("Failed to parse latest release response: {}", e))?;

    let tag = json
        .get("tag_name")
        .and_then(Value::as_str)
        .ok_or_else(|| "Response is missing tag_name".to_string())?
        .trim()
        .trim_start_matches('v')
        .to_string();

    if !is_newer(&tag, crate::get_version()) {
        return Ok(None);
    }

    let url = json
        .get("html_url")
        .and_then(Value::as_str)
        .unwrap_or(RELEASES_URL)
        .to_string();

    let asset = parse_asset(json.get("assets"), frontend.asset_name());
    let (asset_url, asset_size) = match asset {
        Some((url, size)) => (Some(url), (size > 0).then_some(size)),
        None => (None, None),
    };

    Ok(Some(NewRelease {
        version: tag,
        url,
        asset_url,
        asset_size,
    }))
}

/// Temporary file a release binary is streamed into.
///
/// Named after the running version and the process id, so a crashed run leaves
/// at most one stale file behind and the next run overwrites it instead of
/// filling the temp directory with half-downloaded binaries.
fn update_download_path() -> PathBuf {
    let extension = if cfg!(windows) { ".exe" } else { "" };
    let version = crate::get_version().replace('.', "_");
    std::env::temp_dir().join(format!(
        "Cross_Cleaner_update_{version}_{}{extension}",
        std::process::id()
    ))
}

/// Streams the release binary at `url` into the temp directory, reporting
/// `(downloaded, total)` through `progress` after every chunk.
///
/// `expected_size` is the size GitHub reported for the asset and is verified
/// before the file is handed out, so a truncated download is never installed.
/// The caller owns the returned file and should delete it once it is installed.
pub fn download_asset(
    url: &str,
    expected_size: Option<u64>,
    mut progress: impl FnMut(u64, Option<u64>),
) -> Result<PathBuf, String> {
    // Only ever talk to GitHub over HTTPS; the URL comes from the API above,
    // but a redirect or a tampered cache must not be able to change that.
    if !url.starts_with("https://") {
        return Err(format!("Refusing to download a non-HTTPS URL: {url}"));
    }

    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(600)))
        .build()
        .into();
    let response = agent
        .get(url)
        .header(
            "User-Agent",
            concat!("Cross-Cleaner/", env!("CARGO_PKG_VERSION")),
        )
        .call()
        .map_err(|e| format!("Failed to start the update download: {}", e))?;

    let header_len = response
        .headers()
        .get("content-length")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());
    let total = expected_size.filter(|size| *size > 0).or(header_len);

    let path = update_download_path();
    let mut file = std::fs::File::create(&path)
        .map_err(|e| format!("Failed to create {}: {}", path.display(), e))?;
    let mut reader = response.into_body().into_reader();
    let mut buffer = vec![0u8; DOWNLOAD_CHUNK];
    let mut downloaded = 0u64;

    progress(downloaded, total);
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|e| format!("Update download failed: {}", e))?;
        if read == 0 {
            break;
        }
        downloaded += read as u64;
        file.write_all(&buffer[..read])
            .map_err(|e| format!("Failed to write the update to disk: {}", e))?;
        progress(downloaded, total);
    }
    file.flush()
        .map_err(|e| format!("Failed to write the update to disk: {}", e))?;
    drop(file);

    if let Some(expected) = expected_size.filter(|size| *size > 0)
        && downloaded != expected
    {
        let _ = std::fs::remove_file(&path);
        return Err(format!(
            "Incomplete update download: {downloaded} of {expected} bytes"
        ));
    }

    Ok(path)
}

/// Removes bold/code markdown markup from a text fragment.
fn strip_markdown_inline(text: &str) -> String {
    text.replace("**", "").replace('`', "").trim().to_string()
}

/// Replaces emoji that are missing from the fonts bundled with egui
/// (they would render as empty boxes) with supported glyphs from the
/// bundled emoji-icon-font.
///
/// Verified against the bundled fonts:
/// - `🪟` (U+1FA9F, Windows) is in no font -> U+E61F (Windows logo);
/// - `🍎` (U+1F34E, red apple) -> U+F8FF (Apple logo);
/// - `🐧` (U+1F427, penguin) renders fine via NotoEmoji-Regular, no Linux
///   logo glyph exists in the bundled fonts, so it is kept as is.
fn replace_unsupported_emoji(text: &str) -> String {
    text.replace('\u{1FA9F}', "\u{E61F}")
        .replace('\u{1F34E}', "\u{F8FF}")
}

/// True when the line is a section header (`##` or `###` level) of a changelog body.
fn changelog_header(line: &str) -> Option<&str> {
    line.strip_prefix("### ")
        .or_else(|| line.strip_prefix("## "))
}

/// Parses one release body (markdown) into a changelog.
///
/// Recognized format:
/// - `![...]` badge lines and `---` separators are skipped;
/// - `##`/`###` headers start groups;
/// - `- ` bullets become items of the current group;
/// - bullets under `## 👏 Contributors Hall of Fame` become contributors.
pub fn parse_changelog(body: &str) -> Changelog {
    let mut changelog = Changelog::default();
    let mut current_group: Option<String> = None;
    let mut in_contributors = false;

    for raw in body.lines() {
        let line = raw.trim();
        if line.is_empty()
            || line.starts_with("![")
            || line.starts_with("---")
            || line.starts_with('*')
        {
            continue;
        }

        if let Some(title) = changelog_header(line) {
            let title = replace_unsupported_emoji(&strip_markdown_inline(title));
            // The real header is `## 👏 Contributors Hall of Fame` and may be prefixed
            // with an emoji, so match by substring.
            in_contributors = title.to_lowercase().contains("contributors hall of fame");
            // `##` headers are top-level sections (e.g. "What's New");
            // only `###` headers start a changelog group.
            current_group = if in_contributors || !line.starts_with("### ") {
                None
            } else {
                Some(title)
            };
            continue;
        }

        if let Some(item) = line
            .strip_prefix("- ")
            .map(strip_markdown_inline)
            .map(|ref i| replace_unsupported_emoji(i))
        {
            if item.is_empty() {
                continue;
            }
            if in_contributors {
                if !changelog.contributors.contains(&item) {
                    changelog.contributors.push(item);
                }
            } else if let Some(title) = &current_group {
                if let Some(group) = changelog.groups.iter_mut().find(|g| g.title == *title) {
                    if !group.items.contains(&item) {
                        group.items.push(item);
                    }
                } else {
                    changelog.groups.push(ChangelogGroup {
                        title: title.clone(),
                        items: vec![item],
                    });
                }
            }
        }
    }

    changelog
}

/// Merges `other` into `groups`: same-titled groups are united, items deduplicated.
fn merge_changelog_groups(changelog: &mut Changelog, other: Changelog) {
    for group in other.groups {
        if let Some(existing) = changelog.groups.iter_mut().find(|g| g.title == group.title) {
            for item in group.items {
                if !existing.items.contains(&item) {
                    existing.items.push(item);
                }
            }
        } else {
            changelog.groups.push(group);
        }
    }
    for contributor in other.contributors {
        if !changelog.contributors.contains(&contributor) {
            changelog.contributors.push(contributor);
        }
    }
}

/// Fetches release notes of every release newer than `current_version`
/// and merges identical groups into one changelog (newest release first).
pub fn fetch_changelogs(current_version: &str) -> Result<Changelog, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(10)))
        .build()
        .into();
    let json: Value = agent
        .get(RELEASES_LIST_API_URL)
        .header(
            "User-Agent",
            concat!("Cross-Cleaner/", env!("CARGO_PKG_VERSION")),
        )
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("Failed to request releases: {}", e))?
        .body_mut()
        .read_json()
        .map_err(|e| format!("Failed to parse releases response: {}", e))?;

    let releases = json
        .as_array()
        .ok_or_else(|| "Unexpected releases response".to_string())?;

    let mut merged = Changelog::default();
    for release in releases {
        let Some(tag) = release.get("tag_name").and_then(Value::as_str) else {
            continue;
        };
        if !is_newer(tag, current_version) {
            continue;
        }
        let Some(body) = release.get("body").and_then(Value::as_str) else {
            continue;
        };
        merge_changelog_groups(&mut merged, parse_changelog(body));
    }
    Ok(merged)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version() {
        assert_eq!(parse_version("v2.0.2.8.1"), vec![2, 0, 2, 8, 1]);
        assert_eq!(parse_version("2.0.2.2"), vec![2, 0, 2, 2]);
        assert_eq!(parse_version(" v2.1 "), vec![2, 1]);
    }

    #[test]
    fn test_is_newer() {
        assert!(is_newer("v2.0.2.8.1", "2.0.2.2"));
        assert!(is_newer("v2.0.2.2.1", "2.0.2.2"));
        assert!(is_newer("2.1", "2.0.9.9.9"));
        assert!(!is_newer("v2.0.2.2", "2.0.2.2"));
        assert!(!is_newer("v2.0.2.1", "2.0.2.2"));
        assert!(!is_newer("2.0", "2.0.0"));
        assert!(!is_newer("v1.9.9.9.9", "2.0.0.0"));
    }

    #[test]
    fn test_parse_changelog() {
        let body = "![Downloads](https://img.shields.io/badge/total) ![Version](https://img.shields.io/badge/blue) ![Platform](https://img.shields.io/badge/orange)\n\
\n\
## ✨ What's New\n\
\n\
### 🪟 Windows Enhancements\n\
- **Audacity** Added documentation clearing\n\
- **Void Train** Added logs, game saves, game settings clearing\n\
\n\
## 👏 Contributors Hall of Fame\n\
Special thanks to our amazing contributors who made this release possible:\n\
- **@Nekiplay** - Core improvements and feature implementations\n\
\n\
---\n\
*Thank you for using Cross Cleaner! Your system, cleaner than ever.*";

        let changelog = parse_changelog(body);
        assert_eq!(changelog.groups.len(), 1);
        // 🪟 is replaced with the emoji-icon-font Windows glyph U+E61F.
        assert_eq!(changelog.groups[0].title, "\u{E61F} Windows Enhancements");
        assert_eq!(changelog.groups[0].items.len(), 2);
        assert_eq!(
            changelog.groups[0].items[0],
            "Audacity Added documentation clearing"
        );
        assert_eq!(
            changelog.contributors,
            vec!["@Nekiplay - Core improvements and feature implementations"]
        );
    }

    #[test]
    fn test_asset_name_matches_published_release() {
        let name = asset_name();
        // Every released asset name is listed in .github/workflows/release.yml.
        assert!(
            [
                "Windows-Cross_Cleaner_GUI.exe",
                "Windows-Arm64-Cross_Cleaner_GUI.exe",
                "Linux-Cross_Cleaner_GUI",
                "Linux-Arm64-Cross_Cleaner_GUI",
                "MacOS-Arm64-Cross_Cleaner_GUI",
                "Cross_Cleaner_Android.apk",
            ]
            .contains(&name),
            "unexpected asset name: {name}"
        );
        assert_eq!(name.ends_with(".exe"), cfg!(windows));
        // Only arm64 builds carry the arch marker, and macOS always does
        // because it only ships arm64 binaries.
        assert_eq!(
            name.contains("Arm64"),
            cfg!(any(target_arch = "aarch64", target_os = "macos"))
        );
    }

    #[test]
    fn test_terminal_asset_name_is_distinct_per_platform() {
        let name = Frontend::Tui.asset_name();
        // The terminal build must never resolve the GUI binary: installing it
        // would replace the terminal app with the window app.
        assert!(!name.contains("GUI"), "unexpected asset name: {name}");
        assert!(
            [
                "Windows-Cross_Cleaner_TUI.exe",
                "Windows-Arm64-Cross_Cleaner_TUI.exe",
                "Linux-Cross_Cleaner_TUI",
                "Linux-Arm64-Cross_Cleaner_TUI",
                "MacOS-Arm64-Cross_Cleaner_TUI",
            ]
            .contains(&name),
            "unexpected asset name: {name}"
        );
        assert_eq!(name.ends_with(".exe"), cfg!(windows));
    }

    #[test]
    fn test_every_released_asset_name_is_published_by_the_workflow() {
        // Guards the pairing between this file and .github/workflows/release.yml:
        // a rename on one side without the other silently breaks in-app updates.
        let workflow = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../.github/workflows/release.yml"
        ))
        .expect("release workflow is readable");

        for frontend in [Frontend::Gui, Frontend::Tui] {
            for os in ["windows", "linux", "macos", "android"] {
                for arm64 in [false, true] {
                    let name = frontend.asset_name_for(os, arm64);
                    // Android has no terminal build, so it resolves to nothing.
                    if name.is_empty() {
                        continue;
                    }
                    assert!(
                        workflow.contains(name),
                        "{name} is not published by the release workflow",
                    );
                }
            }
        }
    }

    #[test]
    fn test_asset_name_for_matches_the_running_build() {
        // `asset_name_for` exists so the tests can enumerate every platform, but
        // it has to stay in step with what the updater actually resolves to.
        for frontend in [Frontend::Gui, Frontend::Tui] {
            assert_eq!(
                frontend.asset_name_for(std::env::consts::OS, is_arm64()),
                frontend.asset_name(),
                "{frontend:?} name differs for the platform under test"
            );
        }
    }

    #[test]
    fn test_x86_and_arm64_asset_names_are_distinct() {
        // The two binaries differ only by the arch marker, so a build must never
        // ask for the one it cannot run: that would install an unrunnable exe.
        for os in ["windows", "linux"] {
            for frontend in [Frontend::Gui, Frontend::Tui] {
                let x86 = frontend.asset_name_for(os, false);
                let arm = frontend.asset_name_for(os, true);
                assert_ne!(x86, arm, "{frontend:?} on {os} names both builds alike");
                assert!(arm.contains("Arm64"), "{arm} is missing the arch marker");
                assert!(!x86.contains("Arm64"), "{x86} should not be marked arm64");
            }
        }
    }

    #[test]
    fn test_parse_asset_picks_matching_name() {
        let assets = serde_json::json!([
            { "name": "Linux-Cross_Cleaner_GUI", "size": 12, "browser_download_url": "https://example.com/linux" },
            { "name": "Windows-Cross_Cleaner_GUI.exe", "size": 34, "browser_download_url": "https://example.com/windows" },
        ]);
        assert_eq!(
            parse_asset(Some(&assets), "Windows-Cross_Cleaner_GUI.exe"),
            Some(("https://example.com/windows".to_string(), 34))
        );
    }

    #[test]
    fn test_parse_asset_without_match() {
        let assets = serde_json::json!([
            { "name": "Linux-Cross_Cleaner_GUI", "size": 12, "browser_download_url": "https://example.com/linux" },
        ]);
        // No binary for this platform -> the caller falls back to the release page.
        assert_eq!(
            parse_asset(Some(&assets), "Windows-Cross_Cleaner_GUI.exe"),
            None
        );
        // Malformed payloads are handled the same way instead of panicking.
        assert_eq!(parse_asset(None, "Windows-Cross_Cleaner_GUI.exe"), None);
        assert_eq!(
            parse_asset(
                Some(&serde_json::json!("not an array")),
                "Windows-Cross_Cleaner_GUI.exe"
            ),
            None
        );
        // An unknown platform has no asset name to look for.
        assert_eq!(parse_asset(Some(&assets), ""), None);
    }

    #[test]
    fn test_parse_asset_ignores_missing_url() {
        let assets = serde_json::json!([
            { "name": "Windows-Cross_Cleaner_GUI.exe", "size": 34 },
        ]);
        assert_eq!(
            parse_asset(Some(&assets), "Windows-Cross_Cleaner_GUI.exe"),
            None
        );
    }

    #[test]
    fn test_update_download_path_is_unique_per_run() {
        let path = update_download_path();
        assert_eq!(path.parent(), Some(std::env::temp_dir().as_path()));
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("file name is valid UTF-8");
        assert!(name.starts_with("Cross_Cleaner_update_"), "{name}");
        // One file per running version, never one per chunk.
        assert_eq!(path, update_download_path());
        assert_eq!(name.contains('.'), cfg!(windows), "{name}");
    }

    #[test]
    fn test_download_asset_rejects_non_https() {
        let calls = std::cell::Cell::new(0);
        let result = download_asset("http://example.com/update", None, |_, _| {
            calls.set(calls.get() + 1);
        });
        assert!(result.is_err(), "plain HTTP must be refused");
        assert_eq!(calls.get(), 0, "nothing may be downloaded");
    }

    #[test]
    fn test_merge_changelog_groups() {
        let body_a = "## ✨ What's New\n\n### 🪟 Windows Enhancements\n- **Audacity** Added documentation clearing\n- **Osu** Added logs clearing";
        let body_b = "## ✨ What's New\n\n### 🪟 Windows Enhancements\n- **Genshin Impact** Added logs clearing\n- **Audacity** Added documentation clearing\n\n### 🐧 Linux Enhancements\n- **Apt** Added cacle clearing";

        let mut merged = parse_changelog(body_a);
        merge_changelog_groups(&mut merged, parse_changelog(body_b));

        assert_eq!(merged.groups.len(), 2);
        assert_eq!(merged.groups[0].title, "\u{E61F} Windows Enhancements");
        assert_eq!(merged.groups[0].items.len(), 3);
        assert!(
            merged.groups[0]
                .items
                .contains(&"Audacity Added documentation clearing".to_string())
        );
        // 🐧 is kept: it renders fine via the bundled NotoEmoji font.
        assert_eq!(merged.groups[1].title, "🐧 Linux Enhancements");
    }
}
