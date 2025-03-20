use crate::CleanerData;
use crate::minecraft_launchers_database::{
    get_minecraft_launchers_folders, get_minecraft_launchers_instances_folders,
};
#[cfg(windows)]
use crate::registry_utils::get_steam_directory_from_registry;
#[cfg(windows)]
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

#[cfg(unix)]
lazy_static! {
    static ref DATABASE: Vec<CleanerData> = {
    let mut database: Vec<CleanerData> = Vec::new();
    let username = &*whoami::username();

    //<editor-fold desc="System">
    let home_cache_thumnails_normal = CleanerData {
        path: String::from("/home/".to_owned() + username + "/.cache/thumbnails/normal/*"),
        program: "System".parse().unwrap(),
        files_to_remove: vec![],
        category: "Cache".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(home_cache_thumnails_normal);
    let home_cache_thumnails_large = CleanerData {
        path: String::from("/home/".to_owned() + username + "/.cache/thumbnails/large/*"),
        program: "System".parse().unwrap(),
        files_to_remove: vec![],
        category: "Cache".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(home_cache_thumnails_large);
    let home_local_share_trash_files = CleanerData {
        path: String::from("/home/".to_owned() + username + "/.local/share/Trash/files/*"),
        program: "System".parse().unwrap(),
        files_to_remove: vec![],
        category: "Trash".parse().unwrap(),
        remove_directories: true,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(home_local_share_trash_files);
    //</editor-fold>
    //<editor-fold desc="JetBrains">
    let home_cache_librewolf_thumnails = CleanerData {
        path: String::from("/home/".to_owned() + username + "/.cache/JetBrains/**/log/*"),
        program: "JetBrains".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: true,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(home_cache_librewolf_thumnails);
    //</editor-fold>
    //<editor-fold desc="GitHub Desktop">
    let home_config_github_desktop_logs = CleanerData {
        path: String::from("/home/".to_owned() + username + "/.config/GitHub Desktop/logs/*"),
        program: "GitHub Desktop".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(home_config_github_desktop_logs);
    let home_config_github_desktop_logs = CleanerData {
        path: String::from("/home/".to_owned() + username + "/.config/GitHub Desktop/Cache/*"),
        program: "GitHub Desktop".parse().unwrap(),
        files_to_remove: vec![],
        category: "Cache".parse().unwrap(),
        remove_directories: true,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(home_config_github_desktop_logs);
    //</editor-fold>
    //<editor-fold desc="Steam">
    let homo_local_share_steam_logs = CleanerData {
        path: String::from("/home/".to_owned() + username + "/.local/share/Steam/logs/*"),
        program: "Steam".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(homo_local_share_steam_logs);
    let homo_local_share_steam_logs = CleanerData {
        path: String::from("/home/".to_owned() + username + "/..local/share/Steam/userdata/*"),
        program: "Steam".parse().unwrap(),
        files_to_remove: vec![],
        category: "Accounts".parse().unwrap(),
        remove_directories: true,
        remove_files: false,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(homo_local_share_steam_logs);
    let homo_local_share_steam_logs = CleanerData {
        path: String::from("/home/".to_owned() + username + "/.local/share/Steam/steamapps/common/Proton Hotfix"),
        program: "Steam proton".parse().unwrap(),
        files_to_remove: vec![
            String::from("LICENSE"),
            String::from("LICENSE.OFL")
        ],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: false,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(homo_local_share_steam_logs);
    //</editor-fold>
    //<editor-fold desc="Ghidra">
    let home_ghidra_logs = CleanerData {
        path: String::from("/home/".to_owned() + username + "/.ghidra/**/*.log"),
        program: "Ghidra".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(home_ghidra_logs);
    //</editor-fold>
    //<editor-fold desc="Thunderbird">
    let home_ghidra_logs = CleanerData {
        path: String::from("/home/".to_owned() + username + "/.cache/thunderbird/**/cache2/*.log"),
        program: "Thunderbird".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(home_ghidra_logs);
    //</editor-fold>
    //<editor-fold desc="Exodus Crypto Wallet">
    let home_config_exodus_wallet = CleanerData {
        path: String::from("/home/".to_owned() + username + "/.config/Exodus"),
        program: "Exodus Crypto Wallet".parse().unwrap(),
        files_to_remove: vec![],
        category: "Accounts".parse().unwrap(),
        remove_directories: false,
        remove_files: false,
        directories_to_remove: vec![
            String::from("exodus.wallet")
        ],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(home_config_exodus_wallet);
    //</editor-fold>

    //<editor-fold desc="Docs">

    //<editor-fold desc="Documents">
    let usr_share_doc_documentation = CleanerData {
        path: String::from("/usr/share/doc/"),
        program: String::from("Documentation"),
        files_to_remove: vec![],
        category: String::from("Documentation"),
        remove_directories: false,
        remove_files: false,
        directories_to_remove: vec![
            String::from("zstd"),
            String::from("zmq"),
            String::from("zix"),
            String::from("zimg"),
            String::from("zbar"),
            String::from("xz"),
            String::from("xorgproto"),
            String::from("xmlsec1"),
            String::from("xfsprogs"),
            String::from("xapian-core"),
            String::from("wireplumber"),
            String::from("wiper"),
            String::from("wavpack"),
            String::from("vpnc"),
            String::from("vlc"),
            String::from("v4l-utils"),
            String::from("userspace-rcu"),
            String::from("usbutils"),
            String::from("twolame"),
            String::from("tor"),
            String::from("thunar"),
            String::from("systemd"),
            String::from("sudo"),
            String::from("stoken"),
            String::from("sratom"),
            String::from("sratom"),
            String::from("speexdsp"),
            String::from("speex"),
            String::from("sntp"),
            String::from("smartmontools"),
            String::from("slsh"),
            String::from("slang"),
            String::from("serd"),
            String::from("rubberband"),
            String::from("rnnoise"),
            String::from("ripgrep"),
            String::from("procps-ng"),
            String::from("portaudio"),
            String::from("oniguruma"),
            String::from("npth"),
            String::from("ngtcp2"),
            String::from("nftables"),
            String::from("nfs-utils"),
            String::from("NetworkManager"),
            String::from("neon-0.34.0"),
            String::from("nano"),
            String::from("namcap"),
            String::from("mujs"),
            String::from("mtools"),
            String::from("mpv"),
            String::from("mpfr"),
            String::from("mpdecimal"),
            String::from("mariadb"),
            String::from("man-db"),
            String::from("lzo"),
            String::from("lv2"),
            String::from("lilv"),
            String::from("libXrender"),
            String::from("libxnvctrl-390xx"),
            String::from("libxcb"),
            String::from("libuv"),
            String::from("libusb"),
            String::from("libupnp"),
            String::from("libunistring"),
            String::from("libtiff"),
            String::from("libtheora-1.1.1"),
            String::from("libthai"),
            String::from("libsndfile"),
            String::from("libshout"),
            String::from("libseccomp"),
            String::from("libsamplerate"),
            String::from("libplacebo"),
            String::from("libopenmpt"),
            String::from("libogg"),
            String::from("libnsl"),
            String::from("libnet"),
            String::from("libmtp-1.1.22"),
            String::from("libltc"),
            String::from("liblrdf"),
            String::from("liblc3"),
            String::from("libjpeg-turbo"),
            String::from("libinstpatch"),
            String::from("libimobiledevice"),
            String::from("libgusb"),
            String::from("libgphoto2_port"),
            String::from("libgphoto2"),
            String::from("libffi"),
            String::from("libexif"),
            String::from("libelf"),
            String::from("libedit"),
            String::from("libdvdread"),
            String::from("libdvdnav"),
            String::from("libdc1394"),
            String::from("libdaemon"),
            String::from("libcap"),
            String::from("libcaca-dev"),
            String::from("libbpf"),
            String::from("libavtp"),
            String::from("accountsservice"),
            String::from("acl"),
            String::from("appstream"),
            String::from("aribb24"),
            String::from("attr"),
            String::from("audit"),
            String::from("bash"),
            String::from("bison"),
            String::from("blueman"),
            String::from("bluez"),
            String::from("c-ares"),
            String::from("chromaprint"),
            String::from("chrpath"),
            String::from("cmake"),
            String::from("crypt-setup"),
            String::from("datrie"),
            String::from("dav1d"),
            String::from("ding-libs"),
            String::from("dmidecode"),
            String::from("dosfstools"),
            String::from("ECM"),
            String::from("efibootmgr"),
            String::from("efitools"),
            String::from("efivar"),
            String::from("elfutils"),
            String::from("enchant"),
            String::from("expat"),
            String::from("faac"),
            String::from("faad2"),
            String::from("fakeroot"),
            String::from("ffmpeg"),
            String::from("fftw"),
            String::from("flex"),
            String::from("fluidsynth"),
            String::from("fontconfig"),
            String::from("gc"),
            String::from("gdbm"),
            String::from("glances"),
            String::from("gnupg"),
            String::from("gnutls"),
            String::from("grepftools"),
            String::from("gprofng"),
            String::from("gsmartcontrol"),
            String::from("gtest"),
            String::from("ImageMagick-7"),
            String::from("iwd"),
            String::from("jasper"),
            String::from("jemalloc"),
            String::from("jsoncpp"),
            String::from("krb5"),
            String::from("lame"),
            String::from("libpcap"),
            String::from("nfc-utils"),
            String::from("libasyncns"),
            String::from("libatasmart"),
            String::from("libassuan"),
            String::from("libaio"),
            String::from("jq"),
            String::from("gperftools"),
            String::from("cryptsetup"),
            String::from("sord"),
            String::from("readline"),
            String::from("raptor"),
            String::from("pv"),
            String::from("pkgconf"),
            String::from("pkcs11-helper"),
            String::from("pavucontrol"),
            String::from("openjpeg"),
            String::from("OpenEXR"),
            String::from("opencore-amr"),
            String::from("openconnect"),
            String::from("openal"),
            String::from("ocl-icd"),
            String::from("oath-toolkit"),
            String::from("ntp"),
            String::from("ntfs-3g"),
            String::from("nppth"),
            String::from("openvpn"),
            String::from("lua"),
            String::from("Linux-PAM")
        ],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(usr_share_doc_documentation);
    //</editor-fold>
    //<editor-fold desc="Steam">
    let usr_share_doc_steam = CleanerData {
        path: String::from("/usr/share/doc/"),
        program: String::from("Steam"),
        files_to_remove: vec![],
        category: String::from("Documentation"),
        remove_directories: false,
        remove_files: false,
        directories_to_remove: vec![
            String::from("steam")
        ],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(usr_share_doc_steam);
    //</editor-fold>
    //<editor-fold desc="Alsa">
    let usr_share_doc_alsa = CleanerData {
        path: String::from("/usr/share/doc/"),
        program: String::from("Alsa"),
        files_to_remove: vec![],
        category: String::from("Documentation"),
        remove_directories: false,
        remove_files: false,
        directories_to_remove: vec![
            String::from("alsa-lib"),
            String::from("alsa-plugins"),
            String::from("alsa-topology-conf"),
            String::from("alsa-ucm-conf"),
            String::from("alsa-utils")
        ],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(usr_share_doc_alsa);
    //</editor-fold>
    //<editor-fold desc="Github Desktop">
    let usr_share_doc_github_desktop = CleanerData {
        path: String::from("/usr/share/doc/"),
        program: String::from("Github Desktop"),
        files_to_remove: vec![],
        category: String::from("Documentation"),
        remove_directories: false,
        remove_files: false,
        directories_to_remove: vec![
            String::from("github-desktop")
        ],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(usr_share_doc_github_desktop);
    //</editor-fold>
    //<editor-fold desc="Nvidia">
    let usr_share_doc_nvidia = CleanerData {
        path: String::from("/usr/share/doc/"),
        program: String::from("Nvidia"),
        files_to_remove: vec![
            String::from("nvidia-utils")
        ],
        category: String::from("Documentation"),
        remove_directories: false,
        remove_files: false,
        directories_to_remove: vec![
            String::from("nvidia"),
            String::from("nvidia-utils")
        ],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(usr_share_doc_nvidia);
    //</editor-fold>
    //<editor-fold desc="Pcre">
    let usr_share_doc_pcre = CleanerData {
        path: String::from("/usr/share/doc/"),
        program: String::from("Pcre"),
        files_to_remove: vec![],
        category: String::from("Documentation"),
        remove_directories: false,
        remove_files: false,
        directories_to_remove: vec![
            String::from("pcre"),
            String::from("pcre2")
        ],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(usr_share_doc_pcre);
    //</editor-fold>
    //<editor-fold desc="Powdertoy">
    let usr_share_doc_powdertoy_bin = CleanerData {
        path: String::from("/usr/share/doc/"),
        program: String::from("Powdertoy"),
        files_to_remove: vec![],
        category: String::from("Documentation"),
        remove_directories: false,
        remove_files: false,
        directories_to_remove: vec![
            String::from("powdertoy-bin")
        ],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(usr_share_doc_powdertoy_bin);
    //</editor-fold>
    //<editor-fold desc="Python">
    let usr_share_doc_pycurl = CleanerData {
        path: String::from("/usr/share/doc/"),
        program: String::from("Python"),
        files_to_remove: vec![],
        category: String::from("Documentation"),
        remove_directories: false,
        remove_files: false,
        directories_to_remove: vec![
            String::from("pycurl"),
            String::from("python-annotated-types"),
            String::from("python-orjson"),
            String::from("python-pathvalidate"),
            String::from("python-pefile"),
            String::from("python-pydantic-core"),
            String::from("python-pyelftools"),
            String::from("python-yaml")
        ],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(usr_share_doc_pycurl);
    //</editor-fold>
    //<editor-fold desc="KeePassXC">
    let usr_share_doc_keepassxc = CleanerData {
        path: String::from("/usr/share/keepassxc/"),
        program: String::from("KeePassXC"),
        files_to_remove: vec![],
        category: String::from("Documentation"),
        remove_directories: false,
        remove_files: false,
        directories_to_remove: vec![
            String::from("docs")
        ],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(usr_share_doc_keepassxc);
    //</editor-fold>
    //<editor-fold desc="Licenses">
    let usr_share_licenses = CleanerData {
        path: String::from("/usr/share/licenses/*"),
        program: String::from("Licenses"),
        files_to_remove: vec![],
        category: String::from("Documentation"),
        remove_directories: true,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(usr_share_licenses);
    //</editor-fold>

    //</editor-fold>

    //<editor-fold desc="Music clients">

    //<editor-fold desc="Yandex Music">
    let home_config_yandex_music_logs = CleanerData {
        path: String::from("/home/".to_owned() + username + "/.config/yandex-music/logs/*"),
        program: "Yandex Music".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(home_config_yandex_music_logs);
    let home_config_yandex_music_cache = CleanerData {
        path: String::from("/home/".to_owned() + username + "/.config/yandex-music/Cache/*"),
        program: "Yandex Music".parse().unwrap(),
        files_to_remove: vec![],
        category: "Cache".parse().unwrap(),
        remove_directories: true,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(home_config_yandex_music_cache);
    let home_config_yandex_music_code_cache = CleanerData {
        path: String::from("/home/".to_owned() + username + "/.config/yandex-music/Code Cache/*"),
        program: "Yandex Music".parse().unwrap(),
        files_to_remove: vec![],
        category: "Cache".parse().unwrap(),
        remove_directories: true,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(home_config_yandex_music_code_cache);
    let home_config_yandex_music_dawn_graphite_cache = CleanerData {
        path: String::from("/home/".to_owned() + username + "/.config/yandex-music/DawnGraphiteCache/*"),
        program: "Yandex Music".parse().unwrap(),
        files_to_remove: vec![],
        category: "Cache".parse().unwrap(),
        remove_directories: true,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(home_config_yandex_music_dawn_graphite_cache);
    let home_config_yandex_music_dawn_web_gpu_cache = CleanerData {
        path: String::from("/home/".to_owned() + username + "/.config/yandex-music/DawnWebGPUCache/*"),
        program: "Yandex Music".parse().unwrap(),
        files_to_remove: vec![],
        category: "Cache".parse().unwrap(),
        remove_directories: true,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(home_config_yandex_music_dawn_web_gpu_cache);
        let home_config_yandex_music_gpu_cache = CleanerData {
            path: String::from("/home/".to_owned() + username + "/.config/yandex-music/GPUCache/*"),
            program: "Yandex Music".parse().unwrap(),
            files_to_remove: vec![],
            category: "Cache".parse().unwrap(),
            remove_directories: true,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(home_config_yandex_music_gpu_cache);
        //</editor-fold>
        //<editor-fold desc="Cassettle">
        let home_cache_cassettle = CleanerData {
            path: String::from("/home/".to_owned() + username + "/.cache/cassette/*.log"),
            program: String::from("Cassettle"),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(home_cache_cassettle);
        let home_cache_cassettle = CleanerData {
            path: String::from("/home/".to_owned() + username + "/.cache/audios/*"),
            program: String::from("Cassettle"),
            files_to_remove: vec![],
            category: "Cache".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(home_cache_cassettle);
    //</editor-fold>
    //<editor-fold desc="Spotify">
    let home_cache_spotify = CleanerData {
        path: String::from("/home/".to_owned() + username + "/.cache/spotify/*.log"),
        program: String::from("Spotify"),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(home_cache_spotify);
    //</editor-fold>

    //</editor-fold>

    //<editor-fold desc="Messangers">

    //<editor-fold desc="Discord">
    let home_config_discord_logs = CleanerData {
        path: String::from("/home/".to_owned() + username + "/.config/discord/logs/*"),
        program: "Discord".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(home_config_discord_logs);
    let home_config_discord_cache = CleanerData {
        path: String::from("/home/".to_owned() + username + "/.config/discord/Cache/*"),
        program: "Discord".parse().unwrap(),
        files_to_remove: vec![],
        category: "Cache".parse().unwrap(),
        remove_directories: true,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(home_config_discord_cache);
    let home_config_discord_code_cache = CleanerData {
        path: String::from("/home/".to_owned() + username + "/.config/discord/Code Cache/*"),
        program: "Discord".parse().unwrap(),
        files_to_remove: vec![],
        category: "Cache".parse().unwrap(),
        remove_directories: true,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(home_config_discord_code_cache);
    let home_config_discord_dawn_graphite_cache = CleanerData {
        path: String::from("/home/".to_owned() + username + "/.config/discord/DawnGraphiteCache/*"),
        program: "Discord".parse().unwrap(),
        files_to_remove: vec![],
        category: "Cache".parse().unwrap(),
        remove_directories: true,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(home_config_discord_dawn_graphite_cache);
    let home_config_discord_dawn_web_gpu_cache = CleanerData {
        path: String::from("/home/".to_owned() + username + "/.config/discord/DawnWebGPUCache/*"),
        program: "Discord".parse().unwrap(),
        files_to_remove: vec![],
        category: "Cache".parse().unwrap(),
        remove_directories: true,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(home_config_discord_dawn_web_gpu_cache);
    let home_config_discord_gpu_cache = CleanerData {
        path: String::from("/home/".to_owned() + username + "/.config/discord/GPUCache/*"),
        program: "Discord".parse().unwrap(),
        files_to_remove: vec![],
        category: "Cache".parse().unwrap(),
        remove_directories: true,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(home_config_discord_gpu_cache);
    //</editor-fold>
    //<editor-fold desc="Telegram">
    let home_local_share_telegram_desktop = CleanerData {
        path: String::from("/home/".to_owned() + username + "/.local/share/TelegramDesktop/log*.txt"),
        program: "Telegram".parse().unwrap(),
        files_to_remove: vec![],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(home_local_share_telegram_desktop);
    let home_local_share_telegram_desktop = CleanerData {
        path: String::from("/home/".to_owned() + username + "/.local/share/TelegramDesktop/tdata/*"),
        program: "Telegram".parse().unwrap(),
        files_to_remove: vec![],
        category: "Accounts".parse().unwrap(),
        remove_directories: true,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(home_local_share_telegram_desktop);
    //</editor-fold>

    //</editor-fold>

    //<editor-fold desc="Browsers">

    //<editor-fold desc="FireFox">
    let home_cache_firefox_thumbnails = CleanerData {
        path: String::from("/home/".to_owned() + username + "/.cache/firefox/**/thumbnails/*"),
        program: "FireFox".parse().unwrap(),
        files_to_remove: vec![],
        category: "Cache".parse().unwrap(),
        remove_directories: true,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(home_cache_firefox_thumbnails);
    let home_mozila_firefox_cookies = CleanerData {
        path: String::from("/home/".to_owned() + username + "/.mozilla/firefox/**/"),
        program: "FireFox".parse().unwrap(),
        files_to_remove: vec![
            String::from("cookies.sqlite"),
            String::from("cookies.sqlite-wal")
        ],
        category: "Browser cookies".parse().unwrap(),
        remove_directories: false,
        remove_files: false,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(home_mozila_firefox_cookies);
    let home_mozila_firefox_cookies = CleanerData {
        path: String::from("/home/".to_owned() + username + "/.mozilla/firefox/**/"),
        program: "FireFox".parse().unwrap(),
        files_to_remove: vec![
            String::from("formhistory.sqlite"),
            String::from("favicons.sqlite"),
            String::from("favicons.sqlite-wal")
        ],
        category: "LastActivity".parse().unwrap(),
        remove_directories: false,
        remove_files: false,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(home_mozila_firefox_cookies);
    //</editor-fold>
    //<editor-fold desc="LibreWolf">
    let home_cache_librewolf_thumnails = CleanerData {
        path: String::from("/home/".to_owned() + username + "/.cache/librewolf/**/thumbnails/*"),
        program: "LibreWolf".parse().unwrap(),
        files_to_remove: vec![],
        category: "Cache".parse().unwrap(),
        remove_directories: true,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(home_cache_librewolf_thumnails);
    let home_cache_librewolf_thumnails = CleanerData {
        path: String::from("/home/".to_owned() + username + "/.librewolf/**/"),
        program: "LibreWolf".parse().unwrap(),
        files_to_remove: vec![
            String::from("favicons.sqlite"),
            String::from("favicons.sqlite-wal"),
            String::from("formhistory.sqlite")
        ],
        category: "LastActivity".parse().unwrap(),
        remove_directories: false,
        remove_files: false,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(home_cache_librewolf_thumnails);
    //</editor-fold>

    //</editor-fold>

    //<editor-fold desc="Minecraft launchers">

    let mut mc_database = get_minecraft_database("", username);
    database.append(&mut mc_database);

    //</editor-fold>

    //<editor-fold desc="Games">

    //<editor-fold desc="Terraria">
    let home_local_share_terraria_worlds = CleanerData {
        path: String::from("/home/".to_owned() + username + "/.local/share/Terraria/Worlds/*"),
        program: "Terraria".parse().unwrap(),
        files_to_remove: vec![],
        category: "Game saves".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(home_local_share_terraria_worlds);
    let home_local_share_terraria_playes = CleanerData {
        path: String::from("/home/".to_owned() + username + "/.local/share/Terraria/Playes/*"),
        program: "Terraria".parse().unwrap(),
        files_to_remove: vec![],
        category: "Game saves".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(home_local_share_terraria_playes);
    let home_local_share_steam_steam_apps_common_terraria = CleanerData {
        path: String::from("/home/".to_owned() + username + "/.local/share/Steam/steamapps/common/Terraria/"),
        program: "Terraria".parse().unwrap(),
        files_to_remove: vec![
            String::from("changelog.txt")
        ],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: false,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(home_local_share_steam_steam_apps_common_terraria);
    let home_local_share_steam_steam_apps_common_terraria = CleanerData {
        path: String::from("/home/".to_owned() + username + "/.local/share/Steam/steamapps/common/Terraria/*.blob"),
        program: "Terraria".parse().unwrap(),
        files_to_remove: vec![],
        category: "Crash reports".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(home_local_share_steam_steam_apps_common_terraria);
    //</editor-fold>
    //<editor-fold desc="Garry's mod">
    let home_local_share_steam_steam_apps_common_garrys_mod = CleanerData {
        path: String::from("/home/".to_owned() + username + "/.local/share/Steam/steamapps/common/GarrysMod/"),
        program: "Garry's mod".parse().unwrap(),
        files_to_remove: vec![
            String::from("chromium.log")
        ],
        category: "Logs".parse().unwrap(),
        remove_directories: false,
        remove_files: false,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(home_local_share_steam_steam_apps_common_garrys_mod);
    let home_local_share_steam_steam_apps_common_garrys_mod = CleanerData {
        path: String::from("/home/".to_owned() + username + "/.local/share/Steam/steamapps/common/GarrysMod/crashes/*"),
        program: "Garry's mod".parse().unwrap(),
        files_to_remove: vec![],
        category: "Crash reports".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(home_local_share_steam_steam_apps_common_garrys_mod);
    //</editor-fold>

    //</editor-fold>

    //<editor-fold desc="Cheats">

    //<editor-fold desc="Future Client">
    let home_local_share_prism_launcher_logs = CleanerData {
        path: String::from("/home/".to_owned() + username + "/"),
        program: "Future Client".parse().unwrap(),
        files_to_remove: vec![],
        category: "Cheats".parse().unwrap(),
        remove_directories: true,
        remove_files: true,
        directories_to_remove: vec![
            String::from("Future")
        ],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(home_local_share_prism_launcher_logs);
    //</editor-fold>

    //</editor-fold>

    database.sort_by(|a, b| a.category.cmp(&b.category));

    database
};
}
#[cfg(windows)]
lazy_static! {
    static ref DATABASE: Vec<CleanerData> = {
    let mut database: Vec<CleanerData> = Vec::new();
    let username = &*whoami::username();

    let steam_directory: String = get_steam_directory_from_registry();

    //<editor-fold desc="Windows">
    let drives = get_letters();
    for drive in drives {

        //<editor-fold desc="Windows">
        let c_windows_debug_wia = CleanerData {
            path: drive.to_owned() + "Windows\\debug\\*",
            program: "Windows".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: true,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(c_windows_debug_wia);
        let c_windows_prefetch = CleanerData {
            path: drive.to_owned() + "Windows\\Prefetch\\*",
            program: "Windows".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(c_windows_prefetch);
        let c_windows_dumps = CleanerData {
            path: drive.to_owned() + "Windows\\Minidump\\*",
            program: "Windows".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(c_windows_dumps);
        let c_windows_security_logs = CleanerData {
            path: drive.to_owned() + "Windows\\security\\logs\\*.log",
            program: "Windows".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(), remove_directories: false,
            remove_files: true, directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(c_windows_security_logs);
        let c_windows_security_database_logs = CleanerData {
            path: drive.to_owned() + "Windows\\security\\database\\*.log",
            program: "Windows".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(c_windows_security_database_logs);
        let c_windows_logs = CleanerData {
            path: drive.to_owned() + "Windows\\Logs\\**\\*",
            program: "Windows".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(c_windows_logs);
        let c_windows_logs = CleanerData {
            path: String::from(drive.clone() + "Windows\\*.log"),
            program: String::from("Windows"),
            files_to_remove: vec![],
            category: String::from("Logs"),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(c_windows_logs);
        let c_temp = CleanerData {
            path: drive.to_owned() + "Temp\\*",
            program: "Windows".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: true,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(c_temp);
        let c_windows_panther = CleanerData {
            path: drive.to_owned() + "Windows\\Panther",
            program: "Windows".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false, directories_to_remove: vec![],
            remove_all_in_dir: true,
            remove_directory_after_clean: false
        };
        database.push(c_windows_panther);
        let c_windows_temp = CleanerData {
            path: drive.to_owned() + "Windows\\Temp\\*",
            program: "Windows".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: true,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(c_windows_temp);
        let c_windows_logs = CleanerData {
            path: drive.to_owned() + "Windows\\Logs\\*",
            program: "Windows".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: true,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(c_windows_logs);
        let c_windows_logs_windows_update = CleanerData {
            path: drive.to_owned() + "Windows\\Logs\\WindowsUpdate\\*",
            program: "Windows".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: true,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(c_windows_logs_windows_update);
        let c_users_appdata_local_temp = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\Temp\\*",
            program: "Windows".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: true,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(c_users_appdata_local_temp);
        let c_programdata_usoshared_logs = CleanerData {
            path: drive.to_owned() + "ProgramData\\USOShared\\Logs\\*",
            program: "Windows".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: true,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(c_programdata_usoshared_logs);
        let c_users_appdata_local_connecteddiveces_platform = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\ConnectedDevicesPlatform\\*",
            program: "Windows".parse().unwrap(),
            files_to_remove: vec![],
            category: "LastActivity".parse().unwrap(),
            remove_directories: true,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(c_users_appdata_local_connecteddiveces_platform);
        let c_users_appdata_local_crash_dumps = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\CrashDumps\\*",
            program: "Windows".parse().unwrap(),
            files_to_remove: vec![],
            category: "Crash reports".parse().unwrap(),
            remove_directories: true,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(c_users_appdata_local_crash_dumps);
        let c_users_downloads = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\Downloads\\*",
            program: "Windows".parse().unwrap(),
            files_to_remove: vec![],
            category: "Downloads".parse().unwrap(),
            remove_directories: true,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(c_users_downloads);
        //</editor-fold>
        //<editor-fold desc="Windows Defender">
        let program_files_windows_defender = CleanerData {
            path: drive.to_owned() + "Program Files\\Windows Defender",
            program: "Windows Defender".parse().unwrap(),
            files_to_remove: vec![
                String::from("ThirdPartyNotices.txt")
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(program_files_windows_defender);
        let program_files_windows_defender = CleanerData {
            path: drive.to_owned() + "Program Files\\Windows Defender Advanced Threat Protection",
            program: "Windows Defender".parse().unwrap(),
            files_to_remove: vec![
                String::from("ThirdPartyNotice"),
                String::from("SenseAp.ThirdPartyNotice.txt")
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(program_files_windows_defender);
        //</editor-fold>
        //<editor-fold desc="OneDrive">
        let c_program_files_nvidia_corporation = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\OneDrive\\cache\\qmlcache\\*.qmlc",
            program: "OneDrive".parse().unwrap(),
            files_to_remove: vec![],
            category: String::from("Cache"),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(c_program_files_nvidia_corporation);
        //</editor-fold>
        //<editor-fold desc="NVIDIA Corporation">
        let c_program_files_nvidia_corporation = CleanerData {
            path: drive.to_owned() + "Program Files\\NVIDIA Corporation",
            program: "NVIDIA Corporation".parse().unwrap(),
            files_to_remove: vec![
                "license.txt".parse().unwrap(),
                "nvstlink.log".parse().unwrap()
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(c_program_files_nvidia_corporation);
        let c_program_files_nvidia_corporation_nvsmi = CleanerData {
            path: drive.to_owned() + "Program Files\\NVIDIA Corporation\\NVSMI",
            program: "NVIDIA Corporation".parse().unwrap(),
            files_to_remove: vec![
                "nvidia-smi.1.pdf".parse().unwrap()
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(c_program_files_nvidia_corporation_nvsmi);
        let c_program_files_nvidia_corporation_nv_stereo_installer = CleanerData {
            path: drive.to_owned() + "Program Files\\NVIDIA Corporation\\nvStereoInstaller",
            program: "NVIDIA Corporation".parse().unwrap(),
            files_to_remove: vec![
                "nvStInst.log".parse().unwrap(),
                "nvStInst.old".parse().unwrap()
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(c_program_files_nvidia_corporation_nv_stereo_installer);
        let c_program_files_nvidia_corporation_nv_fbs_plugin = CleanerData {
            path: drive.to_owned() + "Program Files\\NVIDIA Corporation\\NvFBCPlugin",
            program: "NVIDIA Corporation".parse().unwrap(),
            files_to_remove: vec![
                "logPluginError.txt".parse().unwrap(),
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(c_program_files_nvidia_corporation_nv_fbs_plugin);
        let c_users_appdata_local_nvidia_corporation_gfn_runtime_sdk = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\NVIDIA Corporation\\GfnRuntimeSdk\\*.log",
            program: "NVIDIA Corporation".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(c_users_appdata_local_nvidia_corporation_gfn_runtime_sdk);
        let program_data_nvidia_corporation_nvstapisvr = CleanerData {
            path: drive.to_owned() + "ProgramData\\NVIDIA Corporation\\nvstapisvr\\*",
            program: "NVIDIA Corporation".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(program_data_nvidia_corporation_nvstapisvr);
        let program_data_nvidia_corporation_nv_stereo_installer = CleanerData {
            path: drive.to_owned() + "ProgramData\\NVIDIA Corporation\\nvStereoInstaller\\*",
            program: "NVIDIA Corporation".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(program_data_nvidia_corporation_nv_stereo_installer);
        let program_data_nvidia_corporation = CleanerData {
            path: drive.to_owned() + "ProgramData\\NVIDIA Corporation\\*.log",
            program: "NVIDIA Corporation".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(program_data_nvidia_corporation);
        //</editor-fold>
        //<editor-fold desc="Java">
        let java_cache = vec![
            "javafx-src.zip".parse().unwrap(),
            "src.zip".parse().unwrap()
        ];
        let java_1 = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\.jdks\\**",
            program: "Java".parse().unwrap(),
            files_to_remove: java_cache.clone(),
            category: String::from("Cache"),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(java_1);
        let java_files = vec![
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
            "DISCLAIMER".parse().unwrap(),
            "CHANGELOG.txt".parse().unwrap(),
            "THIRD_PARTY_README".parse().unwrap(),
            "version.txt".parse().unwrap(),
            "README.md".parse().unwrap()
        ];
        let java_2 = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\.jdks\\**",
            program: "Java".parse().unwrap(),
            files_to_remove: java_files.clone(),
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(java_2);
        let java_5 = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\.jdks\\**",
            program: "Java".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![
                "sample".parse().unwrap(),
                "demo".parse().unwrap()
            ],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(java_5);
        let java_2 = CleanerData {
            path: drive.to_owned() + "Program Files\\Java\\**",
            program: "Java".parse().unwrap(),
            files_to_remove: java_files.clone(),
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(java_2);
        let java_3 = CleanerData {
            path: drive.to_owned() + "Program Files\\Eclipse Adoptium\\**",
            program: "Java".parse().unwrap(),
            files_to_remove: java_files.clone(),
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(java_3);
        let java_4 = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\.loliland\\java",
            program: "Java".parse().unwrap(),
            files_to_remove: java_files.clone(),
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(java_4);
        let java_5 = CleanerData {
            path: drive.to_owned() + "Program Files\\Android\\jdk\\**\\**",
            program: "Java".parse().unwrap(),
            files_to_remove: java_files.clone(),
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(java_5);
        let java_6 = CleanerData {
            path: drive.to_owned() + "Program Files\\Android\\jdk\\**\\**",
            program: "Java".parse().unwrap(),
            files_to_remove: java_cache.clone(),
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(java_6);
        let java_7 = CleanerData {
            path: drive.to_owned() + "Program Files\\Zulu\\**",
            program: "Java".parse().unwrap(),
            files_to_remove: java_files.clone(),
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(java_7);
        let java_8 = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\.lunarclient\\jre\\**\\**",
            program: "Java".parse().unwrap(),
            files_to_remove: java_files.clone(),
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(java_8);
        let java_9 = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\.tecknixsoftware\\tecknixclient\\runtimes\\**",
            program: "Java".parse().unwrap(),
            files_to_remove: java_files.clone(),
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(java_9);
        let java_10 = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\MCSkill\\updates\\**",
            program: "Java".parse().unwrap(),
            files_to_remove: java_files.clone(),
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(java_10);
        let java_11 = CleanerData {
            path: drive.to_owned() + "\\Program Files (x86)\\GribLand\\runtime\\**",
            program: "Java".parse().unwrap(),
            files_to_remove: java_files.clone(),
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(java_11);
        //</editor-fold>
        //<editor-fold desc="4uKey for Android">
        let c_program_files_x86_tenorshare_4ukey_for_android_logs = CleanerData {
            path: drive.to_owned() + "Program Files (x86)\\Tenorshare\\4uKey for Android\\Logs\\*",
            program: "4uKey for Android".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_x86_tenorshare_4ukey_for_android_logs);
        let c_users_appdata_roaming_tsmonitor_4uker_for_android = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\TSMonitor\\4uKey for Android\\logs\\*",
            program: "4uKey for Android".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_roaming_tsmonitor_4uker_for_android);
        //</editor-fold>
        //<editor-fold desc="Postman">
        let c_users_appdata_roaming_postman_agent_logs = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\PostmanAgent\\logs\\*.log",
            program: "Postman".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_roaming_postman_agent_logs);
        let c_users_appdata_local_postman_agent = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\Postman-Agent\\*.log",
            program: "4uKey for Android".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_local_postman_agent);
        //</editor-fold>
        //<editor-fold desc="IDA Pro">
        let c_users_appdata_roaming_hex_rays_ida_pro = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\Hex-Rays\\IDA Pro\\*.lst",
            program: "IDA Pro".parse().unwrap(),
            files_to_remove: vec![],
            category: String::from("Cache"),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_roaming_hex_rays_ida_pro);

        //</editor-fold>
        //<editor-fold desc="Xamarin">
        let c_users_appdata_local_xamarin_logs = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\Xamarin\\Logs\\**\\*.log",
            program: "Xamarin".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_local_xamarin_logs);
        //</editor-fold>
        //<editor-fold desc="Windscribe">
        let c_users_appdata_local_windscribe = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\Windscribe\\Windscribe2\\*.txt",
            program: "Windscribe".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_local_windscribe);
        //</editor-fold>
        //<editor-fold desc="GitHub Desktop">
        let c_users_appdata_roaming_github_desktop = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\GitHub Desktop\\*.log",
            program: "GitHub Desktop".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_roaming_github_desktop);
        let c_users_appdata_roaming_github_desktop_logs = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\GitHub Desktop\\logs\\*.log",
            program: "GitHub Desktop".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_roaming_github_desktop_logs);
        let c_users_appdata_roaming_github_desktop_logs2 = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\GitHubDesktop\\**\\*.log",
            program: "GitHub Desktop".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_roaming_github_desktop_logs2);
        //</editor-fold>
        //<editor-fold desc="Panda Security">
        let c_programdata_panda_security_pslogs = CleanerData {
            path: drive.to_owned() + "ProgramData\\Panda Security\\PSLogs\\*.log",
            program: "Panda Security".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_programdata_panda_security_pslogs);
        //</editor-fold>
        //<editor-fold desc="NetLimiter">
        let c_programdata_panda_security_pslogs = CleanerData {
            path: drive.to_owned() + "ProgramData\\Locktime\\NetLimiter\\**\\logs\\*.log",
            program: "NetLimiter".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_programdata_panda_security_pslogs);
        //</editor-fold>
        //<editor-fold desc="MiniBin">
        let c_program_files_x86_minibin = CleanerData {
            path: drive.to_owned() + "Program Files (x86)\\MiniBin\\*.txt",
            program: "MiniBin".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_x86_minibin);
        //</editor-fold>
        //<editor-fold desc="Mem Reduct">
        let c_program_files_brave_software_brave_browser_application = CleanerData {
            path: drive.to_owned() + "Program Files\\Mem Reduct",
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
        };
        database.push(c_program_files_brave_software_brave_browser_application);
        //</editor-fold>
        //<editor-fold desc="qBittorrent">
        let c_program_files_qbittorent = CleanerData {
            path: drive.to_owned() + "Program Files\\qBittorrent\\*.pdb",
            program: "qBittorrent".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_qbittorent);
        let c_program_files_qbittorent_logs = CleanerData {
            path: drive.to_owned() + "Program Files\\qBittorrent\\logs\\*.log",
            program: "qBittorrent".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_qbittorent_logs);
        //</editor-fold>
        //<editor-fold desc="CCleaner">
        let c_program_files_ccleaner_logs = CleanerData {
            path: drive.to_owned() + "Program Files\\CCleaner\\LOG\\*",
            program: "ССleaner".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_ccleaner_logs);
        //</editor-fold>
        //<editor-fold desc="IObit Malware Fighter">
        let c_program_files_ccleaner_logs = CleanerData {
            path: drive.to_owned() + "ProgramData\\IObit\\IObit Malware Fighter\\*.log",
            program: "IObit Malware Fighter".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_ccleaner_logs);
        let c_program_data_iobit_iobit_malware_finghter_homepage_advisor = CleanerData {
            path: drive.to_owned() + "ProgramData\\IObit\\IObit Malware Fighter\\Homepage Advisor\\*.log",
            program: "IObit Malware Fighter".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_data_iobit_iobit_malware_finghter_homepage_advisor);
        //</editor-fold>
        //<editor-fold desc="IObit Driver Booster">
        let c_users_appdata_roaming_iobit_driver_booster_logs = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\IObit\\Driver Booster\\Logs\\*",
            program: "IObit Driver Booster".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: true,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_roaming_iobit_driver_booster_logs);
        let c_program_files_x86_iobit_driver_booster = CleanerData {
            path: drive.to_owned() + "Program Files (x86)\\IObit\\Driver Booster\\*.log",
            program: "IObit Driver Booster".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_x86_iobit_driver_booster);
        let c_program_files_x86_iobit_driver_booster_1 = CleanerData {
            path: drive.to_owned() + "Program Files (x86)\\IObit\\Driver Booster\\*.txt",
            program: "IObit Driver Booster".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_x86_iobit_driver_booster_1);
        //</editor-fold>
        //<editor-fold desc="Process Lasso">
        let c_program_data_process_lasso_logs = CleanerData {
            path: drive.to_owned() + "ProgramData\\ProcessLasso\\logs\\*",
            program: "Process Lasso".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_data_process_lasso_logs);
        //</editor-fold>
        //<editor-fold desc="OBS Studio">
        let c_program_files_obs_studio_bin_64bit = CleanerData {
            path: drive.to_owned() + "Program Files\\obs-studio\\bin\\64bit\\*.log",
            program: "OBS Studio".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_obs_studio_bin_64bit);
        let c_users_appdata_roaming_obs_studio_logs = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\obs-studio\\logs\\*txt",
            program: "OBS Studio".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_roaming_obs_studio_logs);
        //</editor-fold>
        //<editor-fold desc="Unity Hub">
        let c_program_files_unity_hub = CleanerData {
            path: drive.to_owned() + "Program Files\\Unity Hub\\*.html",
            program: "Unity Hub".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_unity_hub);
        //</editor-fold>
        //<editor-fold desc="KeePass 2">
        let c_program_files_keepass_password_safe_2 = CleanerData {
            path: drive.to_owned() + "Program Files\\KeePass Password Safe 2\\*.txt",
            program: "KeePass 2".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_keepass_password_safe_2);
        //</editor-fold>
        //<editor-fold desc="1Password">
        let c_users_appdata_local_1password_logs_setup = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\1Password\\logs\\setup\\*.log",
            program: "1Password".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_local_1password_logs_setup);
        //</editor-fold>
        //<editor-fold desc="LGHUB">
        let c_program_files_lghub = CleanerData {
            path: drive.to_owned() + "Program Files\\LGHUB",
            program: "LGHUB".parse().unwrap(),
            files_to_remove: vec![
                "LICENSE".parse().unwrap(),
                "LICENSES.chromium.html".parse().unwrap()
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_lghub);
        //</editor-fold>
        //<editor-fold desc="DeepL">
        let c_users_appdata_local_deepl_se_logs = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\DeepL_SE\\logs\\*",
            program: "LGHUB".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_local_deepl_se_logs);
        let c_users_appdata_local_deepl_se_cache = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\DeepL_SE\\cache\\*",
            program: "LGHUB".parse().unwrap(),
            files_to_remove: vec![],
            category: String::from("Cache"),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_local_deepl_se_cache);
        //</editor-fold>
        //<editor-fold desc="Microsoft Lobe">
        let c_users_appdata_roaming_lobe_logs = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\Lobe\\logs\\*",
            program: "Microsoft Lobe".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_roaming_lobe_logs);
        //</editor-fold>
        //<editor-fold desc="Tonfotos Telegram Connector">
        let c_users_pictures_tonfotos_telegram_connector = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\Pictures\\Tonfotos Telegram Connector\\*",
            program: "Tonfotos Telegram Connector".parse().unwrap(),
            files_to_remove: vec![],
            category: "Images".parse().unwrap(),
            remove_directories: true,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_pictures_tonfotos_telegram_connector);
        //</editor-fold>
        //<editor-fold desc="DotNet">
        let c_program_files_x86_dotnet = CleanerData {
            path: drive.to_owned() + "Program Files\\dotnet\\*.txt",
            program: "DotNet".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_x86_dotnet);
        let c_program_files_x86_dotnet = CleanerData {
            path: drive.to_owned() + "Program Files (x86)\\dotnet\\*.txt",
            program: "DotNet".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_x86_dotnet);
        let c_users_dotnet_telemetry_storage_service = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\.dotnet\\TelemetryStorageService\\*",
            program: "DotNet".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_dotnet_telemetry_storage_service);
        //</editor-fold>
        //<editor-fold desc="MCCreator">
        let c_users_mccreator_logs = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\.mcreator\\logs\\*.log",
            program: "MCCreator".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_mccreator_logs);
        //</editor-fold>
        //<editor-fold desc="7-Zip">
        let c_program_files_7_zip = CleanerData {
            path: drive.to_owned() + "Program Files\\7-Zip\\*.txt",
            program: "7-Zip".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_7_zip);
        //</editor-fold>
        //<editor-fold desc="Tribler">
        let c_users_appdata_roaming_tribler = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\.Tribler\\*.log",
            program: "Tribler".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_roaming_tribler);
        //</editor-fold>
        //<editor-fold desc="I2P">
        let c_users_appdata_local_i2peasy_addressbook = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\i2peasy\\addressbook",
            program: "I2P".parse().unwrap(),
            files_to_remove: vec![
                "log.txt".parse().unwrap()
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_local_i2peasy_addressbook);
        let c_users_appdata_local_i2peasy = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\i2peasy",
            program: "I2P".parse().unwrap(),
            files_to_remove: vec![
                "eventlog.txt".parse().unwrap(),
                "wrapper.log".parse().unwrap()
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_local_i2peasy);
        let c_users_appdata_local_i2peasy_logs = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\i2peasy\\logs\\*",
            program: "I2P".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: true,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_local_i2peasy_logs);
        let c_users_appdata_local_i2peasy_licenses = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\i2peasy\\licenses\\*",
            program: "I2P".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: true,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_local_i2peasy_licenses);
        //</editor-fold>
        //<editor-fold desc="BoxedAppPacker">
        let c_program_filex_x86_boxedapppacker = CleanerData {
            path: drive.to_owned() + "Program Files (x86)\\BoxedAppPacker",
            program: "BoxedAppPacker".parse().unwrap(),
            files_to_remove: vec![
                "CHANGELOG.txt".parse().unwrap(),
                "HomePage.url".parse().unwrap(),
                "purchase.url".parse().unwrap()
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_filex_x86_boxedapppacker);
        //</editor-fold>
        //<editor-fold desc="Enigma Virtual Box">
        let c_program_files_enigma_virtual_box = CleanerData {
            path: drive.to_owned() + "Program Files (x86)\\Enigma Virtual Box",
            program: "Enigma Virtual Box".parse().unwrap(),
            files_to_remove: vec![
                "help.chm".parse().unwrap(),
                "History.txt".parse().unwrap(),
                "License.txt".parse().unwrap(),
                "site.url".parse().unwrap(),
                "forum.url".parse().unwrap(),
                "support.url".parse().unwrap()
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_enigma_virtual_box);
        //</editor-fold>
        //<editor-fold desc="GnuPG">
        let c_program_files_gnupg = CleanerData {
            path: drive.to_owned() + "Program Files (x86)\\GnuPG",
            program: "GnuPG".parse().unwrap(),
            files_to_remove: vec![
                "README.txt".parse().unwrap(),
                "VERSION".parse().unwrap()
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_gnupg);
        //</editor-fold>
        //<editor-fold desc="Gpg4win">
        let c_program_files_enigma_x86_gpg4win = CleanerData {
            path: drive.to_owned() + "Program Files (x86)\\Gpg4win",
            program: "Gpg4win".parse().unwrap(),
            files_to_remove: vec![
                "VERSION".parse().unwrap()
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_enigma_x86_gpg4win);
        //</editor-fold>
        //<editor-fold desc="Inno Setup 6">
        let c_program_files_enigma_x86_inno_setup_6 = CleanerData {
            path: drive.to_owned() + "Program Files (x86)\\Inno Setup 6",
            program: "Inno Setup 6".parse().unwrap(),
            files_to_remove: vec![
                "whatsnew.htm".parse().unwrap(),
                "isfaq.url".parse().unwrap(),
                "license.txt".parse().unwrap()
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_enigma_x86_inno_setup_6);
        let c_program_files_enigma_x86_inno_setup_6 = CleanerData {
            path: drive.to_owned() + "Program Files (x86)\\Inno Setup 6\\Examples\\*.txt",
            program: "Inno Setup 6".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_enigma_x86_inno_setup_6);
        //</editor-fold>
        //<editor-fold desc="VirtualBox">
        let c_users_virtualbox_vms_logs = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\VirtualBox VMs\\**\\Logs\\*.log",
            program: "VirtualBox".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_virtualbox_vms_logs);
        let c_users_virtualbox_vms = CleanerData {
            path: drive.to_owned() + "Program Files\\Oracle\\VirtualBox\\*.rtf",
            program: "VirtualBox".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_virtualbox_vms);
        let c_users_virtualbox_vms_doc = CleanerData {
            path: drive.to_owned() + "Program Files\\Oracle\\VirtualBox\\doc\\*",
            program: "VirtualBox".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: true,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_virtualbox_vms_doc);
        //</editor-fold>
        //<editor-fold desc="Recaf">
        let c_users_appdata_roaming_recaf = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\Recaf\\*.log",
            program: "Recaf".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_roaming_recaf);
        //</editor-fold>
        //<editor-fold desc="Process Hacker 2">
        let c_program_files_process_hacker_2 = CleanerData {
            path: drive.to_owned() + "Program Files\\Process Hacker 2",
            program: "Process Hacker 2".parse().unwrap(),
            files_to_remove: vec![
                "CHANGELOG.txt".parse().unwrap(),
                "COPYRIGHT.txt".parse().unwrap(),
                "LICENSE.txt".parse().unwrap(),
                "README.txt".parse().unwrap(),
                "ProcessHacker.sig".parse().unwrap()
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_process_hacker_2);
        //</editor-fold>
        //<editor-fold desc="Docker">
        let c_programdata_dockerdesktop = CleanerData {
            path: drive.to_owned() + "ProgramData\\DockerDesktop\\*.txt",
            program: "Docker".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_programdata_dockerdesktop);
        let c_users_appdata_local_docker_logs = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\Docker\\log\\**\\*",
            program: "Docker".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: true,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_local_docker_logs);
        let c_users_appdata_local_docker = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\Docker\\*.txt",
            program: "Docker".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_local_docker);
        //</editor-fold>
        //<editor-fold desc="HiAlgo Boost">
        let c_programdata_dockerdesktop = CleanerData {
            path: drive.to_owned() + "Program Files (x86)\\HiAlgo\\Plugins\\BOOST",
            program: "HiAlgo Boost".parse().unwrap(),
            files_to_remove: vec![
                "hialgo_eula.txt".parse().unwrap(),
                "Update Boost.log".parse().unwrap(),
                "UpdateListing.txt".parse().unwrap()
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_programdata_dockerdesktop);
        //</editor-fold>
        //<editor-fold desc="SoundWire Server">
        let c_program_files_x86_soundwire_server = CleanerData {
            path: drive.to_owned() + "Program Files (x86)\\SoundWire Server",
            program: "SoundWire Server".parse().unwrap(),
            files_to_remove: vec![
                "license.txt".parse().unwrap(),
                "opus_license.txt".parse().unwrap(),
                "readerwriterqueue_license.txt".parse().unwrap()
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_x86_soundwire_server);
        //</editor-fold>
        //<editor-fold desc="System Informer">
        let c_program_files_systeminformer = CleanerData {
            path: drive.to_owned() + "Program Files\\SystemInformer",
            program: "SoundWire Server".parse().unwrap(),
            files_to_remove: vec![
                "LICENSE.txt".parse().unwrap(),
                "README.txt".parse().unwrap(),
                "COPYRIGHT.txt".parse().unwrap()
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_systeminformer);
        //</editor-fold>
        //<editor-fold desc="Sandboxie Plus">
        let c_program_files_sandboxie_plus = CleanerData {
            path: drive.to_owned() + "Program Files\\Sandboxie-Plus",
            program: "SoundWire Server".parse().unwrap(),
            files_to_remove: vec![
                "Manifest0.txt".parse().unwrap(),
                "Manifest1.txt".parse().unwrap(),
                "Manifest2.txt".parse().unwrap()
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_sandboxie_plus);
        //</editor-fold>
        //<editor-fold desc="JetBrains">
        let c_program_files_jetbrains_license = CleanerData {
            path: drive.to_owned() + "Program Files\\JetBrains\\**\\license",
            program: "JetBrains".parse().unwrap(),
            files_to_remove: vec![
                "javahelp_license.txt".parse().unwrap(),
                "javolution_license.txt".parse().unwrap(),
                "launcher-third-party-libraries.html".parse().unwrap(),
                "saxon-conditions.html".parse().unwrap(),
                "third-party-libraries.html".parse().unwrap(),
                "yourkit-license-redist.txt".parse().unwrap(),
                "remote-dev-server.html".parse().unwrap()
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_jetbrains_license);
        let c_program_files_jetbrains = CleanerData {
            path: drive.to_owned() + "Program Files\\JetBrains\\**",
            program: "JetBrains".parse().unwrap(),
            files_to_remove: vec![
                "LICENSE.txt".parse().unwrap(),
                "NOTICE.txt".parse().unwrap()
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_jetbrains);
        let c_users_appdata_local_jetbrains_logs = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\JetBrains\\**\\log\\*",
            program: "JetBrains".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: true,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_local_jetbrains_logs);
        //</editor-fold>
        //<editor-fold desc="AAF Optimus DCH Audio">
        let c_program_files_afftweak = CleanerData {
            path: drive.to_owned() + "Program Files\\AAFTweak",
            program: "AAF Optimus DCH Audio".parse().unwrap(),
            files_to_remove: vec![
                "RT.pdb".parse().unwrap()
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_afftweak);
        //</editor-fold>
        //<editor-fold desc="FL Studio">
        let c_program_files_image_line = CleanerData {
            path: drive.to_owned() + "Program Files\\Image-Line\\**",
            program: "FL Studio".parse().unwrap(),
            files_to_remove: vec![
                "WhatsNew.rtf".parse().unwrap()
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_image_line);
        let c_program_files_image_line_shared_start = CleanerData {
            path: drive.to_owned() + "Program Files\\Image-Line\\Shared\\Start\\**",
            program: "FL Studio".parse().unwrap(),
            files_to_remove: vec![
                "What's new.lnk".parse().unwrap()
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_image_line_shared_start);
        //</editor-fold>
        //<editor-fold desc="ASIO4ALL v2">
        let c_program_files_x86_asio4all = CleanerData {
            path: drive.to_owned() + "Program Files (x86)\\ASIO4ALL v2",
            program: "ASIO4ALL v2".parse().unwrap(),
            files_to_remove: vec![
                "ASIO4ALL Web Site.url".parse().unwrap()
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_x86_asio4all);
        //</editor-fold>
        //<editor-fold desc="Rave">
        let c_users_appdata_roaming_rave_logs = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\Rave\\logs\\*.log",
            program: "Rave".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_roaming_rave_logs);
        let c_users_appdata_roaming_rave_cache = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\Rave\\Cache\\*",
            program: "Rave".parse().unwrap(),
            files_to_remove: vec![],
            category: String::from("Cache"),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_roaming_rave_cache);
        let c_users_appdata_roaming_rave_code_cache = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\Rave\\Code Cache\\*",
            program: "Rave".parse().unwrap(),
            files_to_remove: vec![],
            category: String::from("Cache"),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_roaming_rave_code_cache);
        //</editor-fold>
        //<editor-fold desc="Magpie">
        let c_program_files_magpie_logs = CleanerData {
            path: drive.to_owned() + "Program Files\\Magpie\\logs\\*.log",
            program: "Magpie".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_magpie_logs);
        let c_program_files_magpie_logs = CleanerData {
            path: drive.to_owned() + "Program Files\\Magpie\\cache\\*",
            program: "Magpie".parse().unwrap(),
            files_to_remove: vec![ ],
            category: String::from("Cache"),
            remove_directories: true,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_magpie_logs);
        //</editor-fold>
        //<editor-fold desc="LibreOffice">
        let c_program_files_libreoffice = CleanerData {
            path: drive.to_owned() + "Program Files\\LibreOffice",
            program: "LibreOffice".parse().unwrap(),
            files_to_remove: vec![
                "CREDITS.fodt".parse().unwrap(),
                "LICENSE.html".parse().unwrap(),
                "license.txt".parse().unwrap(),
                "NOTICE".parse().unwrap(),
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_libreoffice);
        let c_program_files_libreoffice_readmes = CleanerData {
            path: drive.to_owned() + "Program Files\\LibreOffice\\readmes\\*",
            program: "LibreOffice".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: true,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_libreoffice_readmes);
        //</editor-fold>
        //<editor-fold desc="Cheat Engine">
        let c_program_files_cheat_engine_7_5 = CleanerData {
            path: drive.to_owned() + "Program Files\\Cheat Engine 7.5\\*.txt",
            program: "Cheat Engine".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_cheat_engine_7_5);
        //</editor-fold>
        //<editor-fold desc="Epic Games">
        let c_users_appdata_local_epic_games_launcher_saved_logs = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\EpicGamesLauncher\\Saved\\Logs\\*.log",
            program: "Epic Games".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_local_epic_games_launcher_saved_logs);
        let c_users_appdata_local_epic_online_services_uihelper_saved_logs = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\EpicOnlineServicesUIHelper\\Saved\\Logs\\*.log",
            program: "Epic Games".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_local_epic_online_services_uihelper_saved_logs);
        //</editor-fold>
        //<editor-fold desc="VK GameCenter">
        let c_users_appdata_local_epic_games_launcher_saved_logs = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\GameCenter\\*.log",
            program: "VK GameCenter".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_local_epic_games_launcher_saved_logs);
        //</editor-fold>
        //<editor-fold desc="Adobe">
        let c_program_files_adobe_legal = CleanerData {
            path: drive.to_owned() + "Program Files\\Adobe\\**\\Legal\\**\\*.html",
            program: "Adobe".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_adobe_legal);
        //</editor-fold>
        //<editor-fold desc="Git">
        let c_program_files_adobe_legal = CleanerData {
            path: drive.to_owned() + "Program Files\\Git",
            program: "Git".parse().unwrap(),
            files_to_remove: vec![
                "LICENSE.txt".parse().unwrap(),
                "ReleaseNotes.html".parse().unwrap()
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_adobe_legal);
        //</editor-fold>
        //<editor-fold desc="DotNet">
        let c_program_files_dotnet = CleanerData {
            path: drive.to_owned() + "Program Files\\dotnet\\*.txt",
            program: "DotNet".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_dotnet);
        //</editor-fold>
        //<editor-fold desc="WinRAR">
        let c_program_files_winrar = CleanerData {
            path: drive.to_owned() + "Program Files\\WinRAR",
            program: "WinRaR".parse().unwrap(),
            files_to_remove: vec![
                "License.txt".parse().unwrap(),
                "Order.htm".parse().unwrap(),
                "Rar.txt".parse().unwrap(),
                "ReadMe.rus.txt".parse().unwrap(),
                "ReadMe.txt".parse().unwrap(),
                "WhatsNew.txt".parse().unwrap()
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_winrar);
        //</editor-fold>
        //<editor-fold desc="Windows SDK">
        let c_program_files_x86_windows_kits_licenses = CleanerData {
            path: drive.to_owned() + "Program Files (x86)\\Windows Kits\\**\\Licenses\\**\\*.rtf",
            program: "Windows SDK".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_x86_windows_kits_licenses);
        //</editor-fold>
        //<editor-fold desc="Electron App's">
        let users_appdata_local_programs = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\Programs\\**",
            program: "Electron App's".parse().unwrap(),
            files_to_remove: vec![
                "LICENSES.chromium.html".parse().unwrap(),
                "LICENSE.electron.txt".parse().unwrap(),
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(users_appdata_local_programs);
        let users_appdata_roaming_ow_electron_logs = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\ow-electron\\**\\logs\\*",
            program: "Electron App's".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: true,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(users_appdata_roaming_ow_electron_logs);
        //</editor-fold>
        //<editor-fold desc="PowerToys">
        let program_files_powertoys = CleanerData {
            path: drive.to_owned() + "Program Files\\PowerToys",
            program: "PowerToys".parse().unwrap(),
            files_to_remove: vec![
                "License.rtf".parse().unwrap(),
                "Notice.md".parse().unwrap()
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: true,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(program_files_powertoys);
        //</editor-fold>
        //<editor-fold desc="LM Studio">
        let users_appdata_roaming_lm_studio_logs = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\LM Studio\\logs\\*",
            program: "LM Studio".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: true,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(users_appdata_roaming_lm_studio_logs);
        //</editor-fold>
        //<editor-fold desc="ImgBurn">
        let users_appdata_roaming_imgburn_log_files = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\ImgBurn\\Log Files\\*",
            program: "ImgBurn".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: true,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(users_appdata_roaming_imgburn_log_files);
        let users_appdata_roaming_imgburn_log_files = CleanerData {
            path: drive.to_owned() + "Program Files (x86)\\ImgBurn",
            program: "ImgBurn".parse().unwrap(),
            files_to_remove: vec![
                String::from("ReadMe.txt")
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(users_appdata_roaming_imgburn_log_files);
        //</editor-fold>
        //<editor-fold desc="Magic TDX">
        let program_files_magic_txd_licenses = CleanerData {
            path: drive.to_owned() + "Program Files\\Magic TXD\\licenses\\*",
            program: "Magic TDX".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: true,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(program_files_magic_txd_licenses);
        //</editor-fold>
        //<editor-fold desc="VulcanRT">
        let program_files_86_vulcan_rt = CleanerData {
            path: drive.to_owned() + "Program Files (x86)\\VulkanRT\\**",
            program: "VulcanRT".parse().unwrap(),
            files_to_remove: vec![
                String::from("LICENSE.txt"),
                String::from("VULKANRT_LICENSE.rtf")
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: true,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(program_files_86_vulcan_rt);
        //</editor-fold>
        //<editor-fold desc="Git">
        let program_files_git = CleanerData {
            path: drive.to_owned() + "Program Files\\Git",
            program: "Git".parse().unwrap(),
            files_to_remove: vec![
                String::from("LICENSE.txt"),
                String::from("ReleaseNotes.html")
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: true,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(program_files_git);
        //</editor-fold>

        //<editor-fold desc="Code Editors">

        //<editor-fold desc="Sublime Text">
        let c_program_files_sublime_text = CleanerData {
            path: drive.to_owned() + "Program Files\\Sublime Text\\*.txt",
            program: "Sublime Text".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_sublime_text);
        //</editor-fold>
        //<editor-fold desc="VS Code">
        let c_users_appdata_roaming_code_logs = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\Code\\logs",
            program: "VS Code".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: true,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_roaming_code_logs);
        let c_users_appdata_roaming_code_logs = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\Code\\Network",
            program: "VS Code".parse().unwrap(),
            files_to_remove: vec![
                String::from("Browser cookies"),
                String::from("Cookies-journal"),
            ],
            category: String::from("Browser cookies"),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: true,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_roaming_code_logs);
        //</editor-fold>

        //</editor-fold>
        //<editor-fold desc="Browsers">

        //<editor-fold desc="Brave Browser">
        let c_program_files_brave_software_brave_browser_application = CleanerData {
            path: drive.to_owned() + "Program Files\\BraveSoftware\\Brave-Browser\\Application\\*.log",
            program: "Brave Browser".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_brave_software_brave_browser_application);
        let c_users_appdata_local_brave_software_brave_browser_user_data_default = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\BraveSoftware\\Brave-Browser\\User Data\\Default",
            program: "Brave Browser".parse().unwrap(),
            files_to_remove: vec![
                "Favicons".parse().unwrap(),
                "Favicons-journal".parse().unwrap(),
                "History".parse().unwrap(),
                "History-journal".parse().unwrap(),
                "Visited Links".parse().unwrap()
            ],
            category: "LastActivity".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_local_brave_software_brave_browser_user_data_default);
        let users_appdata_local_bravesoftware_brave_browser_user_data_default = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\BraveSoftware\\Brave-Browser\\User Data\\Default",
            program: "Brave Browser".parse().unwrap(),
            files_to_remove: vec![
                "Login Data".parse().unwrap(),
                "Login Data For Account".parse().unwrap(),
                "Login Data For Account-journal".parse().unwrap(),
                "Login Data-journal".parse().unwrap()
            ],
            category: String::from("Browser passwords"),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(users_appdata_local_bravesoftware_brave_browser_user_data_default);
        let users_appdata_local_bravesoftware_brave_browser_user_data_default_network = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\BraveSoftware\\Brave-Browser\\User Data\\Default\\Network",
            program: "Brave Browser".parse().unwrap(),
            files_to_remove: vec![
                String::from("Browser cookies"),
                "Cookies-journal".parse().unwrap()
            ],
            category: String::from("Browser cookies"),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(users_appdata_local_bravesoftware_brave_browser_user_data_default_network);
        let c_users_appdata_local_brave_software_brave_browser_user_data_default_dawn_cache = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\BraveSoftware\\Brave-Browser\\User Data\\Default\\DawnCache",
            program: "Brave Browser".parse().unwrap(),
            files_to_remove: vec![
                "data_0".parse().unwrap(),
                "data_1".parse().unwrap(),
                "data_2".parse().unwrap(),
                "data_3".parse().unwrap(),
                "index".parse().unwrap()
            ],
            category: String::from("Cache"),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_local_brave_software_brave_browser_user_data_default_dawn_cache);
        let c_users_appdata_local_brave_software_brave_browser_user_data_default_gpu_cache = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\BraveSoftware\\Brave-Browser\\User Data\\Default\\GPUCache",
            program: "Brave Browser".parse().unwrap(),
            files_to_remove: vec![
                "data_0".parse().unwrap(),
                "data_1".parse().unwrap(),
                "data_2".parse().unwrap(),
                "data_3".parse().unwrap(),
                "index".parse().unwrap()
            ],
            category: String::from("Cache"),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_local_brave_software_brave_browser_user_data_default_gpu_cache);
        //</editor-fold>
        //<editor-fold desc="Google Chrome">
        let program_files_x86_google_google_updater = CleanerData {
            path: drive.to_owned() + "Program Files (x86)\\Google\\GoogleUpdater\\*.log",
            program: "Google Chrome".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(program_files_x86_google_google_updater);
        let c_users_appdata_local_google_chrome_user_data_default = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\Google\\Chrome\\User Data\\Default",
            program: "Google Chrome".parse().unwrap(),
            files_to_remove: vec![
                "Favicons".parse().unwrap(),
                "Favicons-journal".parse().unwrap(),
                "History".parse().unwrap(),
                "History-journal".parse().unwrap(),
                "Visited Links".parse().unwrap()
            ],
            category: "LastActivity".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_local_google_chrome_user_data_default);
        let c_users_appdata_local_google_chrome_user_data_default = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\Google\\Chrome\\User Data\\Default",
            program: "Google Chrome".parse().unwrap(),
            files_to_remove: vec![
                "Login Data".parse().unwrap(),
                "Login Data For Account".parse().unwrap(),
                "Login Data For Account-journal".parse().unwrap(),
                "Login Data-journal".parse().unwrap()
            ],
            category: String::from("Browser passwords"),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_local_google_chrome_user_data_default);
        let c_users_appdata_local_google_chrome_user_data_default_network = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\Google\\Chrome\\User Data\\Default\\Network",
            program: "Google Chrome".parse().unwrap(),
            files_to_remove: vec![
                String::from("Browser cookies"),
                "Cookies-journal".parse().unwrap()
            ],
            category: String::from("Browser cookies"),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_local_google_chrome_user_data_default_network);
        //</editor-fold>
        //<editor-fold desc="Vivaldi">
        let c_users_appdata_local_vivaldi_user_data_default = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\Vivaldi\\User Data\\Default",
            program: "Vivaldi".parse().unwrap(),
            files_to_remove: vec![
                "Favicons".parse().unwrap(),
                "Favicons-journal".parse().unwrap(),
                "History".parse().unwrap(),
                "History-journal".parse().unwrap(),
                "Visited Links".parse().unwrap()
            ],
            category: "LastActivity".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_local_vivaldi_user_data_default);
        let c_users_appdata_local_vivaldi_user_data_default = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\Vivaldi\\User Data\\Default",
            program: "Vivaldi".parse().unwrap(),
            files_to_remove: vec![
                "Login Data".parse().unwrap(),
                "Login Data For Account".parse().unwrap(),
                "Login Data For Account-journal".parse().unwrap(),
                "Login Data-journal".parse().unwrap()
            ],
            category: String::from("Browser passwords"),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_local_vivaldi_user_data_default);
        let c_users_appdata_local_vivaldi_user_data_default_network = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\Vivaldi\\User Data\\Default\\Network",
            program: "Vivaldi".parse().unwrap(),
            files_to_remove: vec![
                String::from("Browser cookies"),
                "Cookies-journal".parse().unwrap()
            ],
            category: String::from("Browser cookies"),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_local_vivaldi_user_data_default_network);
        let c_users_appdata_local_vivaldi_user_data_default_network = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\Pictures\\Vivaldi Captures\\*",
            program: "Vivaldi".parse().unwrap(),
            files_to_remove: vec![],
            category: String::from("Images"),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_local_vivaldi_user_data_default_network);
        //</editor-fold>
        //<editor-fold desc="Opera GX">
        let users_appdata_roaming_opera_software_opera_gx_stable = CleanerData {
            path: String::from(drive.clone() + "Users\\" + username + "\\AppData\\Roaming\\Opera Software\\Opera GX Stable"),
            program: String::from("Opera GX"),
            files_to_remove: vec![
               String::from("Favicons"),
               String::from("Favicons-journal"),
               String::from("History"),
               String::from("History-journal"),
               String::from("Visited Links"),
            ],
            category: "LastActivity".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(users_appdata_roaming_opera_software_opera_gx_stable);
        let users_appdata_roaming_opera_software_opera_gx_stable = CleanerData {
            path: String::from(drive.clone() + "Users\\" + username + "\\AppData\\Roaming\\Opera Software\\Opera GX Stable"),
            program: String::from("Opera GX"),
            files_to_remove: vec![
                String::from("Login Data"),
                String::from("Login Data-journal")
            ],
            category: String::from("Browser passwords"),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(users_appdata_roaming_opera_software_opera_gx_stable);
        let users_appdata_roaming_opera_software_opera_gx_stable = CleanerData {
            path: String::from(drive.clone() + "Users\\" + username + "\\AppData\\Roaming\\Opera Software\\Opera GX Stable\\Network"),
            program: String::from("Opera GX"),
            files_to_remove: vec![
                String::from("Browser cookies"),
                String::from("Cookies-journal")
            ],
            category: String::from("Browser cookies"),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(users_appdata_roaming_opera_software_opera_gx_stable);
        //</editor-fold>
        //<editor-fold desc="Mozilla Firefox">
        let program_files_mozila_firefox = CleanerData {
            path: String::from(drive.clone() + "Program Files\\Mozilla Firefox"),
            program: String::from("Mozilla Firefox"),
            files_to_remove: vec![
                String::from("install.log"),
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(program_files_mozila_firefox);
        let users_appdata_roaming_mozila_firefox_profiles = CleanerData {
            path: String::from(drive.clone() + "Users\\" + username + "\\AppData\\Roaming\\Mozilla\\Firefox\\Profiles\\**"),
            program: String::from("Mozilla Firefox"),
            files_to_remove: vec![
                String::from("favicons.sqlite"),
                String::from("favicons.sqlite-shm"),
                String::from("favicons.sqlite-wal"),
            ],
            category: String::from("LastActivity"),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(users_appdata_roaming_mozila_firefox_profiles);
        let users_appdata_roaming_mozila_firefox_profiles = CleanerData {
            path: String::from(drive.clone() + "Users\\" + username + "\\AppData\\Roaming\\Mozilla\\Firefox\\Profiles\\**"),
            program: String::from("Mozilla Firefox"),
            files_to_remove: vec![
                String::from("cookies.sqlite"),
                String::from("cookies.sqlite-shm"),
                String::from("cookies.sqlite-wal"),
            ],
            category: String::from("Browser cookies"),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(users_appdata_roaming_mozila_firefox_profiles);
        let users_appdata_roaming_mozila_firefox_profiles_shader_cache = CleanerData {
            path: String::from(drive.clone() + "Users\\" + username + "\\AppData\\Roaming\\Mozilla\\Firefox\\Profiles\\**\\shader-cache\\*"),
            program: String::from("Mozilla Firefox"),
            files_to_remove: vec![],
            category: String::from("Cache"),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(users_appdata_roaming_mozila_firefox_profiles_shader_cache);
        //</editor-fold>
        //<editor-fold desc="LibreWoolf">
        let users_appdata_roaming_librewolf_profiles_favicons = CleanerData {
            path: String::from(drive.clone() + "Users\\" + username + "\\AppData\\Roaming\\librewolf\\Profiles\\**"),
            program: String::from("LibreWolf"),
            files_to_remove: vec![
                String::from("favicons.sqlite"),
                String::from("favicons.sqlite-shm"),
                String::from("favicons.sqlite-wal"),
            ],
            category: String::from("LastActivity"),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(users_appdata_roaming_librewolf_profiles_favicons);
        let users_appdata_roaming_librewolf_profiles_cookies = CleanerData {
            path: String::from(drive.clone() + "Users\\" + username + "\\AppData\\Roaming\\librewolf\\Profiles\\**"),
            program: String::from("LibreWolf"),
            files_to_remove: vec![
                String::from("cookies.sqlite"),
                String::from("cookies.sqlite-shm"),
                String::from("cookies.sqlite-wal"),
            ],
            category: String::from("Browser cookies"),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(users_appdata_roaming_librewolf_profiles_cookies);
        //</editor-fold>

        //</editor-fold>
        //<editor-fold desc="Video">

        //<editor-fold desc="HandBrake">
        let c_users_appdata_roaming_handbrake_logs = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\HandBrake\\logs\\*.txt",
            program: "HandBrake".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_roaming_handbrake_logs);
        let c_users_appdata_roaming_handbrake_docs = CleanerData {
            path: drive.to_owned() + "Program Files\\HandBrake\\doc\\*",
            program: "HandBrake".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: true,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_roaming_handbrake_docs);
        //</editor-fold>
        //<editor-fold desc="Topaz Video AI">
        let c_programdata_topaz_labs_llc_topaz_video_ai = CleanerData {
            path: drive.to_owned() + "ProgramData\\Topaz Labs LLC\\Topaz Video AI\\*.txt",
            program: "Topaz Video AI".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_programdata_topaz_labs_llc_topaz_video_ai);
        //</editor-fold>
        //<editor-fold desc="AVCLabs Video Enhancer AI">
        let c_program_files_x86_avclabs_avclabs_video_encharcer_ai_1 = CleanerData {
            path: drive.to_owned() + "Program Files (x86)\\AVCLabs\\AVCLabs Video Enhancer AI\\*.txt",
            program: "AVCLabs Video Enhancer AI".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_x86_avclabs_avclabs_video_encharcer_ai_1);
        let c_program_files_x86_avclabs_avclabs_video_encharcer_ai_2 = CleanerData {
            path: drive.to_owned() + "Program Files (x86)\\AVCLabs\\AVCLabs Video Enhancer AI\\*.html",
            program: "AVCLabs Video Enhancer AI".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_x86_avclabs_avclabs_video_encharcer_ai_2);
        let c_program_files_x86_avclabs_avclabs_video_encharcer_ai_logs = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\AVCLabs Video Enhancer AI\\logs\\*.log",
            program: "AVCLabs Video Enhancer AI".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_x86_avclabs_avclabs_video_encharcer_ai_logs);
        //</editor-fold>
        //<editor-fold desc="iTop Screen Recorder">
        let c_users_appdata_roaming_itop_screen_recorder_logs = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\iTop Screen Recorder\\Logs\\*.log",
            program: "iTop Screen Recorder".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_roaming_itop_screen_recorder_logs);
        //</editor-fold>
        //<editor-fold desc="VideoLAN">
        let c_program_files_videolan_vlc = CleanerData {
            path: drive.to_owned() + "Program Files\\VideoLAN\\VLC",
            program: "ASIO4ALL v2".parse().unwrap(),
            files_to_remove: vec![
                "AUTHORS.txt".parse().unwrap(),
                "COPYING.txt".parse().unwrap(),
                "NEWS.txt".parse().unwrap(),
                "README.txt".parse().unwrap(),
                "THANKS.txt".parse().unwrap(),
                "VideoLAN Website.url".parse().unwrap(),
                "Documentation.url".parse().unwrap(),
                "New_Skins.url".parse().unwrap(),
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_videolan_vlc);
        //</editor-fold>

        //</editor-fold>
        //<editor-fold desc="Crypto Wallets">

        //<editor-fold desc="Exodus Crypto Wallet">
        let c_users_appdata_local_exodus = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\exodus\\*.log",
            program: "Exodus Crypto Wallet".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_local_exodus);
        let c_users_appdata_local_exodus = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\exodus\\**",
            program: "Exodus Crypto Wallet".parse().unwrap(),
            files_to_remove: vec![
                String::from("SquirrelSetup.log")
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_local_exodus);
        let c_users_appdata_local_exodus = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\exodus",
            program: "Exodus Crypto Wallet".parse().unwrap(),
            files_to_remove: vec![
                String::from("SquirrelSetup.log")
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_local_exodus);
        //</editor-fold>
        //<editor-fold desc="Wasabi Wallet">
        let c_users_appdata_roaming_walletwasabi_client = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\WalletWasabi\\Client\\*.txt",
            program: "Wasabi Wallet".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_roaming_walletwasabi_client);
        //</editor-fold>
        //<editor-fold desc="Bit Monero">
        let c_programdata_bitmonero = CleanerData {
            path: drive.to_owned() + "ProgramData\\bitmonero\\*.log",
            program: "Bit Monero".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_programdata_bitmonero);
        //</editor-fold>

        //</editor-fold>
        //<editor-fold desc="AntiCheats">

        //<editor-fold desc="FACEIT AC">
        let c_program_files_faceit_ac_logs = CleanerData {
            path: drive.to_owned() + "Program Files\\FACEIT AC\\logs\\*.log",
            program: "FACEIT AC".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_faceit_ac_logs);
        //</editor-fold>
        //<editor-fold desc="EasyAntiCheat">
        let c_program_files_faceit_ac_logs = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\EasyAntiCheat\\*.log",
            program: "EasyAntiCheat".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_faceit_ac_logs);
        //</editor-fold>

        //</editor-fold>
        //<editor-fold desc="Emulators">

        //<editor-fold desc="Nox">
        let c_users_vmlogs = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\vmlogs\\*",
            program: "Nox".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_vmlogs);
        let c_users_bignox = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\.BigNox\\*",
            program: "Nox".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_bignox);
        //</editor-fold>
        //<editor-fold desc="Memu">
        let c_users_memuhyperv = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\.MemuHyperv\\*log*",
            program: "Memu".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_memuhyperv);
        //</editor-fold>
        //<editor-fold desc="Gameloop">
        let c_users_appdata_roaming_gametop_launcher = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\com.gametop.launcher\\logs\\*",
            program: "Gameloop".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_roaming_gametop_launcher);
        //</editor-fold>
        //<editor-fold desc="BlueStacks 5">
        let programdata_bluestacks_nxt_dumps = CleanerData {
            path: drive.to_owned() + "ProgramData\\BlueStacks_nxt\\Dumps\\*",
            program: "BlueStacks 5".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: true,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(programdata_bluestacks_nxt_dumps);
        let c_appdata_bluestacks_nxt_logs = CleanerData {
            path: drive.to_owned() + "Program Files\\BlueStacks_nxt",
            program: "BlueStacks 5".parse().unwrap(),
            files_to_remove: vec![
                String::from("LICENSE.txt"),
                String::from("NOTICE.html"),
                String::from("ffmpeg_command_template.txt")
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_appdata_bluestacks_nxt_logs);
        let c_appdata_bluestacks_nxt_logs = CleanerData {
            path: drive.to_owned() + "ProgramData\\BlueStacks_nxt\\Logs\\*.log",
            program: "BlueStacks 5".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_appdata_bluestacks_nxt_logs);
        let c_users_pictures_bluestacks = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\Pictures\\BlueStacks\\*.png",
            program: "BlueStacks 5".parse().unwrap(),
            files_to_remove: vec![],
            category: "Images".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_pictures_bluestacks);
        //</editor-fold>
        //<editor-fold desc="GameGuard">
        let c_program_files_x86_gameguard_cache = CleanerData {
            path: drive.to_owned() + "Program Files (x86)\\GameGuard\\cache\\*.cache",
            program: "GameGuard".parse().unwrap(),
            files_to_remove: vec![],
            category: String::from("Cache"),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_x86_gameguard_cache);
        //</editor-fold>

        //</editor-fold>
        //<editor-fold desc="Games">

        //<editor-fold desc="Melissia Games Launcher">
        let users_appdata_locallow_melissia_games_launcher_game_folder_logs = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\LocalLow\\MelissiaGamesLauncher\\GameFolder\\_logs\\*.log",
            program: "Melissia Games Launcher".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(users_appdata_locallow_melissia_games_launcher_game_folder_logs);
        let program_files_x86_melissia_games_melissia_games_launcher = CleanerData {
            path: drive.to_owned() + "Program Files (x86)\\Melissia Games\\Melissia Games Launcher\\**",
            program: "Melissia Games Launcher".parse().unwrap(),
            files_to_remove: vec![
                "THIRD_PARTY_NOTICES.md".parse().unwrap(),
                "LICENSE.rtf".parse().unwrap(),
                "InnoSetupHelper.pdb".parse().unwrap(),
                "Hi3Helper.Sophon.pdb".parse().unwrap(),
                "Hi3Helper.Http.pdb".parse().unwrap(),
                "Hi3Helper.Core.pdb".parse().unwrap(),
                "DiscordRPC.pdb".parse().unwrap(),
                "ColorThief.pdb".parse().unwrap(),
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(program_files_x86_melissia_games_melissia_games_launcher);
        //</editor-fold>
        //<editor-fold desc="Lords Mobile">
        let c_users_appdata_locallow_igg_lords_mobile_pc = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\LocalLow\\IGG\\Lords Mobile PC\\*.log",
            program: "Lords Mobile".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(c_users_appdata_locallow_igg_lords_mobile_pc);
        let c_users_appdata_locallow_igg_lords_mobile = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\LocalLow\\IGG\\Lords Mobile\\*.log",
            program: "Lords Mobile".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(c_users_appdata_locallow_igg_lords_mobile);
        //</editor-fold>
        //<editor-fold desc="Roblox">
        let c_users_appdata_local_roblox_logs = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\Roblox\\logs",
            program: "Roblox".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: true,
            remove_directory_after_clean: false
        };
        database.push(c_users_appdata_local_roblox_logs);
        //</editor-fold>
        //<editor-fold desc="The Powder Toy">
        let c_users_appdata_local_roblox_logs = CleanerData {
            path: String::from(drive.clone() + "Users\\" + username + "\\AppData\\Roaming\\The Powder Toy\\Saves"),
            program: String::from("The Powder Toy"),
            files_to_remove: vec![],
            category: String::from("Game saves"),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: true,
            remove_directory_after_clean: false
        };
        database.push(c_users_appdata_local_roblox_logs);
        //</editor-fold>
        //<editor-fold desc="Terraria">
        let users_documents_my_gam_terraria_players = CleanerData {
            path: String::from(drive.clone() + "Users\\" + username + "\\Documents\\My Games\\Terraria\\Players"),
            program: String::from("Terraria"),
            files_to_remove: vec![],
            category: String::from("Game saves"),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: true,
            remove_directory_after_clean: false
        };
        database.push(users_documents_my_gam_terraria_players);
        let users_documents_my_gam_terraria_players = CleanerData {
            path: String::from(drive.clone() + "Users\\" + username + "\\Documents\\My Games\\Terraria\\Worlds"),
            program: String::from("Terraria"),
            files_to_remove: vec![],
            category: String::from("Game saves"),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: true,
            remove_directory_after_clean: false
        };
        database.push(users_documents_my_gam_terraria_players);
        let users_documents_my_gam_terraria = CleanerData {
            path: String::from(drive.clone() + "Users\\" + username + "\\Documents\\My Games\\Terraria"),
            program: String::from("Terraria"),
            files_to_remove: vec![
                String::from("favorites.json")
            ],
            category: String::from("Game saves"),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: true,
            remove_directory_after_clean: false
        };
        database.push(users_documents_my_gam_terraria);
        let users_documents_my_gam_terraria = CleanerData {
            path: String::from(drive.clone() + "Users\\" + username + "\\Documents\\My Games\\Terraria"),
            program: String::from("Terraria"),
            files_to_remove: vec![
                String::from("config.json"),
                String::from("input profiles.json")
            ],
            category: String::from("Game settings"),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: true,
            remove_directory_after_clean: false
        };
        database.push(users_documents_my_gam_terraria);
        //</editor-fold>
        //<editor-fold desc="Arizona Games">
        let users_appdata_local_programs_arizona_games_launcher = CleanerData {
            path: String::from(drive.clone() + "Users\\" + username + "\\AppData\\Local\\Programs\\Arizona Games Launcher"),
            program: String::from("Arizona Games Launcher"),
            files_to_remove: vec![
                String::from("logs.log")
            ],
            category: String::from("Logs"),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(users_appdata_local_programs_arizona_games_launcher);
        let users_appdata_local_programs_arizona_games_launcher_bin_moonloader = CleanerData {
            path: String::from(drive.clone() + "Users\\" + username + "\\AppData\\Local\\Programs\\Arizona Games Launcher\\bin\\**\\moonloader"),
            program: String::from("Arizona Games Launcher"),
            files_to_remove: vec![
                String::from("moonloader.log")
            ],
            category: String::from("Logs"),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(users_appdata_local_programs_arizona_games_launcher_bin_moonloader);
        let users_appdata_local_programs_arizona_games_launcher_bin_sampfuncs = CleanerData {
            path: String::from(drive.clone() + "Users\\" + username + "\\AppData\\Local\\Programs\\Arizona Games Launcher\\bin\\**\\SAMPFUNCS"),
            program: String::from("Arizona Games Launcher"),
            files_to_remove: vec![
                String::from("SAMPFUNCS.log")
            ],
            category: String::from("Logs"),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(users_appdata_local_programs_arizona_games_launcher_bin_sampfuncs);
        let users_appdata_local_programs_arizona_games_launcher_bin_crashlogs = CleanerData {
            path: String::from(drive.clone() + "Users\\" + username + "\\AppData\\Local\\Programs\\Arizona Games Launcher\\bin\\**\\crashlog\\*"),
            program: String::from("Arizona Games Launcher"),
            files_to_remove: vec![],
            category: String::from("Crash reports"),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: true,
            remove_directory_after_clean: false
        };
        database.push(users_appdata_local_programs_arizona_games_launcher_bin_crashlogs);
        let users_appdata_local_programs_arizona_games_launcher_bin_crashlogs = CleanerData {
            path: String::from(drive.clone() + "Users\\" + username + "\\AppData\\Local\\Programs\\Arizona Games Launcher\\bin\\**"),
            program: String::from("Arizona Games Launcher"),
            files_to_remove: vec![
                String::from("!GAMELOG.txt"),
                String::from("fastman92limitAdjuster.log"),
                String::from("libped.log")
            ],
            category: String::from("Logs"),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(users_appdata_local_programs_arizona_games_launcher_bin_crashlogs);
        //</editor-fold>

        //<editor-fold desc="Minecraft Clients">

        //<editor-fold desc="McLaunch">
        let users_appdata_roaming_mclaunch_launcher_crashreports = CleanerData {
            path: String::from(drive.clone() + "Users\\" + username + "\\AppData\\Roaming\\.mclaunch\\launcher_crashreports\\*"),
            program: String::from("McLaunch"),
            files_to_remove: vec![],
            category: String::from("Crash reports"),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: true
        };
        database.push(users_appdata_roaming_mclaunch_launcher_crashreports);
        //</editor-fold>

        let mut mc_database = get_minecraft_database(&drive, username);
        database.append(&mut mc_database);
        //</editor-fold>

        //</editor-fold>
        //<editor-fold desc="Messangers""">

        //<editor-fold desc="Discord">
        let c_users_appdata_local_discord = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\Discord\\*.log",
            program: "Discord".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_local_discord);
        let c_users_appdata_local_discord_logs = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\Discord\\*.log",
            program: "Discord".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_local_discord_logs);
        let c_users_appdata_roaming_discord_logs = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\discord\\logs\\*",
            program: "Discord".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_roaming_discord_logs);
        //</editor-fold>
        //<editor-fold desc="Guilded">
        let c_users_appdata_roaming_guilded = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\Guilded\\*.log",
            program: "Guilded".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_roaming_guilded);
        //</editor-fold>
        //<editor-fold desc="Element">
        let c_users_appdata_local_element_desktop = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\element-desktop\\*.log",
            program: "Element".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_local_element_desktop);
        //</editor-fold>
        //<editor-fold desc="Telegram">
        let c_users_appdata_roaming_telefram_desktop_tdata = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\Telegram Desktop\\tdata",
            program: "Telegram".parse().unwrap(),
            files_to_remove: vec![
                "key_datas".parse().unwrap()
            ],
            category: "Accounts".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_roaming_telefram_desktop_tdata);
        let c_users_appdata_roaming_telefram_desktop = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\Telegram Desktop\\*.txt",
            program: "Telegram".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_roaming_telefram_desktop);
        let c_users_appdata_roaming_telefram_desktop_logs = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\Telegram Desktop\\*.log",
            program: "Telegram".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_roaming_telefram_desktop_logs);
        let c_users_appdata_roaming_telefram_desktop_tdata_emoji_cache = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\Telegram Desktop\\tdata\\emoji\\*cache_*",
            program: "Telegram".parse().unwrap(),
            files_to_remove: vec![],
            category: String::from("Cache"),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_roaming_telefram_desktop_tdata_emoji_cache);
        let c_users_appdata_roaming_telefram_desktop_tdata_user_data_cache = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\Telegram Desktop\\tdata\\user_data\\cache\\**\\*",
            program: "Telegram".parse().unwrap(),
            files_to_remove: vec![],
            category: String::from("Cache"),
            remove_directories: true,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_roaming_telefram_desktop_tdata_user_data_cache);
        let c_users_appdata_roaming_telefram_desktop_tdata_user_data_media_cache = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\Telegram Desktop\\tdata\\user_data\\media_cache\\**\\*",
            program: "Telegram".parse().unwrap(),
            files_to_remove: vec![],
            category: String::from("Cache"),
            remove_directories: true,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_roaming_telefram_desktop_tdata_user_data_media_cache);
        //</editor-fold>
        //<editor-fold desc="Signal">
        let c_users_appdata_roaming_signal = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\Signal\\logs\\*",
            program: "Signal".parse().unwrap(),
            files_to_remove: vec![ ],
            category: "Logs".parse().unwrap(),
            remove_directories: true,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_roaming_signal);
        let c_users_appdata_roaming_signal_update_cache = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\Signal\\update-cache\\*",
            program: "Signal".parse().unwrap(),
            files_to_remove: vec![ ],
            category: String::from("Cache"),
            remove_directories: true,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_roaming_signal_update_cache);
        //</editor-fold>

        //</editor-fold>
        //<editor-fold desc="VPN Clients">

        //<editor-fold desc="Amnezia VPN">
        let c_program_files_amnezia_vpn = CleanerData {
            path: drive.to_owned() + "Program Files\\AmneziaVPN",
            program: "Amnezia VPN".parse().unwrap(),
            files_to_remove: vec![
                "InstallationLog.txt".parse().unwrap()
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: true,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_amnezia_vpn);
        let c_program_files_amnezia_vpn_tap = CleanerData {
            path: drive.to_owned() + "Program Files\\AmneziaVPN\\tap",
            program: "Amnezia VPN".parse().unwrap(),
            files_to_remove: vec![
                "license.txt".parse().unwrap()
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: true,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_amnezia_vpn_tap);
        //</editor-fold>
        //<editor-fold desc="Radmin VPN">
        let c_program_filex_x86_radmin_vpn_chatlogs = CleanerData {
            path: drive.to_owned() + "Program Files (x86)\\Radmin VPN\\CHATLOGS\\*",
            program: "Radmin VPN".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: true,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_filex_x86_radmin_vpn_chatlogs);
        let c_program_files_radmin_vpn = CleanerData {
            path: drive.to_owned() + "ProgramData\\Famatech\\Radmin VPN\\*.txt",
            program: "Radmin VPN".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_radmin_vpn);
        let c_program_files_radmin_vpn_logs = CleanerData {
            path: drive.to_owned() + "ProgramData\\Famatech\\Radmin VPN\\*.log",
            program: "Radmin VPN".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_radmin_vpn_logs);
        let program_files_x86_radmin_vpn = CleanerData {
            path: drive.to_owned() + "Program Files (x86)\\Radmin VPN",
            program: "Radmin VPN".parse().unwrap(),
            files_to_remove: vec![
                "eula.txt".parse().unwrap()
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(program_files_x86_radmin_vpn);
        //</editor-fold>
        //<editor-fold desc="UrbanVPN">
        let c_users_urbanvpm_logs = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\UrbanVPN\\log\\*",
            program: "UrbanVPN".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: true,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_urbanvpm_logs);
        //</editor-fold>
        //<editor-fold desc="CloudFlare">
        let c_users_urbanvpm_logs = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\Cloudflare\\*.log",
            program: "CloudFlare".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_urbanvpm_logs);
        //</editor-fold>
        //<editor-fold desc="PlanetVPN">
        let c_users_appdata_local_planetvpn_cache_qmlcache = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\PlanetVPN\\cache\\qmlcache\\*",
            program: "PlanetVPN".parse().unwrap(),
            files_to_remove: vec![],
            category: String::from("Cache"),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_local_planetvpn_cache_qmlcache);
        //</editor-fold>
        //<editor-fold desc="iTop VPN">
        let c_programdata_itop_vpn = CleanerData {
            path: drive.to_owned() + "ProgramData\\iTop VPN",
            program: "iTop VPN".parse().unwrap(),
            files_to_remove: vec![
                "iTop_setup.log".parse().unwrap(),
                "Setup.log".parse().unwrap()
            ],
            category: String::from("Cache"),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_programdata_itop_vpn);
        //</editor-fold>

        //</editor-fold>
        //<editor-fold desc="Images">

        //<editor-fold desc="ImageGlass">
        let c_program_files_imageglass = CleanerData {
            path: drive.to_owned() + "Program Files\\ImageGlass",
            program: "ImageGlass".parse().unwrap(),
            files_to_remove: vec![
                "ReadMe.rtf".parse().unwrap(),
                "CliWrap.xml".parse().unwrap(),
                "DotNetZip.pdb".parse().unwrap(),
                "DotNetZip.xml".parse().unwrap(),
                "ImageGlass.ImageBox.xml".parse().unwrap(),
                "ImageGlass.ImageListView.xml".parse().unwrap(),
                "LICENSE".parse().unwrap(),
                "Magick.NET.Core.xml".parse().unwrap(),
                "Magick.NET.SystemDrawing.xml".parse().unwrap(),
                "Magick.NET-Q16-HDRI-OpenMP-x64.xml".parse().unwrap(),
                "Microsoft.Bcl.AsyncInterfaces.xml".parse().unwrap(),
                "System.Buffers.xml".parse().unwrap(),
                "System.Memory.xml".parse().unwrap(),
                "System.Numerics.Vectors.xml".parse().unwrap(),
                "System.Runtime.CompilerServices.Unsafe.xml".parse().unwrap(),
                "System.Text.Encodings.Web.xml".parse().unwrap(),
                "System.Text.Json.xml".parse().unwrap(),
                "System.Threading.Tasks.Extensions.xml".parse().unwrap(),
                "System.ValueTuple.xml".parse().unwrap(),"".parse().unwrap(),
                "ImageGlass.WebP.pdb".parse().unwrap(),
                "Visit ImageGlass website.url".parse().unwrap(),
                "default.jpg".parse().unwrap()

            ],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_imageglass);
        let c_users_appdata_local_imageglass_thumbails_cache = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\ImageGlass\\ThumbnailsCache\\*",
            program: "ImageGlass".parse().unwrap(),
            files_to_remove: vec![],
            category: "Cache".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_local_imageglass_thumbails_cache);
        let c_users_appdata_local_imageglass_thumbails_cache = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Local\\ImageGlass\\ThumbnailsCache\\*",
            program: "ImageGlass".parse().unwrap(),
            files_to_remove: vec![],
            category: "Cache".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_local_imageglass_thumbails_cache);
        let program_files_imageglass_license = CleanerData {
            path: drive.to_owned() + "Program Files\\ImageGlass\\License\\*",
            program: "ImageGlass".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(program_files_imageglass_license);
        //</editor-fold>
        //<editor-fold desc="InkSpace">
        let c_program_files_inkscape = CleanerData {
            path: drive.to_owned() + "Program Files\\Inkscape",
            program: "InkSpace".parse().unwrap(),
            files_to_remove: vec![
                "NEWS.md".parse().unwrap(),
                "README.md".parse().unwrap()
            ],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_program_files_inkscape);
        let c_users_appdata_roaming_inkscape = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\inkscape\\*.log",
            program: "InkSpace".parse().unwrap(),
            files_to_remove: vec![ ],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false,
        };
        database.push(c_users_appdata_roaming_inkscape);
        //</editor-fold>
        //<editor-fold desc="ShareX">
        let sharex_1 = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\Documents\\ShareX\\Screenshots\\**\\*.jpg",
            program: "ShareX".parse().unwrap(),
            files_to_remove: vec![],
            category: "Images".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(sharex_1);
        let c_users_documents_sharex_screenshots = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\Documents\\ShareX\\Screenshots\\**\\*.png",
            program: "ShareX".parse().unwrap(),
            files_to_remove: vec![],
            category: "Images".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(c_users_documents_sharex_screenshots);
        let c_users_documents_sharex_logs = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\Documents\\ShareX\\Logs\\*",
            program: "ShareX".parse().unwrap(),
            files_to_remove: vec![],
            category: "Logs".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(c_users_documents_sharex_logs);
        let c_users_documents_sharex_backups = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\Documents\\ShareX\\Backup\\*",
            program: "ShareX".parse().unwrap(),
            files_to_remove: vec![],
            category: "Backups".parse().unwrap(),
            remove_directories: false,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: false,
            remove_directory_after_clean: false
        };
        database.push(c_users_documents_sharex_backups);
        //</editor-fold>

        //</editor-fold>
        //<editor-fold desc="Cheats">

        //<editor-fold desc="Weave">
        let c_weave = CleanerData {
            path: drive.to_owned() + "Weave",
            program: "Weave".parse().unwrap(),
            files_to_remove: vec![],
            category: "Cheats".parse().unwrap(),
            remove_directories: true,
            remove_files: true,
            directories_to_remove: vec![],
            remove_all_in_dir: true,
            remove_directory_after_clean: true
        };
        database.push(c_weave);
        //</editor-fold>
        //<editor-fold desc="INTERIUM">
        let interium = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\INTERIUM",
            program: "INTERIUM".parse().unwrap(),
            files_to_remove: vec![],
            category: "Cheats".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: true,
            remove_directory_after_clean: true
        };
        database.push(interium);
        //</editor-fold>
        //<editor-fold desc="Krnl">
        let krnl = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\Krnl",
            program: "Krnl".parse().unwrap(),
            files_to_remove: vec![],
            category: "Cheats".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: true,
            remove_directory_after_clean: true
        };
        database.push(krnl);
        //</editor-fold>
        //<editor-fold desc="ExecHack">
        let krnl = CleanerData {
            path: drive.to_owned() + "exechack",
            program: "Krnl".parse().unwrap(),
            files_to_remove: vec![],
            category: "Cheats".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: true,
            remove_directory_after_clean: true
        };
        database.push(krnl);
        //</editor-fold>
        //<editor-fold desc="Vape Client">
        let vapeclient = CleanerData {
            path: drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\.vapeclient",
            program: "Vape Client".parse().unwrap(),
            files_to_remove: vec![],
            category: "Cheats".parse().unwrap(),
            remove_directories: false,
            remove_files: false,
            directories_to_remove: vec![],
            remove_all_in_dir: true,
            remove_directory_after_clean: true
        };
        database.push(vapeclient);
        //</editor-fold>

        //</editor-fold>
    }

    //<editor-fold desc="Cheats">

    //<editor-fold desc="Weave">
    let steam_common_counter_string_global_offensive_weave = CleanerData {
        path: steam_directory.clone() + "\\steamapps\\common\\Counter-Strike Global Offensive\\weave",
        program: "Weave".parse().unwrap(),
        files_to_remove: vec![],
        category: "Cheats".parse().unwrap(),
        remove_directories: false,
        remove_files: false,
        directories_to_remove: vec![],
        remove_all_in_dir: true,
        remove_directory_after_clean: true
    };
    database.push(steam_common_counter_string_global_offensive_weave);
    //</editor-fold>
    //<editor-fold desc="Fatality">
    let steam_common_counter_string_global_offensive = CleanerData {
        path: steam_directory.clone() + "\\steamapps\\common\\Counter-Strike Global Offensive",
        program: "Fatality".parse().unwrap(),
        files_to_remove: vec![
            "slot1".parse().unwrap(),
            "slot2".parse().unwrap(),
            "slot3".parse().unwrap(),
            "slot4".parse().unwrap(),
            "skins".parse().unwrap(),
            "flog.log".parse().unwrap()
        ],
        category: "Cheats".parse().unwrap(),
        remove_directories: false,
        remove_files: false,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(steam_common_counter_string_global_offensive);
    let steam_common_counter_string_global_offensive_fatality = CleanerData {
        path: steam_directory.clone() + "\\steamapps\\common\\Counter-Strike Global Offensive\\fatality\\*",
        program: "Fatality".parse().unwrap(),
        files_to_remove: vec![],
        category: "Cheats".parse().unwrap(),
        remove_directories: true,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: true
    };
    database.push(steam_common_counter_string_global_offensive_fatality);
    //</editor-fold>
    //<editor-fold desc="Pandora">
    let steam_common_counter_string_global_offensive_pdr = CleanerData {
        path: steam_directory.clone() + "\\steamapps\\common\\Counter-Strike Global Offensive\\*.pdr",
        program: "Pandora".parse().unwrap(),
        files_to_remove: vec![],
        category: "Cheats".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(steam_common_counter_string_global_offensive_pdr);
    let steam_common_counter_string_global_offensive_pandora = CleanerData {
        path: steam_directory.clone() + "\\steamapps\\common\\Counter-Strike Global Offensive\\Pandora",
        program: "Pandora".parse().unwrap(),
        files_to_remove: vec![],
        category: "Cheats".parse().unwrap(),
        remove_directories: true,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: true
    };
    database.push(steam_common_counter_string_global_offensive_pandora);
    //</editor-fold>
    //<editor-fold desc="OneTap">
    let steam_common_counter_string_global_offensive_ot = CleanerData {
        path: steam_directory.clone() + "\\steamapps\\common\\Counter-Strike Global Offensive\\ot",
        program: "OneTap".parse().unwrap(),
        files_to_remove: vec![],
        category: "Cheats".parse().unwrap(),
        remove_directories: false,
        remove_files: false,
        directories_to_remove: vec![],
        remove_all_in_dir: true,
        remove_directory_after_clean: true
    };
    database.push(steam_common_counter_string_global_offensive_ot);
    //</editor-fold>

    //</editor-fold>
    //<editor-fold desc="Steam games">

    //<editor-fold desc="Counter-Strike Global Offensive">
    let steam_userdata_730_local_cfg = CleanerData {
        path: steam_directory.clone() + "\\userdata\\**\\730\\local\\cfg\\*",
        program: "Counter-Strike Global Offensive".parse().unwrap(),
        files_to_remove: vec![],
        category: "Game settings".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(steam_userdata_730_local_cfg);
    //</editor-fold>
    //<editor-fold desc="Dota 2">
    let steam_userdata_570_local_cfg = CleanerData {
        path: steam_directory.clone() + "\\userdata\\**\\570\\local\\cfg\\*",
        program: "Dota 2".parse().unwrap(),
        files_to_remove: vec![],
        category: "Game settings".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(steam_userdata_570_local_cfg);
    //</editor-fold>
    //<editor-fold desc="Rust">
    let steam_userdata_252490_local_cfg = CleanerData {
        path: steam_directory.clone() + "\\userdata\\**\\252490\\local\\cfg\\*",
        program: "Rust".parse().unwrap(),
        files_to_remove: vec![],
        category: "Game settings".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(steam_userdata_252490_local_cfg);
    //</editor-fold>
    //<editor-fold desc="Unturned">
    let steam_userdata_252490_local_cfg = CleanerData {
        path: steam_directory.clone() + "\\userdata\\**\\304930\\local\\cfg\\*",
        program: "Unturned".parse().unwrap(),
        files_to_remove: vec![],
        category: "Game settings".parse().unwrap(),
        remove_directories: false,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(steam_userdata_252490_local_cfg);
    //</editor-fold>

    //</editor-fold>

    //<editor-fold desc="Steam">
    let steam_userdata = CleanerData {
        path: steam_directory.clone() + "\\userdata\\**",
        program: "Steam".parse().unwrap(),
        files_to_remove: vec![],
        category: "Accounts".parse().unwrap(),
        remove_directories: true,
        remove_files: true,
        directories_to_remove: vec![],
        remove_all_in_dir: false,
        remove_directory_after_clean: false
    };
    database.push(steam_userdata);

    //</editor-fold>

    database.sort_by(|a, b| a.category.cmp(&b.category));

    database
};
}

pub fn get_database() -> &'static Vec<CleanerData> {
    &DATABASE
}
