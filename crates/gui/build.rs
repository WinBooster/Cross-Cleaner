#[cfg(windows)]
use flate2::Compression;
#[cfg(windows)]
use flate2::write::GzEncoder;
#[cfg(windows)]
use std::fs;
#[cfg(windows)]
use std::fs::File;
#[cfg(windows)]
use std::io::{Read, Write};

/// Compresses a menu/settings PNG into a sibling `.gz` so the embedded bytes
/// are gz-compressed inside the binary.
///
/// The UI sounds are *not* handled here: both frontends play them, so they are
/// compressed once by `appcore/build.rs` and reached through
/// `appcore::sounds`.
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
    fs::write(format!("assets/{}{}", asset, ".gz"), &compressed).unwrap();
}

fn main() {
    #[cfg(windows)]
    {
        for asset in ["menu.png", "settings.png"] {
            println!("cargo:rerun-if-changed=assets/{}", asset);
            asset_compressor(asset);
        }
    }
}
