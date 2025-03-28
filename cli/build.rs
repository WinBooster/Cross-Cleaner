use winres::WindowsResource;

#[cfg(windows)]
fn main() {
    let mut res = WindowsResource::new();
    res.set_icon("src/icon.ico")
        .set("AppId", "com.crosscleaner.cli");

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

    if let Err(e) = res.compile() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

#[cfg(unix)]
fn main() {}
