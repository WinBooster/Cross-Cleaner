use flate2::Compression;
use flate2::write::GzEncoder;
use std::env;
use std::fs;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

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
    encoder.write_all(&bytes).expect("Failed to compress sound");
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
    encoder.write_all(&bytes).expect("Failed to compress");
    let compressed = encoder.finish().expect("Failed to finalize compression");
    std::fs::write(format!("assets/{}{}", asset, ".gz"), &compressed).unwrap();
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
        for asset in ["menu.png", "settings.png"] {
            println!("cargo:rerun-if-changed=assets/{}", asset);
            asset_compressor(asset);
        }
    }
}
