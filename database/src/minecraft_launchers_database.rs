#[cfg(unix)]
pub fn get_minecraft_launchers_instances_folders(username: &str) -> Vec<(String, String)> {
    let mut database = Vec::new();

    database.push((
        String::from("/home/".to_owned() + username + "/.minecraft"),
        String::from("Minecraft"),
    ));
    database.push((
        String::from(
            "/home/".to_owned() + username + "/.local/share/MultiMC/instances/**/.minecraft",
        ),
        String::from("MultiMC"),
    ));
    database.push((
        String::from(
            "/home/".to_owned() + username + "/.local/share/PolyMC/instances/**/.minecraft",
        ),
        String::from("PolyMC"),
    ));
    database.push((
        String::from(
            "/home/".to_owned() + username + "/.local/share/PrismLauncher/instances/**/minecraft",
        ),
        String::from("Prism Launcher"),
    ));
    database.push((
        String::from("/home/".to_owned() + username + "/.lunarclient/offline/multiver"),
        String::from("Lunar Client"),
    ));
    database.push((
        String::from("/home/".to_owned() + username + "/.cristalix/updates/**"),
        String::from("Cristalix"),
    ));
    database.push((
        String::from("/home/".to_owned() + username + "/cubixworld/updates/**"),
        String::from("CubixWorld"),
    ));

    database
}
#[cfg(unix)]
pub fn get_minecraft_launchers_folders(username: &str) -> Vec<(String, String)> {
    let mut database = Vec::new();

    database.push((
        String::from("/home/".to_owned() + username + "/.minecraft"),
        String::from("Minecraft"),
    ));
    database.push((
        String::from("/home/".to_owned() + username + "/.local/share/MultiMC"),
        String::from("MultiMC"),
    ));
    database.push((
        String::from("/home/".to_owned() + username + "/.local/share/PolyMC"),
        String::from("PolyMC"),
    ));
    database.push((
        String::from("/home/".to_owned() + username + "/.local/share/PrismLauncher"),
        String::from("Prism Launcher"),
    ));
    database.push((
        String::from("/home/".to_owned() + username + "/.lunarclient"),
        String::from("Lunar Client"),
    ));

    database
}

#[cfg(windows)]
pub fn get_minecraft_launchers_instances_folders(
    drive: &str,
    username: &str,
) -> Vec<(String, String)> {
    let mut database = Vec::new();

    database.push((
        String::from(drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\.minecraft"),
        String::from("Minecraft"),
    ));
    database.push((
        String::from(
            drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\Badlion Client",
        ),
        String::from("Badlion Client"),
    ));
    database.push((
        String::from(
            drive.to_owned() + "Users\\" + username + "\\.tecknixsoftware\\tecknixlauncher",
        ),
        String::from("Tecknix Client"),
    ));
    database.push((
        String::from(drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\ModrinthApp"),
        String::from("Modrinth"),
    ));
    database.push((
        String::from(
            drive.to_owned()
                + "Users\\"
                + username
                + "\\AppData\\Roaming\\MultiMC\\instances\\**\\minecraft",
        ),
        String::from("MultiMC"),
    ));
    database.push((
        String::from(
            drive.to_owned()
                + "Users\\"
                + username
                + "\\AppData\\Roaming\\PrismLauncher\\instances\\**\\minecraft",
        ),
        String::from("PrismLauncher"),
    ));
    database.push((
        String::from(
            drive.to_owned()
                + "Users\\"
                + username
                + "\\AppData\\Roaming\\PrismLauncher\\instances\\**\\.minecraft",
        ),
        String::from("PrismLauncher"),
    ));
    database.push((
        String::from(
            drive.to_owned()
                + "Users\\"
                + username
                + "\\AppData\\Roaming\\PolyMC\\instances\\**\\minecraft",
        ),
        String::from("PolyMC"),
    ));
    database.push((
        String::from(
            drive.to_owned()
                + "Users\\"
                + username
                + "\\AppData\\Roaming\\PolyMC\\instances\\**\\.minecraft",
        ),
        String::from("PolyMC"),
    ));
    database.push((
        String::from(
            drive.to_owned()
                + "Users\\"
                + username
                + "\\AppData\\Roaming\\ATLauncher\\instances\\**",
        ),
        String::from("ATLauncher"),
    ));
    database.push((
        String::from(drive.to_owned() + ".loliland\\updates\\clients\\**"),
        String::from("LoliLand"),
    ));
    database.push((
        String::from(drive.to_owned() + "Users\\" + username + "\\.cristalix\\updates\\**"),
        String::from("Cristalix"),
    ));
    database.push((
        String::from(
            drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\MCSkill\\updates\\**",
        ),
        String::from("MCSkill"),
    ));
    database.push((
        String::from(drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\.gribland\\**"),
        String::from("GribLand"),
    ));
    database.push((
        String::from(
            drive.to_owned() + "Users\\" + username + "\\curseforge\\minecraft\\Instances\\**",
        ),
        String::from("CurseForge"),
    ));

    database
}

#[cfg(windows)]
pub fn get_minecraft_launchers_folders(drive: &str, username: &str) -> Vec<(String, String)> {
    let mut database = Vec::new();
    database.push((
        String::from(
            drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\Badlion Client",
        ),
        String::from("Badlion Client"),
    ));
    database.push((
        String::from(
            drive.to_owned() + "Users\\" + username + "\\.tecknixsoftware\\tecknixlauncher",
        ),
        String::from("Tecknix Client"),
    ));
    database.push((
        String::from(drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\ModrinthApp"),
        String::from("Modrinth"),
    ));
    database.push((
        String::from(drive.to_owned() + "Users\\" + username + "\\.lunarclient"),
        String::from("Lunar Client"),
    ));
    database.push((
        String::from(drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\MultiMC"),
        String::from("MultiMC"),
    ));
    database.push((
        String::from(drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\PrismLauncher"),
        String::from("PrismLauncher"),
    ));
    database.push((
        String::from(drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\PolyMC"),
        String::from("PolyMC"),
    ));
    database.push((
        String::from(drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\ATLauncher"),
        String::from("ATLauncher"),
    ));
    database.push((
        String::from(
            drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\.gribland\\bin",
        ),
        String::from("GribLand"),
    ));
    database.push((
        String::from(drive.to_owned() + "Users\\" + username + "\\AppData\\Roaming\\.gribland"),
        String::from("GribLand"),
    ));
    database.push((
        String::from(drive.to_owned() + "Users\\" + username + "\\curseforge\\minecraft\\Install"),
        String::from("CurseForge"),
    ));

    database
}
