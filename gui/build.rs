use flate2::Compression;
use flate2::write::GzEncoder;
use std::env;
use std::fs;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

#[cfg(windows)]
extern crate winres;

/// Compresses a sound from the repo-root `assets/` into `OUT_DIR` so the
/// embedded bytes are gz-compressed inside the binary (same approach as
/// `database/build.rs`).
fn sound_compressor(asset: &str) {
    let out_dir = env::var("OUT_DIR").unwrap();
    let mut bytes: Vec<u8> = vec![];
    File::open(format!("../assets/{}", asset))
        .unwrap_or_else(|e| panic!("Failed to open sound asset {}: {}", asset, e))
        .read_to_end(&mut bytes)
        .unwrap_or_else(|e| panic!("Failed to read sound asset {}: {}", asset, e));

    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder
        .write_all(&*bytes)
        .expect("Failed to compress sound");
    let compressed = encoder
        .finish()
        .expect("Failed to finalize sound compression");

    let out_path = Path::new(&out_dir).join(format!("{}.gz", asset));
    fs::write(&out_path, &compressed).expect("Failed to write compressed sound");

    println!(
        "../assets/{}: {} bytes -> {} bytes ({:.1}% reduction)",
        asset,
        bytes.len(),
        compressed.len(),
        100.0 - (compressed.len() as f64 / bytes.len() as f64 * 100.0)
    );
}

#[cfg(windows)]
fn asset_compressor(asset: &str) {
    let mut bytes: Vec<u8> = vec![];
    File::open(format!("assets/{}", asset))
        .unwrap()
        .read_to_end(&mut bytes)
        .expect("Failed read asset");

    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(&*bytes).expect("Failed to compress");
    let compressed = encoder.finish().expect("Failed to finalize compression");
    std::fs::write(format!("assets/{}{}", asset, ".gz"), compressed).unwrap();
}

fn main() {
    // Sounds from the repo-root assets/ (all platforms).
    for sound in [
        "check.mp3",
        "unckeck.mp3",
        "done.mp3",
        "click.mp3",
        "pop.mp3",
    ] {
        println!("cargo:rerun-if-changed=../assets/{}", sound);
        sound_compressor(sound);
    }

    #[cfg(windows)]
    {
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
        res.set_icon("..\\assets\\icon.ico");

        // Only require admin for release builds, not for tests
        let profile = env::var("PROFILE").unwrap_or_else(|_| String::from("debug"));
        if profile == "release" {
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
        }

        // Hide console window
        res.set("NO_CONSOLE", "1");

        res.set_version_info(winres::VersionInfo::PRODUCTVERSION, version_num)
            .set_version_info(winres::VersionInfo::FILEVERSION, version_num);

        if let Err(e) = res.compile() {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }

        asset_compressor("menu.png")
    }
}
