#[cfg(windows)]
extern crate winres;

#[cfg(windows)]
fn main() {
    let mut res = winres::WindowsResource::new();
    res.set_icon("../assets\\icon.ico");
    res.set("AppId", "com.crosscleaner.cli");

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

    if let Err(e) = res.compile() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

#[cfg(unix)]
fn main() {}
