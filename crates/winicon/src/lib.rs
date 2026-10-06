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

/// The application icon, owned by this crate so both frontends stamp the same
/// one without either of them carrying the file.
///
/// The raw bytes are here for the runtimes that draw the icon as a window icon
/// (eframe on desktop, the activity on Android).
pub const ICON: &[u8] = include_bytes!("../assets/icon.ico");

/// The same file as a path, for `winres`, which takes a filename rather than
/// bytes.
///
/// `CARGO_MANIFEST_DIR` expands here — while *this* crate is compiled — so the
/// value is this crate's own directory no matter which build script ends up
/// calling [`apply`]. That is what lets callers stop passing a relative path
/// around: `desktop/build.rs` and `tui/build.rs` live two directories away and
/// would each have to count the `..` segments on their own.
pub const ICON_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icon.ico");

/// What to stamp into a Windows executable.
#[derive(Debug, Clone, Copy)]
pub struct Options {
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
pub fn apply(options: Options) {
    let mut res = winres::WindowsResource::new();
    res.set_icon(ICON_PATH);

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
pub fn apply(_options: Options) {}

#[cfg(test)]
mod tests {
    use super::*;

    /// The icon is embedded at compile time, so a missing or empty file would
    /// produce a binary with no icon and nothing else would notice.
    #[test]
    fn the_icon_is_embedded() {
        assert!(!ICON.is_empty(), "icon.ico must be embedded");
        // ICO magic: reserved = 0, type = 1 (icon), then the image count.
        assert_eq!(&ICON[..4], &[0x00, 0x00, 0x01, 0x00], "not an ICO file");
    }

    /// `winres` only ever sees a filename, so a path that does not resolve would
    /// fail deep inside the resource compiler with a much less obvious message.
    #[test]
    fn the_icon_path_points_at_the_embedded_icon() {
        assert!(
            ICON_PATH.ends_with("assets/icon.ico"),
            "unexpected icon path: {ICON_PATH}",
        );
        let on_disk = std::fs::read(ICON_PATH).expect("icon.ico must exist on disk");
        assert_eq!(
            on_disk, ICON,
            "the path and the embedded bytes must be the same file",
        );
    }

    /// The Inno Setup script names the icon too, and Inno resolves it relative to
    /// the repository root. When the icon moved into this crate that line was left
    /// pointing at the old location, and the release job failed at the last step
    /// with "The system cannot find the path specified" — after the binaries were
    /// already built and uploaded nowhere.
    #[test]
    fn the_installer_script_points_at_the_icon() {
        let script = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../build_setup.iss"
        ))
        .expect("build_setup.iss is readable");

        let icon_line = script
            .lines()
            .find(|line| line.trim_start().starts_with("SetupIconFile="))
            .expect("the installer script sets SetupIconFile");
        let relative = icon_line
            .trim()
            .trim_start_matches("SetupIconFile=")
            .replace('\\', "/");

        // Relative to the repository root, which is where `iscc` runs from.
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
        let resolved = format!("{root}/{relative}");
        let bytes =
            std::fs::read(&resolved).unwrap_or_else(|e| panic!("{resolved} is unreadable: {e}"));
        assert_eq!(
            bytes, ICON,
            "the installer must ship the same icon the binaries are stamped with",
        );
    }

    /// The script is compiled twice per release, once for x64 and once for arm64
    /// (see `.github/workflows/release.yml`), so the architecture has to be a
    /// parameter rather than a constant baked into the file.
    ///
    /// Both output names have to stay in step with the workflow: the arm64
    /// installer is picked up by name, and `Cross_Cleaner_Setup.exe` is the one
    /// winget publishes, so a rename on either side breaks a release quietly.
    #[test]
    fn the_installer_script_builds_both_architectures() {
        let script = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../build_setup.iss"
        ))
        .expect("build_setup.iss is readable");

        assert!(
            script.contains("/DMyArch=arm64"),
            "the script takes its architecture from the command line"
        );
        assert!(
            script.contains("#ifndef MyArch"),
            "a local build has no /DMyArch and must fall back to a default"
        );
        // Both names have to be present, because which one a build produces is
        // decided by /DMyArch rather than by the file.
        assert!(script.contains("Cross_Cleaner_Setup_Arm64"));
        assert!(script.contains("OutputBaseFilename={#MyOutputBaseFilename}"));

        let workflow = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../.github/workflows/release.yml"
        ))
        .expect("release workflow is readable");
        for name in ["Cross_Cleaner_Setup.exe", "Cross_Cleaner_Setup_Arm64.exe"] {
            assert!(
                workflow.contains(name),
                "{name} is not published by the release workflow",
            );
        }
    }

    /// The version packing is platform-independent logic worth pinning down:
    /// the field layout is what Windows reads back in Explorer.
    ///
    /// Windows keeps the four components as two 16-bit pairs, most significant
    /// first, so `3.2.1.4` is major `3`, minor `2`, build `1`, revision `4` —
    /// not `3.2.0.4`. Getting the shift wrong produces a file that still looks
    /// versioned and is only wrong when read back.
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
        const MAJOR: u64 = 1 << 48;
        const MINOR: u64 = 1 << 32;
        const BUILD: u64 = 1 << 16;
        const REVISION: u64 = 1;

        assert_eq!(
            pack("3.2.1.4"),
            3 * MAJOR | 2 * MINOR | BUILD | 4 * REVISION,
            "all four components",
        );
        // A missing component is zero, so a three-part version is the same
        // version with no revision.
        assert_eq!(pack("3.2.1"), 3 * MAJOR | 2 * MINOR | BUILD);
        assert_eq!(pack("3.2"), 3 * MAJOR | 2 * MINOR);
        // Anything past the fourth is dropped, not shifted into the wrong field.
        assert_eq!(
            pack("3.2.1.4.9"),
            3 * MAJOR | 2 * MINOR | BUILD | 4 * REVISION,
        );
        // Unparsable components must not panic: the workflow passes whatever the
        // user typed into the release dispatch.
        assert_eq!(pack("1.x.0"), MAJOR);
        assert_eq!(pack(""), 0);
    }
}
