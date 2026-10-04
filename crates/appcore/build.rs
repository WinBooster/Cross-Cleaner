//! Compresses the UI sounds into `OUT_DIR` so the embedded bytes are
//! gz-compressed inside the binary.
//!
//! The sounds live in `crates/appcore/assets/` because both frontends play them:
//! `gui` through its own buttons and `tui` through its key bindings. Compressing
//! them here — rather than in either frontend — keeps one copy of the data and
//! one definition of the file list.
//!
//! Same approach as `database/build.rs`.

use flate2::Compression;
use flate2::write::GzEncoder;
use std::env;
use std::fs;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

/// Every sound the app can play. `appcore::sounds` embeds exactly these.
const SOUNDS: [&str; 5] = [
    "check.mp3",
    "unckeck.mp3",
    "done.mp3",
    "click.mp3",
    "pop.mp3",
];

fn compress(asset: &str) {
    let out_dir = env::var("OUT_DIR").unwrap();
    let source = format!("assets/{asset}");
    let mut bytes: Vec<u8> = vec![];
    File::open(&source)
        .unwrap_or_else(|e| panic!("Failed to open sound asset {source}: {e}"))
        .read_to_end(&mut bytes)
        .unwrap_or_else(|e| panic!("Failed to read sound asset {source}: {e}"));
    assert!(!bytes.is_empty(), "{source} is empty");

    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(&bytes).expect("Failed to compress sound");
    let compressed = encoder
        .finish()
        .expect("Failed to finalize sound compression");

    let out_path = Path::new(&out_dir).join(format!("{asset}.gz"));
    fs::write(&out_path, &compressed).expect("Failed to write compressed sound");

    println!("cargo:rerun-if-changed={source}");
    println!(
        "  {asset}: {} bytes -> {} bytes ({:.1}% reduction)",
        bytes.len(),
        compressed.len(),
        100.0 - (compressed.len() as f64 / bytes.len() as f64 * 100.0)
    );
}

fn main() {
    for sound in SOUNDS {
        compress(sound);
    }
}
