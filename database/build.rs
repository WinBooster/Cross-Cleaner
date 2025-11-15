use flate2::Compression;
use flate2::write::GzEncoder;
use std::env;
use std::fs;
use std::io::Write;
use std::path::Path;

fn minify_and_compress_json(json: &str) -> Vec<u8> {
    // Minify JSON
    let value: serde_json::Value = serde_json::from_str(json).expect("Failed to parse JSON");
    let minified = serde_json::to_string(&value).expect("Failed to serialize JSON");

    // Compress with gzip
    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder
        .write_all(minified.as_bytes())
        .expect("Failed to compress");
    encoder.finish().expect("Failed to finalize compression")
}

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();

    // Process windows_database.json
    let registry_json = fs::read_to_string("registry_database.json")
        .expect("Failed to read registry_database.json");
    let registry_compressed = minify_and_compress_json(&registry_json);
    let registry_out_path = Path::new(&out_dir).join("registry_database.min.json.gz");

    println!(
        "Registry DB: {} bytes -> {} bytes ({:.1}% reduction)",
        registry_json.len(),
        registry_compressed.len(),
        100.0 - (registry_compressed.len() as f64 / registry_json.len() as f64 * 100.0)
    );

    fs::write(&registry_out_path, &registry_compressed)
        .expect("Failed to write compressed registry database");

    // Process windows_database.json
    let windows_json =
        fs::read_to_string("windows_database.json").expect("Failed to read windows_database.json");
    let windows_compressed = minify_and_compress_json(&windows_json);
    let windows_out_path = Path::new(&out_dir).join("windows_database.min.json.gz");

    println!(
        "Windows DB: {} bytes -> {} bytes ({:.1}% reduction)",
        windows_json.len(),
        windows_compressed.len(),
        100.0 - (windows_compressed.len() as f64 / windows_json.len() as f64 * 100.0)
    );

    fs::write(&windows_out_path, &windows_compressed)
        .expect("Failed to write compressed windows database");

    // Process linux_database.json
    let linux_json =
        fs::read_to_string("linux_database.json").expect("Failed to read linux_database.json");
    let linux_compressed = minify_and_compress_json(&linux_json);
    let linux_out_path = Path::new(&out_dir).join("linux_database.min.json.gz");

    println!(
        "Linux DB: {} bytes -> {} bytes ({:.1}% reduction)",
        linux_json.len(),
        linux_compressed.len(),
        100.0 - (linux_compressed.len() as f64 / linux_json.len() as f64 * 100.0)
    );

    fs::write(&linux_out_path, &linux_compressed)
        .expect("Failed to write compressed linux database");

    // Process macos_database.json
    let macos_json =
        fs::read_to_string("linux_database.json").expect("Failed to read macos_database.json");
    let macos_compressed = minify_and_compress_json(&linux_json);
    let macos_out_path = Path::new(&out_dir).join("macos_database.min.json.gz");

    println!(
        "MacOS DB: {} bytes -> {} bytes ({:.1}% reduction)",
        macos_json.len(),
        macos_compressed.len(),
        100.0 - (macos_compressed.len() as f64 / macos_json.len() as f64 * 100.0)
    );

    fs::write(&macos_out_path, &macos_compressed)
        .expect("Failed to write compressed linux database");

    // Tell Cargo to rerun if the database files change
    println!("cargo:rerun-if-changed=registry_database.json");
    println!("cargo:rerun-if-changed=windows_database.json");
    println!("cargo:rerun-if-changed=linux_database.json");
    println!("cargo:rerun-if-changed=macos_database.json");
}
