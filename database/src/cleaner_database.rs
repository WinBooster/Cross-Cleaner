use crate::CleanerData;
use serde_json;
use std::fs;
use std::fs::File;
use std::io::Write;
use crate::minecraft_launchers_database::{
    get_minecraft_launchers_folders, get_minecraft_launchers_instances_folders,
};
use crate::registry_utils::get_steam_directory_from_registry;
use disk_name::get_letters;
use lazy_static::lazy_static;

fn get_minecraft_database(drive: &str, username: &str) -> Vec<CleanerData> {
    let mut database: Vec<CleanerData> = Vec::new();
    #[cfg(unix)]
    let get_minecraft_launchers_instances_folders =
        get_minecraft_launchers_instances_folders(username);
    #[cfg(windows)]
    let get_minecraft_launchers_instances_folders =
        get_minecraft_launchers_instances_folders(drive, username);
    for instance in get_minecraft_launchers_instances_folders {
        let instance_logs = CleanerData {
            path: instance.0.clone() + "/logs/*",
            program: instance.1.clone(),
            files_to_remove: vec![],
            category: String::from("Logs"),
            remove_directories: true,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(instance_logs);
        let instance_crash_reports = CleanerData {
            path: instance.0.clone() + "/crash-reports/*",
            program: instance.1.clone(),
            files_to_remove: vec![],
            category: String::from("Crash reports"),
            remove_directories: true,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(instance_crash_reports);
        let instance_saves = CleanerData {
            path: instance.0.clone() + "/saves/*",
            program: instance.1.clone(),
            files_to_remove: vec![],
            category: String::from("Game saves"),
            remove_directories: true,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(instance_saves);
        let instance_screenshots = CleanerData {
            path: instance.0.clone() + "/screenshots/*",
            program: instance.1.clone(),
            files_to_remove: vec![],
            category: String::from("Images"),
            remove_directories: true,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(instance_screenshots);
        let instance_cheats = CleanerData {
            path: instance.0.clone(),
            program: instance.1.clone(),
            files_to_remove: vec![],
            category: String::from("Cheats"),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![
                String::from("meteor-client"),
                String::from("LiquidBounce"),
                String::from("Impact"),
                String::from("Wurst"),
                String::from("Nodus"),
                String::from("Aristois"),
            ],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(instance_cheats);
    }

    #[cfg(unix)]
    let get_minecraft_launchers_folders = get_minecraft_launchers_folders(username);
    #[cfg(windows)]
    let get_minecraft_launchers_folders = get_minecraft_launchers_folders(drive, username);
    for folder in get_minecraft_launchers_folders {
        let folder_game_cache = CleanerData {
            path: folder.0.clone() + "/game-cache/*",
            program: folder.1.clone(),
            files_to_remove: vec![],
            category: String::from("Cache"),
            remove_directories: true,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(folder_game_cache);
        let folder_cache = CleanerData {
            path: folder.0.clone() + "/cache/*",
            program: folder.1.clone(),
            files_to_remove: vec![],
            category: String::from("Cache"),
            remove_directories: true,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(folder_cache);
        let folder_licenses = CleanerData {
            path: folder.0.clone() + "/licenses/*",
            program: folder.1.clone(),
            files_to_remove: vec![],
            category: String::from("Logs"),
            remove_directories: true,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(folder_licenses);
        let folder_logs = CleanerData {
            path: folder.0.clone() + "/logs/*",
            program: folder.1.clone(),
            files_to_remove: vec![],
            category: String::from("Logs"),
            remove_directories: true,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(folder_logs);
        let folder_accounts = CleanerData {
            path: folder.0.clone() + "/",
            program: folder.1.clone(),
            files_to_remove: vec![
                String::from("accounts.json"),
                String::from("launcher_accounts.json"),
            ],
            category: String::from("Accounts"),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(folder_accounts);
        let folder_launcher_log_files = CleanerData {
            path: folder.0.clone() + "/*log*",
            program: folder.1.clone(),
            files_to_remove: vec![],
            category: String::from("Logs"),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(folder_launcher_log_files);
    }
    database
}

lazy_static! {
    static ref DATABASE: Vec<CleanerData> = {
        // Определяем путь к JSON-файлу в зависимости от ОС
        let file_path = if cfg!(windows) {
            "windows_database.json"
        } else if cfg!(unix) {
            "linux_database.json"
        } else {
            panic!("Unsupported OS");
        };

        // Чтение JSON-файла
        let data = fs::read_to_string(file_path)
            .expect(&format!("Failed to read {}", file_path));

        // Десериализация JSON в Vec<CleanerData>
        let database: Vec<CleanerData> = serde_json::from_str(&data)
            .expect(&format!("Failed to parse {}", file_path));

        // Получаем имя пользователя
        let username = whoami::username();

        // Получаем список дисков (только для Windows)
        let drives = if cfg!(windows) {
            get_letters()
        } else {
            vec![] // На Linux диски не используются
        };

        // Получаем путь к Steam
        let steam_directory = if cfg!(windows) {
            get_steam_directory_from_registry()
        } else {
            String::new() // На Linux не используются
        };

        // Создаем новую базу данных с заменой плейсхолдеров
        let mut expanded_database = Vec::new();

        for entry in database {
            let mut new_entry = entry.clone();

            // Заменяем {username}
            new_entry.path = new_entry.path.replace("{username}", &username);

            // Заменяем {steam}
            new_entry.path = new_entry.path.replace("{steam}", &steam_directory);

            // Заменяем {drive} (только для Windows)
            if cfg!(windows) && new_entry.path.contains("{drive}") {
                for drive in &drives {
                    let mut drive_entry = new_entry.clone();
                    drive_entry.path = drive_entry.path.replace("{drive}", drive);
                    expanded_database.push(drive_entry);
                }
            } else {
                expanded_database.push(new_entry);
            }
        }

        expanded_database
    };
}

pub fn get_database() -> &'static Vec<CleanerData> {
    &DATABASE
}

pub fn save_database_json() -> Result<String, std::io::Error> {
    let text = serde_json::to_string(&*DATABASE).unwrap();
    let file_path = "database.json";  // файл для записи
    let mut output_file = File::create(file_path)?; // создаем файл
    output_file.write_all(text.as_bytes())?;     // записываем в файл текст
    Ok(text)
}
