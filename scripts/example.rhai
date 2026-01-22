// Example Rhai script for Cross Cleaner
// This script adds a custom cleaning category for temporary files
let temp_cleaner = #{
    path: "C:\\Temp\\*",
    category: "CustomTemp",
    program: "Custom Temp Cleaner",
    class: "Custom",
    files_to_remove: [],
    directories_to_remove: [],
    remove_all_in_dir: false,
    remove_directory_after_clean: false,
    remove_directories: true,
    remove_files: true
};

add_cleaner_data(temp_cleaner);