//! Opening a URL in the system browser, used by the GitHub / donate buttons
//! and by the "new version available" banner.

/// Refuses anything that is not plain HTTPS: these URLs are handed to a shell
/// on unix and to `ShellExecuteW` on Windows, so a `file://` or `ms-msdt:`
/// style link would be an easy way to run something the user never intended.
fn is_safe(url: &str) -> bool {
    url.starts_with("https://")
}

/// Opens `url` in the system browser. Failures are reported on stderr; the
/// frontend stays usable without a browser.
#[cfg(windows)]
pub fn open_in_browser(url: &str) {
    if !is_safe(url) {
        database::diag::warn(format!("refusing to open non-HTTPS URL: {url}"));
        return;
    }
    // `ShellExecuteW` is reached through `cmd /c start` so this crate stays
    // free of the `windows` crate dependency the GUI pulls in for its own
    // window chrome.
    let _ = std::process::Command::new("cmd")
        .args(["/c", "start", "", url])
        .spawn();
}

#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
))]
pub fn open_in_browser(url: &str) {
    if !is_safe(url) {
        database::diag::warn(format!("refusing to open non-HTTPS URL: {url}"));
        return;
    }
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

#[cfg(target_os = "macos")]
pub fn open_in_browser(url: &str) {
    if !is_safe(url) {
        database::diag::warn(format!("refusing to open non-HTTPS URL: {url}"));
        return;
    }
    let _ = std::process::Command::new("open").arg(url).spawn();
}

#[cfg(not(any(
    target_os = "windows",
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
)))]
pub fn open_in_browser(_url: &str) {}

#[cfg(test)]
mod tests {
    use super::is_safe;

    #[test]
    fn only_https_is_accepted() {
        assert!(is_safe(
            "https://github.com/Cross-Optimizations/Cross-Cleaner"
        ));
        assert!(!is_safe("http://github.com/"));
        assert!(!is_safe("file:///C:/Windows/System32/cmd.exe"));
        assert!(!is_safe("javascript:alert(1)"));
        assert!(!is_safe(""));
    }
}
