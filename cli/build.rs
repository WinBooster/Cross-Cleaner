#[cfg(windows)]
use winres::WindowsResource;

#[cfg(windows)]
fn main() {
    let mut res = WindowsResource::new();
    res.set_icon("../assets\\icon.ico");

    if let Err(e) = res.compile() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

#[cfg(unix)]
fn main() {}
