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
        &fs::read_to_string(input_path).expect(&format!("Failed to read {}", input_path))
    ).expect(&format!("Failed to parse JSON from {}", input_path));

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
        .expect(&format!("Failed to write compressed {}", output_name));
}

fn main() {
    process_database("registry_database.json", "registry_database.min.json.gz");
    process_database("windows_database.json", "windows_database.min.json.gz");
    process_database("linux_database.json", "linux_database.min.json.gz");
    process_database("macos_database.json", "macos_database.min.json.gz");

    println!("cargo:rerun-if-changed=registry_database.json");
    println!("cargo:rerun-if-changed=windows_database.json");
    println!("cargo:rerun-if-changed=linux_database.json");
    println!("cargo:rerun-if-changed=macos_database.json");
}
