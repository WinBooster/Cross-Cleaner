//! Windows resource setup shared by the frontend build scripts.
//!
//! Every shipped `.exe` needs the same icon and the same version block, so the
//! logic lives here instead of in each `build.rs`. What *differs* between the
//! window app and the terminal app is passed in through [`Options`], because
//! getting it wrong breaks the app in ways that are hard to spot:
//!
//! * [`Options::no_console`] must stay off for the terminal app. It switches the
//!   binary to the GUI subsystem, which detaches it from the terminal it was
//!   launched from — the whole point of that app.
//! * [`Options::require_admin`] matches the window app: the cleaner writes to
//!   system-wide locations, so an unelevated run silently under-cleans.
//!
//! The descriptive fields (`FileDescription`, `ProductName`, …) are not set
//! here: `winres` reads them from `[package.metadata.winres]` of the crate being
//! built, so each frontend keeps its own identity in its own manifest.
//!
//! On non-Windows targets [`apply`] is a no-op, so callers need no `cfg`.

/// What to stamp into a Windows executable.
pub struct Options<'a> {
    /// Path to the `.ico`, relative to the calling crate's manifest directory.
    pub icon: &'a str,
    /// Marks release builds as needing administrator rights.
    pub require_admin: bool,
    /// Switches the binary to the GUI subsystem. Only for window apps.
    pub no_console: bool,
}

/// A release build must be elevated; a debug build has to stay usable without a
/// UAC prompt, so a developer can attach a debugger.
#[cfg(windows)]
fn is_release() -> bool {
    std::env::var("PROFILE").is_ok_and(|profile| profile == "release")
}

/// Packs a dotted version string into the four 16-bit fields Windows expects.
///
/// Missing or unparsable components become `0`, so `2.0.2` and a full
/// `2.0.2.8.1` both produce a valid resource.
#[cfg(windows)]
fn version_number() -> u64 {
    let version = std::env::var("APP_VERSION").unwrap_or_else(|_| "1.0.0".to_string());
    let parts: Vec<u64> = version
        .split('.')
        .map(|part| part.parse().unwrap_or(0))
        .collect();
    let get = |index: usize| parts.get(index).copied().unwrap_or(0);
    (get(0) << 48) | (get(1) << 32) | (get(2) << 16) | get(3)
}

/// The manifest that marks the binary as needing administrator rights.
///
/// It replaces the default manifest wholesale, which is what `winres` does for
/// `set_manifest`; both frontends then behave the same way, so they also drift
/// the same way.
#[cfg(windows)]
const ELEVATE_MANIFEST: &str = r#"
    <assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
    <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
        <security>
            <requestedPrivileges>
                <requestedExecutionLevel level="requireAdministrator" uiAccess="false" />
            </requestedPrivileges>
        </security>
    </trustInfo>
    </assembly>
    "#;

/// Compiles the resources into the calling crate's binary.
///
/// Fails the build on error: a binary without its icon or version block is not
/// the artifact the release workflow expects, and shipping one silently is worse
/// than not shipping it at all.
#[cfg(windows)]
pub fn apply(options: Options<'_>) {
    let mut res = winres::WindowsResource::new();
    res.set_icon(options.icon);

    if options.require_admin && is_release() {
        res.set_manifest(ELEVATE_MANIFEST);
    }
    if options.no_console {
        res.set("NO_CONSOLE", "1");
    }

    let version = version_number();
    res.set_version_info(winres::VersionInfo::PRODUCTVERSION, version)
        .set_version_info(winres::VersionInfo::FILEVERSION, version);

    if let Err(e) = res.compile() {
        eprintln!("Failed to compile Windows resources: {e}");
        std::process::exit(1);
    }
}

/// Non-Windows no-op, so callers do not need their own `cfg`.
#[cfg(not(windows))]
pub fn apply(_options: Options<'_>) {}

#[cfg(test)]
mod tests {
    /// The version packing is platform-independent logic worth pinning down:
    /// the field layout is what Windows reads back in Explorer.
    #[test]
    fn version_number_packs_four_components() {
        fn pack(version: &str) -> u64 {
            let parts: Vec<u64> = version
                .split('.')
                .map(|part| part.parse().unwrap_or(0))
                .collect();
            let get = |index: usize| parts.get(index).copied().unwrap_or(0);
            (get(0) << 48) | (get(1) << 32) | (get(2) << 16) | get(3)
        }
        assert_eq!(pack("2.0.2"), 2 << 48);
        assert_eq!(pack("2.0.2.8"), (2 << 48) | (8 << 16));
        assert_eq!(pack("2.0.2.8.1"), (2 << 48) | (8 << 16) | 1);
        // Unparsable components must not panic: the workflow passes whatever the
        // user typed into the release dispatch.
        assert_eq!(pack("1.x.0"), 1 << 48);
        assert_eq!(pack(""), 0);
    }
}