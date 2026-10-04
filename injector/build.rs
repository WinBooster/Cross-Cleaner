use std::env;

#[cfg(windows)]
extern crate winres;

/// Version info and manifest for the library.
///
/// The manifest stays `asInvoker`: the library runs *inside* another program, so
/// it inherits whatever rights that program has. The DPI settings are the point
/// here, so the cleaner window is sharp on a scaled display.
fn main() {
    #[cfg(windows)]
    {
        let version_str = env::var("APP_VERSION").unwrap_or_else(|_| "1.0.0".to_string());
        let version_numbers: Vec<u64> = version_str
            .split('.')
            .map(|s| s.parse().unwrap_or(0))
            .collect();
        let version_num = version_numbers.first().copied().unwrap_or(0) << 48
            | version_numbers.get(1).copied().unwrap_or(0) << 32
            | version_numbers.get(2).copied().unwrap_or(0) << 16
            | version_numbers.get(3).copied().unwrap_or(0);

        let mut res = winres::WindowsResource::new();
        res.set_icon("..\\assets\\icon.ico");
        res.set("NO_CONSOLE", "1");
        res.set_manifest(
            r#"
    <assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
    <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
        <security>
            <requestedPrivileges>
                <requestedExecutionLevel level="asInvoker" uiAccess="false" />
            </requestedPrivileges>
        </security>
    </trustInfo>
    <application xmlns="urn:schemas-microsoft-com:asm.v3">
        <windowsSettings>
            <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true/pm</dpiAware>
            <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
        </windowsSettings>
    </application>
    </assembly>
    "#,
        );
        res.set_version_info(winres::VersionInfo::PRODUCTVERSION, version_num)
            .set_version_info(winres::VersionInfo::FILEVERSION, version_num);

        if let Err(e) = res.compile() {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}