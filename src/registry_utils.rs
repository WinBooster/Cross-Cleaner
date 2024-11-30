use winreg::enums::HKEY_CURRENT_USER;
use winreg::RegKey;

pub fn get_steam_directory_from_registry() -> String {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let software_valve_steam = hkcu.open_subkey("SOFTWARE\\Valve\\Steam");
    match software_valve_steam {
        Ok(software_valve_steam) => {
            software_valve_steam.get_value("SteamPath").unwrap_or_default()
        }
        Err(_) => { String::from("") }
    }
}