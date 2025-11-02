use std::env;
use std::fs;
use std::path::Path;

fn main() {
    if let Ok(version) = env::var("APP_VERSION") {
        let cargo_toml_files = ["cli/Cargo.toml", "gui/Cargo.toml", "database/Cargo.toml"];

        for file_path in cargo_toml_files.iter() {
            if let Ok(contents) = fs::read_to_string(file_path) {
                let updated_contents = contents
                    .replace(
                        &format!("version = \"{}\"", "1.0.0"),
                        &format!("version = \"{}\"", version),
                    )
                    .replace(
                        &format!("ProductVersion = \"{}\"", "1.0.0"),
                        &format!("ProductVersion = \"{}\"", version),
                    );

                if contents != updated_contents {
                    fs::write(file_path, updated_contents).expect("Failed to write Cargo.toml");
                    println!("cargo:warning=Updated version in {}", file_path);
                }
            }
        }
    }
}
