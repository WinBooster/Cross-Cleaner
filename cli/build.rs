#[cfg(windows)]
use winres::WindowsResource;

#[cfg(windows)]
fn main() {
    let mut res = WindowsResource::new();
    res.set_icon("../assets\\icon.ico")
        .set("AppId", "{9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3}");

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
