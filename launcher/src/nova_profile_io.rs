//! Profile export and import. A profile file (`*.nova-profile.json`) holds a profile's settings:
//! its name, group, whether Fabric mods are on and, for a Minecraft: Java Edition profile, its
//! version, loader, memory and Java arguments. It holds no mod files, worlds or paths from the
//! computer it came from: the importer makes a new, empty profile with those settings, and the
//! mods are added again from the Mods page.
use horizon_launcher::instances::{self, Instance, JavaGame};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The file's format version, so a later format can be told apart.
const FORMAT: u32 = 1;
/// The most memory an imported profile may ask for, in MiB (1 TiB): a typo, not a setting.
const MAX_MEMORY_MB: u32 = 1 << 20;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileFile {
    pub nova_profile: u32,
    pub name: String,
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub java_mods: bool,
    #[serde(default)]
    pub java: Option<JavaGame>,
}

impl ProfileFile {
    pub fn of(instance: &Instance) -> ProfileFile {
        ProfileFile {
            nova_profile: FORMAT,
            name: instance.name.clone(),
            group: instance.group.clone(),
            java_mods: instance.java_mods,
            // The Java home is a path on this computer: never shared.
            java: instance.java.clone().map(|mut j| {
                j.home = None;
                j
            }),
        }
    }
}

pub fn export_text(instance: &Instance) -> Result<String, String> {
    serde_json::to_string_pretty(&ProfileFile::of(instance)).map_err(|e| e.to_string())
}

/// Reads and checks a profile file's text.
pub fn parse(text: &str) -> Result<ProfileFile, String> {
    let file: ProfileFile = serde_json::from_str(text)
        .map_err(|e| format!("This is not a Nova profile file: {e}"))?;
    if file.nova_profile != FORMAT {
        return Err(format!(
            "This profile file is format {}, and this launcher reads format {FORMAT}.",
            file.nova_profile
        ));
    }
    if file.name.trim().is_empty() {
        return Err("The profile has no name.".into());
    }
    if let Some(java) = &file.java {
        if java.minecraft.trim().is_empty() {
            return Err("The profile names no Minecraft version.".into());
        }
        if java.memory_mb.is_some_and(|m| !(256..=MAX_MEMORY_MB).contains(&m)) {
            return Err("The profile's memory is out of range.".into());
        }
    }
    Ok(file)
}

/// `name`, or `name (imported)`, `name (imported 2)`, ... when `taken` has it.
pub fn free_name(taken: &[String], name: &str) -> String {
    let has = |n: &str| taken.iter().any(|t| t.eq_ignore_ascii_case(n));
    if !has(name) {
        return name.to_string();
    }
    let first = format!("{name} (imported)");
    if !has(&first) {
        return first;
    }
    (2..)
        .map(|n| format!("{name} (imported {n})"))
        .find(|n| !has(n))
        .expect("an unbounded range always ends")
}

/// Creates the profile `file` describes, next to the existing ones (never over one).
pub fn import(file: &ProfileFile, taken: &[String]) -> Result<Instance, String> {
    let name = free_name(taken, file.name.trim());
    let created = match &file.java {
        Some(java) => instances::create_java(&name, &java.minecraft)?,
        None => instances::create(&name, None)?,
    };
    let (java, group, mods) = (file.java.clone(), file.group.clone(), file.java_mods);
    instances::edit(&created.id, |m| {
        if let Some(java) = java {
            m.java = Some(java);
        }
        m.group = group;
        m.java_mods = mods;
    })
}

/// Where exported profiles are written.
pub fn export_dir() -> PathBuf {
    crate::nova_crash_history::export_dir().with_file_name("profiles")
}

/// Writes `instance`'s profile file; its path.
pub fn export(instance: &Instance) -> Result<PathBuf, String> {
    let dir = export_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let clean: String = instance
        .name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '-' | '_') { c } else { '-' })
        .collect();
    let file = dir.join(format!("{clean}.nova-profile.json"));
    std::fs::write(&file, export_text(instance)?).map_err(|e| format!("{}: {e}", file.display()))?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(java: Option<JavaGame>) -> ProfileFile {
        ProfileFile {
            nova_profile: FORMAT,
            name: "Survival".into(),
            group: Some("Worlds".into()),
            java_mods: true,
            java,
        }
    }

    #[test]
    fn a_profile_file_round_trips() {
        let java = JavaGame {
            minecraft: "1.21.4".into(),
            memory_mb: Some(6144),
            jvm_args: "-XX:+UseG1GC".into(),
            ..JavaGame::default()
        };
        let original = file(Some(java));
        let text = serde_json::to_string_pretty(&original).unwrap();
        assert_eq!(parse(&text).unwrap(), original);
    }

    #[test]
    fn nonsense_is_refused_with_a_reason() {
        assert!(parse("not json").unwrap_err().contains("not a Nova profile"));
        let mut other = file(None);
        other.nova_profile = 9;
        let text = serde_json::to_string(&other).unwrap();
        assert!(parse(&text).unwrap_err().contains("format 9"));
        let mut nameless = file(None);
        nameless.name = "  ".into();
        assert!(parse(&serde_json::to_string(&nameless).unwrap()).is_err());
        let huge = file(Some(JavaGame {
            minecraft: "1.21.4".into(),
            memory_mb: Some(u32::MAX),
            ..JavaGame::default()
        }));
        assert!(parse(&serde_json::to_string(&huge).unwrap()).unwrap_err().contains("memory"));
        let versionless = file(Some(JavaGame::default()));
        assert!(parse(&serde_json::to_string(&versionless).unwrap()).is_err());
    }

    #[test]
    fn an_import_never_takes_an_existing_name() {
        let taken = vec!["Survival".to_string()];
        assert_eq!(free_name(&taken, "Survival"), "Survival (imported)");
        let taken = vec!["Survival".to_string(), "survival (imported)".to_string()];
        assert_eq!(free_name(&taken, "Survival"), "Survival (imported 2)");
        assert_eq!(free_name(&taken, "Other"), "Other");
    }
}
