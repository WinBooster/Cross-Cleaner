use rhai::Engine;

pub fn run_scripts<F>(
    engine: &mut Engine,
    add_cleaner_data: F,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: Fn(rhai::Map) + 'static,
{
    // Register types
    engine.register_type::<database::structures::CleanerData>();
    engine.register_type::<database::structures::CleanerResult>();

    // Register the clear_data function
    engine.register_fn(
        "clear_data",
        |data: database::structures::CleanerData| -> database::structures::CleanerResult {
            database::structures::CleanerResult {
                working: cleaner::clear_data(&data).working,
                path: cleaner::clear_data(&data).path,
                program: cleaner::clear_data(&data).program,
                category: cleaner::clear_data(&data).category,
                bytes: cleaner::clear_data(&data).bytes,
                files: cleaner::clear_data(&data).files,
                folders: cleaner::clear_data(&data).folders,
            }
        },
    );

    // Register function to add custom cleaning data
    engine.register_fn("add_cleaner_data", add_cleaner_data);

    // Register file system functions
    engine.register_fn("delete_file", |path: String| -> i64 {
        if let Ok(metadata) = std::fs::metadata(&path) {
            let size = metadata.len() as i64;
            if std::fs::remove_file(&path).is_ok() {
                size
            } else {
                0
            }
        } else {
            0
        }
    });

    engine.register_fn("get_file_size", |path: String| -> i64 {
        std::fs::metadata(&path)
            .map(|m| m.len() as i64)
            .unwrap_or(0)
    });

    engine.register_fn("delete_directory_recursive", |path: String| -> bool {
        std::fs::remove_dir_all(&path).is_ok()
    });

    // Determine scripts directory path relative to executable
    let scripts_dir = std::env::current_exe()?.parent().unwrap().join("scripts");

    println!("Scripts directory: {}", scripts_dir.display());
    if let Ok(entries) = std::fs::read_dir(&scripts_dir) {
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("rhai") {
                    println!("Loading script: {}", path.display());
                    let script = std::fs::read_to_string(&path)?;
                    engine.run(&script)?;
                }
            }
        }
    } else {
        println!("Failed to read scripts directory");
    }

    Ok(())
}
