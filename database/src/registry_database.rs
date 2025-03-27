#[cfg(windows)]
use crate::registry_utils::{remove_all_in_registry, remove_all_in_tree_in_registry};
#[cfg(windows)]
use winreg::RegKey;
#[cfg(windows)]
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};

#[cfg(windows)]
pub fn clear_last_activity() -> u64 {
    let mut total_bytes = 0;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    // HKEY_CURRENT_USER paths
    let hkcu_paths = [
        (
            "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Explorer\\TypedPaths",
            false,
        ),
        (
            "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Explorer\\FeatureUsage\\ShowJumpView",
            false,
        ),
        (
            "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\AppCompatFlags\\Compatibility Assistant\\Store",
            false,
        ),
        (
            "SOFTWARE\\Classes\\Local Settings\\Software\\Microsoft\\Windows\\Shell\\MuiCache",
            false,
        ),
        (
            "SOFTWARE\\Classes\\Local Settings\\Software\\Microsoft\\Windows\\Shell\\Bags",
            true,
        ),
        (
            "SOFTWARE\\Classes\\Local Settings\\Software\\Microsoft\\Windows\\Shell\\BagMRU",
            false,
        ),
        (
            "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Explorer\\ComDlg32",
            true,
        ),
        (
            "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Explorer\\FeatureUsage\\AppSwitched",
            false,
        ),
        (
            "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Explorer\\RecentDocs",
            false,
        ),
    ];

    // Process HKEY_CURRENT_USER paths
    for (path, is_tree) in hkcu_paths {
        total_bytes += if is_tree {
            remove_all_in_tree_in_registry(&hkcu, path.to_string())
        } else {
            remove_all_in_registry(&hkcu, path.to_string())
        };
    }

    // Process HKEY_LOCAL_MACHINE path
    total_bytes += remove_all_in_tree_in_registry(
        &hklm,
        "SYSTEM\\ControlSet001\\Services\\bam\\State\\UserSettings".to_string(),
    );

    total_bytes
}
