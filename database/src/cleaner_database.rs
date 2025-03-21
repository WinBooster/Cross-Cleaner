use crate::CleanerData;
use serde_json;
use std::fs;
use std::fs::File;
use std::io::Write;
use crate::registry_utils::get_steam_directory_from_registry;
use disk_name::get_letters;
use lazy_static::lazy_static;

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
