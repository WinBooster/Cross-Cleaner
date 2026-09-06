use serde_json::Value;
use std::time::Duration;

/// Base URL of the GitHub releases page.
pub const RELEASES_URL: &str = "https://github.com/WinBooster/Cross-Cleaner/releases";

const LATEST_RELEASE_API_URL: &str =
    "https://api.github.com/repos/WinBooster/Cross-Cleaner/releases/latest";

/// Information about a newer release found on GitHub.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewRelease {
    /// Version string without the leading `v` (e.g. `2.0.2.8.1`).
    pub version: String,
    /// URL of the release page.
    pub url: String,
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

/// Checks GitHub for the latest release and returns it when its tag
/// (e.g. `v2.0.2.8.1`) is newer than the current version.
pub fn check_new_version() -> Result<Option<NewRelease>, String> {
    let json: Value = ureq::get(LATEST_RELEASE_API_URL)
        .timeout(Duration::from_secs(10))
        .set(
            "User-Agent",
            concat!("Cross-Cleaner/", env!("CARGO_PKG_VERSION")),
        )
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("Failed to request latest release: {}", e))?
        .into_json()
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

    Ok(Some(NewRelease { version: tag, url }))
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
}
