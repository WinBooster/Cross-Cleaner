use std::ffi::OsStr;
use std::fmt::format;
use std::fs;
use std::io::stdin;
use std::path::Path;
use crossterm::execute;
use glob::{glob, GlobResult, Paths, PatternError};
use inquire::formatter::MultiOptionFormatter;
use inquire::list_option::ListOption;
use inquire::MultiSelect;
use inquire::validator::Validation;
use tabled::{Table, Tabled};

#[derive(PartialEq, Tabled)]
struct Cleared {
    Program: String,

}
impl PartialEq<Option<Cleared>> for &Cleared {
    fn eq(&self, other: &Option<Cleared>) -> bool {
        match other {
            Some(other) => return other.Program.eq(&*self.Program),
            None => return false,
        }
    }
}
struct CleanerData {
    pub path: String,
    pub category: String,
    pub program: String,

    pub files_to_remove: Vec<String>,
    pub folders_to_remove: Vec<String>,
    pub directories_to_remove: Vec<String>,

    pub remove_all_in_dir: bool,
    pub remove_directory_after_clean: bool,
    pub remove_directories: bool,
    pub remove_files: bool
}
fn get_steam_directory() {

}

fn get_file_lenght(path: String) {
    let mut vec = vec![];

    vec.try_reserve_exact(n)?;
    file.by_ref().take(n).read_to_end(&mut vec)?;
}

fn main() {
    execute!(
        std::io::stdout(),
        crossterm::terminal::SetTitle("WinBooster CLI")
    );

    let username = &*whoami::username();
    let mut database: Vec<CleanerData> = Vec::new();

    let mut options: Vec<&str> = vec![

    ];

    //<editor-fold desc="Windows">
    let c_windows_debug_wia = CleanerData {
        path: "C:\\Windows\\debug\\WIA\\*".parse().unwrap(),
        program: "Windows".parse().unwrap(),
        files_to_remove: vec!["wiatrace.log".parse().unwrap()],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: false,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![]
    };
    database.push(c_windows_debug_wia);
    let c_windows_prefetch = CleanerData {
        path: "C:\\Windows\\Prefetch\\*".parse().unwrap(),
        program: "Windows".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![]
    };
    database.push(c_windows_prefetch);
    let c_windows_dumps = CleanerData {
        path: "C:\\Windows\\Minidump\\*".parse().unwrap(),
        program: "Windows".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![]
    };
    database.push(c_windows_dumps);
    let c_windows_security_logs = CleanerData {
        path: "C:\\Windows\\security\\logs\\*.log".parse().unwrap(),
        program: "Windows".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(), remove_directories: false,
        remove_files: true, directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![]
    };
    database.push(c_windows_security_logs);
    let c_windows_security_database_logs = CleanerData {
        path: "C:\\Windows\\security\\database\\*.log".parse().unwrap(),
        program: "Windows".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![]
    };
    database.push(c_windows_security_database_logs);

    let c_temp = CleanerData {
        path: "C:\\Temp\\*".parse().unwrap(),
        program: "Windows".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: true,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![]
    };
    database.push(c_temp);
    let c_windows_panther = CleanerData {
        path: "C:\\Windows\\Panther".parse().unwrap(),
        program: "Windows".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: false, directories_to_remove: vec![],
        remove_all_in_dir: true,
        remove_directory_after_clean: false,
        folders_to_remove: vec![]
    };
    database.push(c_windows_panther);
    let c_windows_temp = CleanerData {
        path: "C:\\Windows\\Temp\\*".parse().unwrap(),
        program: "Windows".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: true,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![]
    };
    database.push(c_windows_temp);
    let c_windows_logs = CleanerData {
        path: "C:\\Windows\\Logs\\*".parse().unwrap(),
        program: "Windows".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: true,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![]
    };
    database.push(c_windows_logs);
    let c_windows_logs_windows_update = CleanerData {
        path: "C:\\Windows\\Logs\\WindowsUpdate\\*".parse().unwrap(),
        program: "Windows".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: true,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![]
    };
    database.push(c_windows_logs_windows_update);
    let c_users_appdata_local_temp = CleanerData {
        path: "C:\\Users\\{username}\\AppData\\Local\\Temp\\*".parse().unwrap(),
        program: "Windows".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: true,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![]
    };
    database.push(c_users_appdata_local_temp);
    let c_programdata_usoshared_logs = CleanerData {
        path: "C:\\ProgramData\\USOShared\\Logs\\*".parse().unwrap(),
        program: "Windows".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: true,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![]
    };
    database.push(c_programdata_usoshared_logs);
    let c_users_appdata_local_connecteddiveces_platform = CleanerData {
        path: "C:\\Users\\".to_owned() + username + "\\AppData\\Local\\ConnectedDevicesPlatform\\*",
        program: "Windows".parse().unwrap(),
        files_to_remove: vec![],
        category: "LastActivity".parse().unwrap(),
        remove_directories: true,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![]
    };
    database.push(c_users_appdata_local_connecteddiveces_platform);
    let c_users_appdata_local_crash_dumps = CleanerData {
        path: "C:\\Users\\".to_owned() + username + "\\AppData\\Local\\CrashDumps\\*",
        program: "Windows".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: true,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![]
    };
    database.push(c_users_appdata_local_crash_dumps);
    let c_users_downloads = CleanerData {
        path: "C:\\Users\\".to_owned() + username + "\\Downloads\\*",
        program: "Windows".parse().unwrap(),
        files_to_remove: vec![],
        category: "Downloads".parse().unwrap(),
        remove_directories: true,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![]
    };
    database.push(c_users_downloads);
    //</editor-fold>
    //<editor-fold desc="NVIDIA Corporation">
    let c_program_files_nvidia_corporation = CleanerData { 
        path: "C:\\Program Files\\NVIDIA Corporation".parse().unwrap(), 
        program: "NVIDIA Corporation".parse().unwrap(), 
        files_to_remove: vec!["license.txt".parse().unwrap()], 
        category: "Logs".parse().unwrap(), 
        remove_directories: false, 
        remove_files: false, 
        directories_to_remove: vec![], 
        remove_all_in_dir: false, 
        remove_directory_after_clean: false, 
        folders_to_remove: vec![]
    };
    database.push(c_program_files_nvidia_corporation);
    let c_program_files_nvidia_corporation_nvsmi = CleanerData { 
        path: "C:\\Program Files\\NVIDIA Corporation\\NVSMI".parse().unwrap(),
        program: "NVIDIA Corporation".parse().unwrap(), 
        files_to_remove: vec!["nvidia-smi.1.pdf".parse().unwrap()], 
        category: "Logs".parse().unwrap(), 
        remove_directories: false, 
        remove_files: false, 
        directories_to_remove: vec![], 
        remove_all_in_dir: false, 
        remove_directory_after_clean: false, 
        folders_to_remove: vec![] 
    };
    database.push(c_program_files_nvidia_corporation_nvsmi);
    //</editor-fold>
    //<editor-fold desc="Java">
    let java_1 = CleanerData {
        path: "C:\\Users\\".to_owned() + username + "\\.jdks\\**",
        program: "Java".parse().unwrap(),
        files_to_remove: vec![
            "javafx-src.zip".parse().unwrap(),
            "src.zip".parse().unwrap()
        ],
        category: "Cache".parse().unwrap(),
        remove_directories: false,
        remove_files: false,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![],
    };
    database.push(java_1);
    let java_2 = CleanerData {
        path: "C:\\Users\\".to_owned() + username + "\\.jdks\\**",
        program: "Java".parse().unwrap(),
        files_to_remove: vec![
            "NOTICE".parse().unwrap(),
            "COPYRIGHT".parse().unwrap(),
            "LICENSE".parse().unwrap(),
            "release".parse().unwrap(),
            "README".parse().unwrap(),
            "ADDITIONAL_LICENSE_INFO".parse().unwrap(),
            "ASSEMBLY_EXCEPTION".parse().unwrap(),
            "Welcome.html".parse().unwrap(),
            "THIRDPARTYLICENSEREADME-JAVAFX.txt".parse().unwrap(),
            "THIRDPARTYLICENSEREADME.txt".parse().unwrap(),
            "README.txt".parse().unwrap(),
            "DISCLAIMER".parse().unwrap()
        ],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: false,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![],
    };
    database.push(java_2);
    let java_5 = CleanerData {
        path: "C:\\Users\\".to_owned() + username + "\\.jdks\\**",
        program: "Java".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: false,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![
            "sample".parse().unwrap(),
            "demo".parse().unwrap()
        ],
    };
    database.push(java_5);
    let java_2 = CleanerData {
        path: "C:\\Program Files\\Java\\**".parse().unwrap(),
        program: "Java".parse().unwrap(),
        files_to_remove: vec![
            "NOTICE".parse().unwrap(),
            "COPYRIGHT".parse().unwrap(),
            "LICENSE".parse().unwrap(),
            "release".parse().unwrap(),
            "README".parse().unwrap(),
            "ADDITIONAL_LICENSE_INFO".parse().unwrap(),
            "ASSEMBLY_EXCEPTION".parse().unwrap(),
            "Welcome.html".parse().unwrap(),
            "THIRDPARTYLICENSEREADME-JAVAFX.txt".parse().unwrap(),
            "THIRDPARTYLICENSEREADME.txt".parse().unwrap(),
            "README.txt".parse().unwrap(),
            "DISCLAIMER".parse().unwrap()
        ],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: false,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![],
    };
    database.push(java_2);
    let java_3 = CleanerData {
        path: "C:\\Program Files\\Eclipse Adoptium\\**".parse().unwrap(),
        program: "Java".parse().unwrap(),
        files_to_remove: vec![
            "NOTICE".parse().unwrap(),
            "COPYRIGHT".parse().unwrap(),
            "LICENSE".parse().unwrap(),
            "release".parse().unwrap(),
            "README".parse().unwrap(),
            "ADDITIONAL_LICENSE_INFO".parse().unwrap(),
            "ASSEMBLY_EXCEPTION".parse().unwrap(),
            "Welcome.html".parse().unwrap(),
            "THIRDPARTYLICENSEREADME-JAVAFX.txt".parse().unwrap(),
            "THIRDPARTYLICENSEREADME.txt".parse().unwrap(),
            "README.txt".parse().unwrap(),
            "DISCLAIMER".parse().unwrap()
        ],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: false,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![],
    };
    database.push(java_3);
    let java_4 = CleanerData {
        path: "C:\\Users\\".to_owned() + username + "\\AppData\\Roaming\\.loliland\\java",
        program: "Java".parse().unwrap(),
        files_to_remove: vec![
            "NOTICE".parse().unwrap(),
            "COPYRIGHT".parse().unwrap(),
            "LICENSE".parse().unwrap(),
            "release".parse().unwrap(),
            "README".parse().unwrap(),
            "ADDITIONAL_LICENSE_INFO".parse().unwrap(),
            "ASSEMBLY_EXCEPTION".parse().unwrap(),
            "Welcome.html".parse().unwrap(),
            "THIRDPARTYLICENSEREADME-JAVAFX.txt".parse().unwrap(),
            "THIRDPARTYLICENSEREADME.txt".parse().unwrap(),
            "README.txt".parse().unwrap(),
            "DISCLAIMER".parse().unwrap()
        ],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: false,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![],
    };
    database.push(java_4);
    //</editor-fold>
    //<editor-fold desc="4uKey for Android">
    let c_program_files_x86_tenorshare_4ukey_for_android_logs = CleanerData {
        path: "C:\\Program Files (x86)\\Tenorshare\\4uKey for Android\\Logs\\*".parse().unwrap(),
        program: "4uKey for Android".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![],
    };
    database.push(c_program_files_x86_tenorshare_4ukey_for_android_logs);
    let c_users_appdata_roaming_tsmonitor_4uker_for_android = CleanerData {
        path: "C:\\Users\\".to_owned() + username +"\\AppData\\Roaming\\TSMonitor\\4uKey for Android\\logs\\*",
        program: "4uKey for Android".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![],
    };
    database.push(c_users_appdata_roaming_tsmonitor_4uker_for_android);
    //</editor-fold>
    //<editor-fold desc="Postman">
    let c_users_appdata_roaming_postman_agent_logs = CleanerData {
        path: "C:\\Users\\".to_owned() + username + "\\AppData\\Roaming\\PostmanAgent\\logs\\*.log",
        program: "Postman".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![],
    };
    database.push(c_users_appdata_roaming_postman_agent_logs);
    let c_users_appdata_local_postman_agent = CleanerData {
        path: "C:\\Users\\".to_owned() + username +"\\AppData\\Local\\Postman-Agent\\*.log",
        program: "4uKey for Android".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![],
    };
    database.push(c_users_appdata_local_postman_agent);
    //</editor-fold>
    //<editor-fold desc="IDA Pro">
    let c_users_appdata_roaming_hex_rays_ida_pro = CleanerData {
        path: "C:\\Users\\".to_owned() + username + "\\AppData\\Roaming\\Hex-Rays\\IDA Pro\\*.lst",
        program: "IDA Pro".parse().unwrap(),
        files_to_remove: vec![],
        category: "Cache".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![],
    };
    database.push(c_users_appdata_roaming_hex_rays_ida_pro);

    //</editor-fold>
    //<editor-fold desc="Xamarin"">
    let c_users_appdata_local_xamarin_logs = CleanerData {
        path: "C:\\Users\\".to_owned() + username + "\\AppData\\Local\\Xamarin\\Logs\\**\\*.log",
        program: "Xamarin".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![],
    };
    database.push(c_users_appdata_local_xamarin_logs);
    //</editor-fold>
    //<editor-fold desc="Windscribe"">
    let c_users_appdata_local_windscribe = CleanerData {
        path: "C:\\Users\\".to_owned() + username + "\\AppData\\Local\\Windscribe\\Windscribe2\\*.txt",
        program: "Windscribe".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![],
    };
    database.push(c_users_appdata_local_windscribe);
    //</editor-fold>
    //<editor-fold desc="GitHub Desktop"">
    let c_users_appdata_roaming_github_desktop = CleanerData {
        path: "C:\\Users\\".to_owned() + username + "\\AppData\\Roaming\\GitHub Desktop\\*.log",
        program: "GitHub Desktop".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![],
    };
    database.push(c_users_appdata_roaming_github_desktop);
    let c_users_appdata_roaming_github_desktop_logs = CleanerData {
        path: "C:\\Users\\".to_owned() + username + "\\AppData\\Roaming\\GitHub Desktop\\logs\\*.log",
        program: "GitHub Desktop".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![],
    };
    database.push(c_users_appdata_roaming_github_desktop_logs);
    //</editor-fold>
    //<editor-fold desc="Panda Security"">
    let c_programdata_panda_security_pslogs = CleanerData {
        path: "C:\\ProgramData\\Panda Security\\PSLogs\\*.log".parse().unwrap(),
        program: "Panda Security".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![],
    };
    database.push(c_programdata_panda_security_pslogs);
    //</editor-fold>
    //<editor-fold desc="NetLimiter"">
    let c_programdata_panda_security_pslogs = CleanerData {
        path: "C:\\ProgramData\\Locktime\\NetLimiter\\**\\logs\\*.log".parse().unwrap(),
        program: "NetLimiter".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![],
    };
    database.push(c_programdata_panda_security_pslogs);
    //</editor-fold>
    //<editor-fold desc="MiniBin"">
    let c_program_files_x86_minibin = CleanerData {
        path: "C:\\Program Files (x86)\\MiniBin\\*.txt".parse().unwrap(),
        program: "MiniBin".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![],
    };
    database.push(c_program_files_x86_minibin);
    //</editor-fold>
    //<editor-fold desc="Brave Browser"">
    let c_program_files_brave_software_brave_browser_application = CleanerData {
        path: "C:\\Program Files\\BraveSoftware\\Brave-Browser\\Application\\*.log".parse().unwrap(),
        program: "Brave Browser".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![],
    };
    database.push(c_program_files_brave_software_brave_browser_application);
    let c_users_appdata_local_brave_software_brave_browser_user_data_default = CleanerData {
        path: "C:\\Users\\".to_owned() + username+ "\\AppData\\Local\\BraveSoftware\\Brave-Browser\\User Data\\Default",
        program: "Brave Browser".parse().unwrap(),
        files_to_remove: vec![
            "Favicons".parse().unwrap(),
            "Favicons-journal".parse().unwrap(),
            "History".parse().unwrap(),
            "History-journal".parse().unwrap(),
            "Visited Links".parse().unwrap()
        ],
        category: "Cache".parse().unwrap(),
        remove_directories: false,
        remove_files: false,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![],
    };
    database.push(c_users_appdata_local_brave_software_brave_browser_user_data_default);
    let c_users_appdata_local_brave_software_brave_browser_user_data_default_dawn_cache = CleanerData {
        path: "C:\\Users\\".to_owned() + username +"\\AppData\\Local\\BraveSoftware\\Brave-Browser\\User Data\\Default\\DawnCache",
        program: "Brave Browser".parse().unwrap(),
        files_to_remove: vec![
            "data_0".parse().unwrap(),
            "data_1".parse().unwrap(),
            "data_2".parse().unwrap(),
            "data_3".parse().unwrap(),
            "index".parse().unwrap()
        ],
        category: "Cache".parse().unwrap(),
        remove_directories: false,
        remove_files: false,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![],
    };
    database.push(c_users_appdata_local_brave_software_brave_browser_user_data_default_dawn_cache);
    let c_users_appdata_local_brave_software_brave_browser_user_data_default_gpu_cache = CleanerData {
        path: "C:\\Users\\".to_owned() + username +"\\AppData\\Local\\BraveSoftware\\Brave-Browser\\User Data\\Default\\GPUCache",
        program: "Brave Browser".parse().unwrap(),
        files_to_remove: vec![
            "data_0".parse().unwrap(),
            "data_1".parse().unwrap(),
            "data_2".parse().unwrap(),
            "data_3".parse().unwrap(),
            "index".parse().unwrap()
        ],
        category: "Cache".parse().unwrap(),
        remove_directories: false,
        remove_files: false,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![],
    };
    database.push(c_users_appdata_local_brave_software_brave_browser_user_data_default_gpu_cache);
    //</editor-fold>
    //<editor-fold desc="Mem Reduct"">
    let c_program_files_brave_software_brave_browser_application = CleanerData {
        path: "C:\\Program Files\\Mem Reduct".parse().unwrap(),
        program: "Mem Reduct".parse().unwrap(),
        files_to_remove: vec![
            "History.txt".parse().unwrap(),
            "License.txt".parse().unwrap(),
            "Readme.txt".parse().unwrap(),
            "memreduct.exe.sig".parse().unwrap()
        ],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: false,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![],
    };
    database.push(c_program_files_brave_software_brave_browser_application);
    //</editor-fold>
    //<editor-fold desc="qBittorrent"">
    let c_program_files_qbittorent = CleanerData {
        path: "C:\\Program Files\\qBittorrent\\*.pdb".parse().unwrap(),
        program: "qBittorrent".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![],
    };
    database.push(c_program_files_qbittorent);
    let c_program_files_qbittorent_logs = CleanerData {
        path: "C:\\Program Files\\qBittorrent\\logs\\*.log".parse().unwrap(),
        program: "qBittorrent".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![],
    };
    database.push(c_program_files_qbittorent_logs);
    //</editor-fold>
    //<editor-fold desc="CCleaner"">
    let c_program_files_ccleaner_logs = CleanerData {
        path: "C:\\Program Files\\CCleaner\\LOG\\*".parse().unwrap(),
        program: "CCleaner".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![],
    };
    database.push(c_program_files_ccleaner_logs);
    //</editor-fold>
    //<editor-fold desc="IObit Malware Fighter"">
    let c_program_files_ccleaner_logs = CleanerData {
        path: "C:\\ProgramData\\IObit\\IObit Malware Fighter\\*.log".parse().unwrap(),
        program: "IObit Malware Fighter".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![],
    };
    database.push(c_program_files_ccleaner_logs);
    let c_program_data_iobit_iobit_malware_finghter_homepage_advisor = CleanerData {
        path: "C:\\ProgramData\\IObit\\IObit Malware Fighter\\Homepage Advisor\\*.log".parse().unwrap(),
        program: "IObit Malware Fighter".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![],
    };
    database.push(c_program_data_iobit_iobit_malware_finghter_homepage_advisor);
    //</editor-fold>
    //<editor-fold desc="IObit Driver Booster"">
    let c_users_appdata_roaming_iobit_driver_booster_logs = CleanerData {
        path: "C:\\Users\\".to_owned() + username + "\\AppData\\Roaming\\IObit\\Driver Booster\\Logs\\*",
        program: "IObit Driver Booster".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: true,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![],
    };
    database.push(c_users_appdata_roaming_iobit_driver_booster_logs);
    let c_program_files_x86_iobit_driver_booster = CleanerData {
        path: "C:\\Program Files (x86)\\IObit\\Driver Booster\\*.log".parse().unwrap(),
        program: "IObit Driver Booster".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![],
    };
    database.push(c_program_files_x86_iobit_driver_booster);
    let c_program_files_x86_iobit_driver_booster_1 = CleanerData {
        path: "C:\\Program Files (x86)\\IObit\\Driver Booster\\*.txt".parse().unwrap(),
        program: "IObit Driver Booster".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![],
    };
    database.push(c_program_files_x86_iobit_driver_booster_1);
    //</editor-fold>
    //<editor-fold desc="Process Lasso"">
    let c_program_data_process_lasso_logs = CleanerData {
        path: "C:\\ProgramData\\ProcessLasso\\logs\\*".parse().unwrap(),
        program: "Process Lasso".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false,
        folders_to_remove: vec![],
    };
    database.push(c_program_data_process_lasso_logs);
    //</editor-fold>

    //<editor-fold desc="Images">

    //<editor-fold desc="ShareX">
    let sharex_1 = CleanerData { path: "C:\\Users\\".to_owned() + username + "\\Documents\\ShareX\\Screenshots\\**\\*.jpg", program: "ShareX".parse().unwrap(), files_to_remove: vec![], category: "Images".parse().unwrap(), remove_directories: false, remove_files: true, directories_to_remove: vec![], remove_all_in_dir: false, remove_directory_after_clean: false, folders_to_remove: vec![] };
    database.push(sharex_1);
    let sharex_2 = CleanerData { path: "C:\\Users\\".to_owned() + username + "\\Documents\\ShareX\\Screenshots\\**\\*.png", program: "ShareX".parse().unwrap(), files_to_remove: vec![], category: "Images".parse().unwrap(), remove_directories: false, remove_files: true, directories_to_remove: vec![], remove_all_in_dir: false, remove_directory_after_clean: false, folders_to_remove: vec![] };
    database.push(sharex_2);
    //</editor-fold>

    //</editor-fold>
    //<editor-fold desc="Cheats">

    //<editor-fold desc="Weave">
    let weave_1 = CleanerData {
        path: "C:\\Weave\\*".parse().unwrap(),
        program: "Weave".parse().unwrap(),
        files_to_remove: vec![],
        category: "Cheats".parse().unwrap(),
        remove_directories: true,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: true,
        folders_to_remove: vec![]
    };
    database.push(weave_1);
    //</editor-fold>

    //</editor-fold>


    for data in database.iter().clone() {
        if !options.contains(&&*data.category) {
            options.push(&*data.category);
        }
    }
    let validator = |a: &[ListOption<&&str>]| {
        if a.len() < 1 {
            return Ok(Validation::Invalid("No category is selected!".into()));
        }
        else {
            return Ok(Validation::Valid);
        }
    };

    let formatter: MultiOptionFormatter<'_, &str> = &|a| format!("{} selected categories", a.len());
    let ans = MultiSelect::new("Select the clearing categories:", options)
        .with_validator(validator)
        .with_formatter(formatter)
        .prompt();

    let mut bytes_cleared = 0;
    let mut removed_files = 0;
    let mut removed_directories = 0;
    let mut cleared_programs:Vec<Cleared> = vec![];


    match ans {
        Ok(ans) => {
            for data in database.iter().clone() {
                let mut working = false;
                if ans.contains(&&*data.category) {
                    let results: Result<Paths, PatternError> = glob(&*data.path);
                    match results {
                        Ok(results) => {
                            for result in results {
                                match result {
                                    Ok(result) => {
                                        let is_dir: bool = result.is_dir();
                                        let is_file: bool = result.is_file();
                                        let path: &str = result.as_path().to_str().unwrap();
                                        let name: Option<&str> = result.file_name().unwrap().to_str();
                                        let lenght = result.metadata().unwrap().len();
                                        //println!("Found: {}", path);
                                        for file in &data.files_to_remove {
                                            let file_path = path.to_owned() + "\\" + &*file;
                                            match fs::remove_file(file_path) {
                                                Ok(_) => {
                                                    working = true;
                                                    removed_files += 1;
                                                    bytes_cleared += lenght;
                                                    //println!("Removed file: {}", name.unwrap());
                                                }
                                                Err(_) => {}
                                            }
                                        }
                                        for directory in &data.directories_to_remove {
                                            let file_path = path.to_owned() + "\\" + &*directory;
                                            match fs::remove_dir_all(file_path) {
                                                Ok(_) => {
                                                    working = true;
                                                    removed_directories += 1;
                                                    bytes_cleared += lenght;
                                                    //println!("Removed file: {}", name.unwrap());
                                                }
                                                Err(_) => {}
                                            }
                                        }

                                        for dir in &data.directories_to_remove {
                                            let dir_path = path.to_owned() + "\\" + &*dir;
                                            match fs::remove_dir(dir_path) {
                                                Ok(_) => {
                                                    removed_directories += 1;
                                                    bytes_cleared += lenght;
                                                    working = true;
                                                    //println!("Removed directory: {}", name.unwrap());
                                                }
                                                Err(_) => {}
                                            }
                                        }

                                        //println!("Found: {}", path);
                                        if data.remove_files && is_file {
                                            match fs::remove_file(path) {
                                                Ok(_) => {
                                                    removed_files += 1;
                                                    bytes_cleared += lenght;
                                                    working = true;
                                                    //println!("Removed file: {}", name.unwrap());
                                                }
                                                Err(_) => {}
                                            }
                                        }
                                        if data.remove_directories && is_dir {
                                            match fs::remove_dir_all(path) {
                                                Ok(_) => {
                                                    removed_directories += 1;
                                                    bytes_cleared += lenght;
                                                    working = true;
                                                    //println!("Removed directory: {}", name.unwrap());
                                                }
                                                Err(_) => {}
                                            }
                                        }
                                        if data.remove_all_in_dir {
                                            let results: Result<Paths, PatternError> = glob(&*(path.to_owned() + "\\*"));

                                            match fs::remove_dir_all(path) {
                                                Ok(_) => {
                                                    working = true;
                                                    bytes_cleared += lenght;
                                                    removed_files += results.unwrap().count();
                                                }
                                                Err(_) => {}
                                            }
                                        }
                                    }
                                    Err(_) => {}
                                }
                            }
                        }
                        Err(_) => {}
                    }
                }
                let mut program = data.program.clone();
                if working {
                    let data = Cleared { Program: program };
                    if !cleared_programs.contains(&data) {
                        cleared_programs.push(data);
                    }
                }
            }
        },
        Err(_) => println!("Can't work with these categories"),
    }
    println!("Cleared programms:");
    let table = Table::new(cleared_programs).to_string();
    println!("{}", table);
    println!("Removed files: {}", removed_files);
    println!("Removed directories: {}", removed_directories);
    let mut s=String::new();
    stdin().read_line(&mut s).expect("Did not enter a correct string");




}
