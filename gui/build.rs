#[cfg(windows)]
extern crate winres;

#[cfg(windows)]
fn main() {
    use std::env;
    let version_str = env::var("APP_VERSION").unwrap_or_else(|_| "1.0.0".to_string());

    let version_numbers: Vec<u64> = version_str
        .split('.')
        .map(|s| s.parse().unwrap_or(0))
        .collect();

    let version_num = version_numbers.get(0).copied().unwrap_or(0) << 48
        | version_numbers.get(1).copied().unwrap_or(0) << 32
        | version_numbers.get(2).copied().unwrap_or(0) << 16
        | version_numbers.get(3).copied().unwrap_or(0);

    let mut res = winres::WindowsResource::new();
    res.set_icon("../assets\\icon.ico");

    res.set_manifest(
        r#"
    <assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
    <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
        <security>
            <requestedPrivileges>
                <requestedExecutionLevel level="requireAdministrator" uiAccess="false" />
            </requestedPrivileges>
        </security>
    </trustInfo>
    </assembly>
    "#,
    );

    // Hide console window
    res.set("NO_CONSOLE", "1");

    res.set_version_info(winres::VersionInfo::PRODUCTVERSION, version_num)
        .set_version_info(winres::VersionInfo::FILEVERSION, version_num);

    if let Err(e) = res.compile() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

#[cfg(unix)]
fn main() {}
