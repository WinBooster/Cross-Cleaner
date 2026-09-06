use serde_json::Value;
use std::time::Duration;

/// Base URL of the GitHub releases page.
pub const RELEASES_URL: &str = "https://github.com/WinBooster/Cross-Cleaner/releases";

const LATEST_RELEASE_API_URL: &str =
    "https://api.github.com/repos/WinBooster/Cross-Cleaner/releases/latest";

const RELEASES_LIST_API_URL: &str =
    "https://api.github.com/repos/WinBooster/Cross-Cleaner/releases?per_page=30";

/// Information about a newer release found on GitHub.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewRelease {
    /// Version string without the leading `v` (e.g. `2.0.2.8.1`).
    pub version: String,
    /// URL of the release page.
    pub url: String,
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

/// Removes bold/code markdown markup from a text fragment.
fn strip_markdown_inline(text: &str) -> String {
    text.replace("**", "").replace('`', "").trim().to_string()
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
            let title = strip_markdown_inline(title);
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

        if let Some(item) = line.strip_prefix("- ").map(strip_markdown_inline) {
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
    let json: Value = ureq::get(RELEASES_LIST_API_URL)
        .timeout(Duration::from_secs(10))
        .set(
            "User-Agent",
            concat!("Cross-Cleaner/", env!("CARGO_PKG_VERSION")),
        )
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("Failed to request releases: {}", e))?
        .into_json()
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
        assert_eq!(changelog.groups[0].title, "🪟 Windows Enhancements");
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
    fn test_merge_changelog_groups() {
        let body_a = "## ✨ What's New\n\n### 🪟 Windows Enhancements\n- **Audacity** Added documentation clearing\n- **Osu** Added logs clearing";
        let body_b = "## ✨ What's New\n\n### 🪟 Windows Enhancements\n- **Genshin Impact** Added logs clearing\n- **Audacity** Added documentation clearing\n\n### 🐧 Linux Enhancements\n- **Apt** Added cacle clearing";

        let mut merged = parse_changelog(body_a);
        merge_changelog_groups(&mut merged, parse_changelog(body_b));

        assert_eq!(merged.groups.len(), 2);
        assert_eq!(merged.groups[0].title, "🪟 Windows Enhancements");
        assert_eq!(merged.groups[0].items.len(), 3);
        assert!(
            !merged.groups[0]
                .items
                .contains(&"Audacity Added documentation clearing".to_string())
                == false
        );
        assert_eq!(merged.groups[1].title, "🐧 Linux Enhancements");
    }
}
