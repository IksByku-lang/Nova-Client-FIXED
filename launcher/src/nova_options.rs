//! Reading and changing a profile's `options.txt` (vanilla's `key:value` lines): the few video
//! options the Performance page offers. The game reads these keys at its start
//! (`horizon-client-render/src/options_file.rs`: `maxFps` 10-260 where 260 is unlimited,
//! `enableVsync`, `fullscreen`, `renderDistance` 2-128, `simulationDistance` 5-32). Every other
//! line of the file is kept as it was.
use std::path::Path;

pub const MAX_FPS: &str = "maxFps";
pub const VSYNC: &str = "enableVsync";
pub const FULLSCREEN: &str = "fullscreen";
pub const RENDER: &str = "renderDistance";
pub const SIMULATION: &str = "simulationDistance";

/// `maxFps` value that means no limit.
pub const FPS_UNLIMITED: i32 = 260;

/// The value of `key` in `text`, when the file has it.
pub fn get<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines()
        .find_map(|line| line.split_once(':').filter(|(k, _)| *k == key).map(|(_, v)| v.trim()))
}

pub fn get_int(text: &str, key: &str) -> Option<i32> {
    get(text, key)?.parse().ok()
}

pub fn get_bool(text: &str, key: &str) -> Option<bool> {
    match get(text, key)? {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// Whether `value` is one the game accepts for `key`.
pub fn check(key: &str, value: &str) -> Result<(), String> {
    let range = |lo: i32, hi: i32| match value.parse::<i32>() {
        Ok(v) if (lo..=hi).contains(&v) => Ok(()),
        _ => Err(format!("{key} takes a whole number from {lo} to {hi}.")),
    };
    match key {
        MAX_FPS => range(10, FPS_UNLIMITED),
        RENDER => range(2, 128),
        SIMULATION => range(5, 32),
        VSYNC | FULLSCREEN => match value {
            "true" | "false" => Ok(()),
            _ => Err(format!("{key} is true or false.")),
        },
        _ => Err(format!("{key} is not an option the launcher changes.")),
    }
}

/// `text` with `key` set to `value`: the line replaced in place when there, else added at the
/// end. Line endings and every other line stay as they were.
pub fn set(text: &str, key: &str, value: &str) -> Result<String, String> {
    check(key, value)?;
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut found = false;
    let mut lines: Vec<String> = text
        .lines()
        .map(|line| match line.split_once(':') {
            Some((k, _)) if k == key => {
                found = true;
                format!("{key}:{value}")
            }
            _ => line.to_string(),
        })
        .collect();
    if !found {
        lines.push(format!("{key}:{value}"));
    }
    Ok(lines.join(newline) + newline)
}

/// Changes `key` in the `options.txt` of the game folder `home`. The file must exist (the game
/// writes it the first time it runs); a copy of the original is kept once, as
/// `options.txt.nova-backup`, and the new text is written beside the file and renamed over it.
pub fn change(home: &Path, key: &str, value: &str) -> Result<(), String> {
    let file = home.join("options.txt");
    let text = std::fs::read_to_string(&file).map_err(|_| {
        "This profile has no options.txt yet. Start the game once so it creates its options, then change them here.".to_string()
    })?;
    let new = set(&text, key, value)?;
    let backup = home.join("options.txt.nova-backup");
    if !backup.exists() {
        std::fs::copy(&file, &backup).map_err(|e| format!("{}: {e}", backup.display()))?;
    }
    let part = home.join("options.txt.nova-part");
    std::fs::write(&part, new).map_err(|e| format!("{}: {e}", part.display()))?;
    std::fs::rename(&part, &file).map_err(|e| format!("{}: {e}", file.display()))
}

/// The distance presets: render and simulation distance for Low, Balanced and High.
pub fn distances(preset: &str) -> Option<(i32, i32)> {
    match preset {
        "Low" => Some((8, 6)),
        "Balanced" => Some((12, 12)),
        "High" => Some((20, 16)),
        _ => None,
    }
}

/// The preset a pair of distances matches; Custom for any other pair.
pub fn detect(render: i32, simulation: i32) -> &'static str {
    ["Low", "Balanced", "High"]
        .into_iter()
        .find(|p| distances(p) == Some((render, simulation)))
        .unwrap_or("Custom")
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "version:4000\nmaxFps:120\nrenderDistance:12\nlang:en_us\n";

    #[test]
    fn it_reads_values() {
        assert_eq!(get_int(FILE, MAX_FPS), Some(120));
        assert_eq!(get_int(FILE, SIMULATION), None);
        assert_eq!(get_bool("enableVsync:false\n", VSYNC), Some(false));
        assert_eq!(get_bool("enableVsync:maybe\n", VSYNC), None);
    }

    #[test]
    fn it_replaces_in_place_and_keeps_other_lines() {
        let out = set(FILE, MAX_FPS, "60").unwrap();
        assert_eq!(out, "version:4000\nmaxFps:60\nrenderDistance:12\nlang:en_us\n");
    }

    #[test]
    fn it_adds_a_missing_key_and_keeps_windows_line_endings() {
        let out = set("a:1\r\nb:2\r\n", FULLSCREEN, "true").unwrap();
        assert_eq!(out, "a:1\r\nb:2\r\nfullscreen:true\r\n");
    }

    #[test]
    fn values_the_game_would_refuse_are_refused_here() {
        assert!(set(FILE, MAX_FPS, "5").is_err());
        assert!(set(FILE, MAX_FPS, "261").is_err());
        assert!(set(FILE, MAX_FPS, "fast").is_err());
        assert!(set(FILE, RENDER, "129").is_err());
        assert!(set(FILE, SIMULATION, "4").is_err());
        assert!(set(FILE, VSYNC, "yes").is_err());
        assert!(set(FILE, "gamma", "1").is_err());
        assert!(set(FILE, FPS_UNLIMITED.to_string().as_str(), "1").is_err());
    }

    #[test]
    fn changing_a_real_file_keeps_a_backup_and_needs_the_file() {
        let home = std::env::temp_dir().join(format!("nova-options-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        assert!(change(&home, MAX_FPS, "60").unwrap_err().contains("Start the game once"));
        std::fs::write(home.join("options.txt"), FILE).unwrap();
        change(&home, MAX_FPS, "60").unwrap();
        change(&home, RENDER, "16").unwrap();
        let now = std::fs::read_to_string(home.join("options.txt")).unwrap();
        assert_eq!(get_int(&now, MAX_FPS), Some(60));
        assert_eq!(get_int(&now, RENDER), Some(16));
        assert_eq!(get(&now, "lang"), Some("en_us"));
        // The backup is the file as the game left it, kept once.
        let backup = std::fs::read_to_string(home.join("options.txt.nova-backup")).unwrap();
        assert_eq!(backup, FILE);
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn presets_round_trip() {
        assert_eq!(detect(8, 6), "Low");
        assert_eq!(detect(12, 12), "Balanced");
        assert_eq!(detect(20, 16), "High");
        assert_eq!(detect(10, 10), "Custom");
    }
}
