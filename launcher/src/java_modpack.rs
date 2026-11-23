//! Importing a Modrinth modpack (`.mrpack`) into a profile's Java Bridge mods folder. Only the
//! pack's Fabric mods are taken (`mods/*.jar`, downloaded and checked against the checksums the
//! pack lists, plus any jars in its `overrides/mods`); resource packs, configs and worlds in the
//! pack are left alone. A pack for another Minecraft version, or for another loader, is refused
//! before anything is downloaded.
use crate::browser::{modrinth::Modrinth, FileRef, Source};
use crate::java_mods::{dir, MINECRAFT};
use serde_json::Value;
use std::io::Read;
use std::path::Path;

/// The largest index or jar read out of a pack.
const INDEX_MAX: u64 = 8 << 20;
const JAR_MAX: u64 = 64 << 20;

#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub name: String,
    /// Mods to download.
    pub files: Vec<FileRef>,
    /// Jars shipped inside the pack, by their name in the zip.
    pub bundled: Vec<String>,
}

fn plain_jar_name(path: &str) -> Option<String> {
    let name = path.rsplit('/').next()?;
    (name.to_ascii_lowercase().ends_with(".jar")
        && !name.contains('\\')
        && !name.starts_with('.'))
    .then(|| name.to_string())
}

/// What to install from the pack's index and the entry names of its zip. Err with the reason
/// when the pack is not for the Java Bridge's Minecraft version and Fabric.
pub fn plan(index: &Value, entries: &[String]) -> Result<Plan, String> {
    if let Some(game) = index["game"].as_str() {
        if game != "minecraft" {
            return Err(format!("This pack is for {game}, not Minecraft."));
        }
    }
    let deps = index["dependencies"]
        .as_object()
        .ok_or("The pack lists no dependencies, so its Minecraft version is unknown.")?;
    let wants = deps
        .get("minecraft")
        .and_then(Value::as_str)
        .ok_or("The pack names no Minecraft version.")?;
    if wants != MINECRAFT {
        return Err(format!(
            "This pack is for Minecraft {wants}. The Java Bridge runs Minecraft {MINECRAFT}, so its mods would not load."
        ));
    }
    if !deps.contains_key("fabric-loader") {
        let other = ["quilt-loader", "neoforge", "forge"]
            .into_iter()
            .find(|k| deps.contains_key(*k))
            .unwrap_or("another loader");
        return Err(format!("This pack needs {other}. The Java Bridge runs Fabric mods only."));
    }
    let mut files = Vec::new();
    for f in index["files"].as_array().into_iter().flatten() {
        let Some(path) = f["path"].as_str() else {
            continue;
        };
        let in_mods = path.strip_prefix("mods/").is_some_and(|rest| !rest.contains('/'));
        let Some(name) = plain_jar_name(path).filter(|_| in_mods) else {
            continue;
        };
        if f["env"]["client"].as_str() == Some("unsupported") {
            continue;
        }
        let Some(url) = f["downloads"].as_array().and_then(|d| d.first()).and_then(Value::as_str) else {
            continue;
        };
        files.push(FileRef {
            url: url.to_string(),
            name,
            size: f["fileSize"].as_u64().unwrap_or(0),
            sha512: f["hashes"]["sha512"].as_str().map(str::to_string),
            sha256: None,
            sha1: f["hashes"]["sha1"].as_str().map(str::to_string),
            primary: true,
        });
    }
    let bundled = entries
        .iter()
        .filter(|e| {
            ["overrides/mods/", "client-overrides/mods/"]
                .iter()
                .any(|p| e.strip_prefix(p).is_some_and(|rest| !rest.contains('/')))
        })
        .filter(|e| plain_jar_name(e).is_some())
        .cloned()
        .collect();
    Ok(Plan {
        name: index["name"].as_str().unwrap_or("the pack").to_string(),
        files,
        bundled,
    })
}

/// Installs the pack's mods into `home`'s `mods-java/`; what was done, in words. A jar already
/// in the folder is kept as it is. Mods that fail to download are named in the error, and the
/// others stay installed.
pub fn install(home: &Path, pack: &Path) -> Result<String, String> {
    let file = std::fs::File::open(pack).map_err(|e| format!("{}: {e}", pack.display()))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|_| "This is not a .mrpack file.".to_string())?;
    let index: Value = {
        let entry = zip
            .by_name("modrinth.index.json")
            .map_err(|_| "This is not a Modrinth pack: it has no modrinth.index.json.".to_string())?;
        let mut text = String::new();
        entry
            .take(INDEX_MAX)
            .read_to_string(&mut text)
            .map_err(|e| format!("The pack's index could not be read: {e}"))?;
        serde_json::from_str(&text).map_err(|e| format!("The pack's index is not valid: {e}"))?
    };
    // zip 9 returns each entry name as Result<Cow<str>, ZipError>, not &str.
    // Collect the names before borrowing the archive again for the bundled JAR reads.
    let entries: Vec<String> = zip
        .file_names()
        .map(|name| name.map(|name| name.into_owned()))
        .collect::<Result<_, _>>()
        .map_err(|e| format!("The pack contains a ZIP entry whose name can't be read: {e}"))?;
    let plan = plan(&index, &entries)?;
    let folder = dir(home);
    std::fs::create_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;

    let (mut added, mut kept, mut failed) = (0, 0, Vec::new());
    for f in &plan.files {
        if folder.join(&f.name).exists() {
            kept += 1;
        } else {
            match Modrinth.fetch(f, &folder, &|_| {}) {
                Ok(_) => added += 1,
                Err(e) => failed.push(format!("{}: {e}", f.name)),
            }
        }
    }
    for name in &plan.bundled {
        let Some(jar) = plain_jar_name(name) else {
            continue;
        };
        let target = folder.join(&jar);
        if target.exists() {
            kept += 1;
            continue;
        }
        let result = zip
            .by_name(name)
            .map_err(|e| e.to_string())
            .and_then(|entry| {
                if entry.size() > JAR_MAX {
                    return Err("larger than any mod should be".to_string());
                }
                let mut bytes = Vec::new();
                entry.take(JAR_MAX).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
                std::fs::write(&target, bytes).map_err(|e| e.to_string())
            });
        match result {
            Ok(()) => added += 1,
            Err(e) => failed.push(format!("{jar}: {e}")),
        }
    }
    if failed.is_empty() {
        Ok(format!(
            "Imported {}: {added} mod(s) added, {kept} already there.",
            plan.name
        ))
    } else {
        Err(format!(
            "{} of {} mod(s) from {} failed ({added} added). First: {}",
            failed.len(),
            plan.files.len() + plan.bundled.len(),
            plan.name,
            failed[0]
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn index(deps: Value) -> Value {
        json!({
            "game": "minecraft",
            "name": "Pack",
            "dependencies": deps,
            "files": [
                {"path": "mods/a.jar", "hashes": {"sha1": "1", "sha512": "2"}, "downloads": ["https://cdn.modrinth.com/a.jar"], "fileSize": 5, "env": {"client": "required"}},
                {"path": "mods/server-only.jar", "hashes": {"sha1": "1"}, "downloads": ["https://cdn.modrinth.com/s.jar"], "env": {"client": "unsupported"}},
                {"path": "resourcepacks/r.zip", "hashes": {}, "downloads": ["https://cdn.modrinth.com/r.zip"]},
                {"path": "mods/sub/nested.jar", "hashes": {}, "downloads": ["https://cdn.modrinth.com/n.jar"]},
                {"path": "mods/no-download.jar", "hashes": {}, "downloads": []}
            ]
        })
    }

    #[test]
    fn only_client_fabric_mods_are_taken() {
        let deps = json!({"minecraft": MINECRAFT, "fabric-loader": "0.16"});
        let entries = vec![
            "overrides/mods/Bundled.jar".to_string(),
            "overrides/mods/deep/x.jar".to_string(),
            "overrides/config/c.json".to_string(),
            "client-overrides/mods/Client.JAR".to_string(),
        ];
        let plan = plan(&index(deps), &entries).unwrap();
        assert_eq!(plan.name, "Pack");
        assert_eq!(plan.files.len(), 1);
        assert_eq!(plan.files[0].name, "a.jar");
        assert_eq!(plan.files[0].sha512.as_deref(), Some("2"));
        assert_eq!(plan.bundled, ["overrides/mods/Bundled.jar", "client-overrides/mods/Client.JAR"]);
    }

    #[test]
    fn a_pack_for_another_minecraft_is_refused_with_both_versions() {
        let deps = json!({"minecraft": "1.0", "fabric-loader": "0.16"});
        let err = plan(&index(deps), &[]).unwrap_err();
        assert!(err.contains("1.0") && err.contains(MINECRAFT));
    }

    #[test]
    fn a_pack_for_another_loader_is_refused_by_name() {
        let deps = json!({"minecraft": MINECRAFT, "neoforge": "21"});
        assert!(plan(&index(deps), &[]).unwrap_err().contains("neoforge"));
        let deps = json!({"minecraft": MINECRAFT, "forge": "47"});
        assert!(plan(&index(deps), &[]).unwrap_err().contains("forge"));
    }

    #[test]
    fn a_pack_without_dependencies_is_refused() {
        assert!(plan(&json!({"game": "minecraft", "files": []}), &[]).is_err());
        assert!(plan(&json!({"game": "hytale", "dependencies": {}}), &[]).is_err());
    }
}
