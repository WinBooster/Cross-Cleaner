#[cfg(unix)]
pub fn get_minecraft_launchers_instances_folders(username: &str) -> Vec<(String, String)> {
    let mut database = Vec::new();

    database.push((String::from("/home/".to_owned() + username + "/.minecraft"), String::from("Minecraft")));
    database.push((String::from("/home/".to_owned() + username + "/.local/share/MultiMC/instances/**/.minecraft"), String::from("MultiMC")));
    database.push((String::from("/home/".to_owned() + username + "/.local/share/PolyMC/instances/**/.minecraft"), String::from("PolyMC")));
    database.push((String::from("/home/".to_owned() + username + "/.local/share/PrismLauncher/instances/**/minecraft"), String::from("Prism Launcher")));

    database
}
#[cfg(unix)]
pub fn get_minecraft_launchers_folders(username: &str) -> Vec<(String, String)> {
    let mut database = Vec::new();

    database.push((String::from("/home/".to_owned() + username + "/.minecraft"), String::from("Minecraft")));
    database.push((String::from("/home/".to_owned() + username + "/.local/share/MultiMC"), String::from("MultiMC")));
    database.push((String::from("/home/".to_owned() + username + "/.local/share/PolyMC"), String::from("PolyMC")));
    database.push((String::from("/home/".to_owned() + username + "/.local/share/PrismLauncher"), String::from("Prism Launcher")));

    database
}

#[cfg(windows)]
pub fn get_minecraft_launchers_instances_folders(drive: &str, username: &str) -> Vec<(String, String)> {
    let mut database = Vec::new();

    database.push((String::from(drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\.minecraft"), String::from("Minecraft")));
    database.push((String::from(drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\Badlion Client"), String::from("Badlion Client")));
    database.push((String::from(drive.to_owned() + "Users\\" + username + "\\.tecknixsoftware\\tecknixlauncher"), String::from("Tecknix Client")));
    database.push((String::from(drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\ModrinthApp"), String::from("Modrinth")));
    database
}

#[cfg(windows)]
pub fn get_minecraft_launchers_folders(drive: &str, username: &str) -> Vec<(String, String)> {
    let mut database = Vec::new();
    database.push((String::from(drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\Badlion Client"), String::from("Badlion Client")));
    database.push((String::from(drive.to_owned() + "Users\\" + username + "\\.tecknixsoftware\\tecknixlauncher"), String::from("Tecknix Client")));
    database.push((String::from(drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\ModrinthApp"), String::from("Modrinth")));
    database.push((String::from(drive.to_owned() + "Users\\" + username + "\\.lunarclient"), String::from("Lunar Client")));
    database.push((String::from(drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\MultiMC"), String::from("MultiMC")));

    database
}