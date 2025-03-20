#[cfg(windows)]
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
#[cfg(windows)]
use winreg::RegKey;
#[cfg(windows)]
use crate::registry_utils::{remove_all_in_registry, remove_all_in_tree_in_registry};

#[cfg(windows)]
pub fn clear_last_activity() -> String {
    let mut result = String::new();
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    let hkcu_link = &hkcu;
    let hklm_link = &hklm;


    software_microsoft_windows_current_version_explorer_type_paths(hkcu_link);
    software_microsoft_windows_current_version_explorer_feature_usage_show_jump_view(hkcu_link);
    software_microsoft_windows_nt_current_version_app_compat_flags_compatibility_assistant_store(hkcu_link);
    software_classes_local_settings_software_microsoft_windows_shell_mui_cache(hkcu_link);
    software_classes_local_settings_software_microsoft_windows_shell_bags(hkcu_link);
    software_classes_local_settings_software_microsoft_windows_shell_bag_mru(hkcu_link);
    software_microsoft_windows_current_version_explorer_com_dlg32(hkcu_link);
    software_microsoft_windows_current_version_explorer_app_switched(hkcu_link);
    software_microsoft_windows_current_version_explorer_recent_docs(hkcu_link);

    system_contolset_services_bam_state_user_settings(hklm_link);

    result
}

#[cfg(windows)]
fn system_contolset_services_bam_state_user_settings(hkcu: &RegKey) {
    let path = String::from("SYSTEM\\ControlSet001\\Services\\bam\\State\\UserSettings");
    remove_all_in_tree_in_registry(hkcu, path);
}




#[cfg(windows)]
fn software_microsoft_windows_current_version_explorer_recent_docs(hkcu: &RegKey) {
    let path = String::from("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Explorer\\RecentDocs");
    remove_all_in_registry(hkcu, path);
}
#[cfg(windows)]
fn software_microsoft_windows_current_version_explorer_app_switched(hkcu: &RegKey) {
    let path = String::from("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Explorer\\FeatureUsage\\AppSwitched");
    remove_all_in_registry(hkcu, path);
}
#[cfg(windows)]
fn software_microsoft_windows_current_version_explorer_com_dlg32(hkcu: &RegKey) {
    let path = String::from("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Explorer\\ComDlg32");
    remove_all_in_tree_in_registry(hkcu, path);
}
#[cfg(windows)]
fn software_classes_local_settings_software_microsoft_windows_shell_bag_mru(hkcu: &RegKey) {
    let path = String::from("SOFTWARE\\Classes\\Local Settings\\Software\\Microsoft\\Windows\\Shell\\BagMRU");
    remove_all_in_registry(hkcu, path);
}
#[cfg(windows)]
fn software_classes_local_settings_software_microsoft_windows_shell_bags(hkcu: &RegKey) {
    let path = String::from("SOFTWARE\\Classes\\Local Settings\\Software\\Microsoft\\Windows\\Shell\\Bags");
    remove_all_in_tree_in_registry(hkcu, path);
}
#[cfg(windows)]
fn software_classes_local_settings_software_microsoft_windows_shell_mui_cache(hkcu: &RegKey) {
    let path = String::from("SOFTWARE\\Classes\\Local Settings\\Software\\Microsoft\\Windows\\Shell\\MuiCache");
    remove_all_in_registry(hkcu, path);
}
#[cfg(windows)]
fn software_microsoft_windows_nt_current_version_app_compat_flags_compatibility_assistant_store(hkcu: &RegKey) {
    let path = String::from("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\AppCompatFlags\\Compatibility Assistant\\Store");
    remove_all_in_registry(hkcu, path);
}
#[cfg(windows)]
fn software_microsoft_windows_current_version_explorer_feature_usage_show_jump_view(hkcu: &RegKey) {
    let path = String::from("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Explorer\\FeatureUsage\\ShowJumpView");
    remove_all_in_registry(hkcu, path);
}
#[cfg(windows)]
fn software_microsoft_windows_current_version_explorer_type_paths(hkcu: &RegKey) {
    let path = String::from("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Explorer\\TypedPaths");
    remove_all_in_registry(hkcu, path);
}