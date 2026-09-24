use flate2::Compression;
use flate2::write::GzEncoder;
use std::env;
use std::fs;
use std::io::Write;
use std::path::Path;

fn minify_and_compress_json(json: &str) -> Vec<u8> {
    let value: serde_json::Value = serde_json::from_str(json).expect("Failed to parse JSON");
    let minified = serde_json::to_string(&value).expect("Failed to serialize JSON");

    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder
        .write_all(minified.as_bytes())
        .expect("Failed to compress");
    encoder.finish().expect("Failed to finalize compression")
}

fn remove_class_fields(value: &mut serde_json::Value) {
    if let Some(array) = value.as_array_mut() {
        for item in array {
            if let Some(obj) = item.as_object_mut() {
                obj.remove("class");
            }
        }
    }
}

fn process_database(input_path: &str, output_name: &str) {
    let out_dir = env::var("OUT_DIR").unwrap();

    let mut json_data: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(input_path).unwrap_or_else(|_| panic!("Failed to read {}", input_path)),
    )
    .unwrap_or_else(|_| panic!("Failed to parse JSON from {}", input_path));

    remove_class_fields(&mut json_data);

    let json_string = serde_json::to_string(&json_data).expect("Failed to serialize JSON");
    let compressed = minify_and_compress_json(&json_string);
    let out_path = Path::new(&out_dir).join(output_name);

    println!(
        "{}: {} bytes -> {} bytes ({:.1}% reduction)",
        input_path,
        json_string.len(),
        compressed.len(),
        100.0 - (compressed.len() as f64 / json_string.len() as f64 * 100.0)
    );

    fs::write(&out_path, &compressed)
        .unwrap_or_else(|_| panic!("Failed to write compressed {}", output_name));
}

fn main() {
    // Generate compressed databases for ALL targets so cross-compilation
    // (cargo apk2 for aarch64-linux-android on windows host) always finds its file.
    // Previous #[cfg(target_os = "...")] on build script checked HOST, not TARGET,
    // so android file was missing when building on windows.
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let is_android = target_os == "android";

    // Always generate every DB — cheap and ensures OUT_DIR contains needed file for any target
    for (src, out) in [
        ("registry_database.json", "registry_database.min.json.gz"),
        ("windows_database.json", "windows_database.min.json.gz"),
        ("linux_database.json", "linux_database.min.json.gz"),
        ("macos_database.json", "macos_database.min.json.gz"),
        ("android_database.json", "android_database.min.json.gz"),
    ] {
        // On host builds avoid panic if some OS json not relevant? All exist now.
        // But generate unconditionally; skip only if target is android and file is registry? No, generate all.
        process_database(src, out);
    }

    // Ensure rerun triggers for any DB change regardless of target
    println!("cargo:rerun-if-changed=registry_database.json");
    println!("cargo:rerun-if-changed=windows_database.json");
    println!("cargo:rerun-if-changed=linux_database.json");
    println!("cargo:rerun-if-changed=macos_database.json");
    println!("cargo:rerun-if-changed=android_database.json");

    // Hint for cross targets
    if is_android {
        println!("cargo:warning=building for android target, all DBs generated");
    }
}
