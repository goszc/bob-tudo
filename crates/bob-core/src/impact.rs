pub fn impact(name: &str, command: &str) -> &'static str {
    let text = format!("{name} {command}").to_lowercase();
    const HIGH: &[&str] = &[
        "steam",
        "discord",
        "spotify",
        "onedrive",
        "adobe",
        "acrobat",
        "teams",
        "epic",
        "battle.net",
        "dropbox",
        "icloud",
        "cortana",
        "copilot",
        "skype",
        "slack",
        "zoom",
        "chrome",
        "msedge",
        "brave",
        "nvidia",
        "riot",
        "ubisoft",
        "ea desktop",
        "origin",
    ];
    const MID: &[&str] = &["update", "updater", "helper", "agent", "assistant", "sync"];
    if HIGH.iter().any(|key| text.contains(key)) {
        "alto"
    } else if MID.iter().any(|key| text.contains(key)) {
        "médio"
    } else {
        "baixo"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates_known_apps() {
        assert_eq!(impact("Spotify", "spotify.exe"), "alto");
        assert_eq!(impact("updater", "helper.exe"), "médio");
        assert_eq!(impact("notepad", "notepad.exe"), "baixo");
    }
}
