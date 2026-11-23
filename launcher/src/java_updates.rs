//! Updates for the Java mods folder, from Modrinth. A jar's SHA-1 identifies its version there;
//! Modrinth answers with the newest listed version for Fabric and the Java Bridge's Minecraft
//! version, and only a version whose file differs from the jar counts as an update. Mods Modrinth
//! does not know are skipped (they cannot be updated from here), never reported as up to date.
use crate::browser::{modrinth, modrinth::Modrinth, FileRef, Source};
use crate::java_mods::{JavaMod, MINECRAFT};
use sha1::{Digest, Sha1};
use std::path::Path;

/// A newer version of one jar in the folder.
#[derive(Clone, Debug, PartialEq)]
pub struct Update {
    /// The jar in the folder that it replaces.
    pub file: String,
    /// The mod's name, for messages.
    pub title: String,
    pub new: FileRef,
}

pub fn sha1_of(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(Sha1::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect())
}

/// The file of `files` to update to, given the SHA-1 of the jar now installed: the primary file
/// (else the first) when its checksum differs. None when the same file is already installed, or
/// when the version lists no SHA-1 to compare.
pub fn newer<'a>(current_sha1: &str, files: &'a [FileRef]) -> Option<&'a FileRef> {
    let file = files.iter().find(|f| f.primary).or_else(|| files.first())?;
    let theirs = file.sha1.as_deref()?;
    (!theirs.eq_ignore_ascii_case(current_sha1)).then_some(file)
}

/// The updates available for the enabled jars of `mods`, whose files are in `dir`. Stops at the
/// first network failure (the same one would repeat for the rest).
pub fn check(dir: &Path, mods: &[JavaMod]) -> Result<Vec<Update>, String> {
    let mut out = Vec::new();
    for m in mods.iter().filter(|m| m.enabled) {
        let sha1 = sha1_of(&dir.join(&m.file))?;
        let Some(version) = modrinth::update_for_hash(&sha1, "fabric", MINECRAFT)? else {
            continue;
        };
        if let Some(file) = newer(&sha1, &version.files) {
            out.push(Update {
                file: m.file.clone(),
                title: m.title().to_string(),
                new: file.clone(),
            });
        }
    }
    Ok(out)
}

/// Downloads the update (checked against Modrinth's checksum) beside the folder, then swaps it
/// for the old jar. The name of the new jar.
pub fn apply(home: &Path, update: &Update) -> Result<String, String> {
    let temp = crate::java_mods::dir(home).with_file_name("mods-java-update");
    std::fs::create_dir_all(&temp).map_err(|e| format!("{}: {e}", temp.display()))?;
    let result = Modrinth
        .fetch(&update.new, &temp, &|_| {})
        .and_then(|path| crate::java_mods::replace_with(home, &update.file, &path));
    let _ = std::fs::remove_dir_all(&temp);
    result.map(|m| m.file)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str, sha1: Option<&str>, primary: bool) -> FileRef {
        FileRef {
            url: format!("https://example.invalid/{name}"),
            name: name.into(),
            size: 1,
            sha512: None,
            sha256: None,
            sha1: sha1.map(str::to_string),
            primary,
        }
    }

    #[test]
    fn the_same_file_is_not_an_update() {
        let files = vec![file("a.jar", Some("ABC123"), true)];
        assert_eq!(newer("abc123", &files), None);
    }

    #[test]
    fn a_different_primary_file_is_the_update() {
        let files = vec![file("a-src.jar", Some("1"), false), file("a.jar", Some("2"), true)];
        assert_eq!(newer("3", &files).unwrap().name, "a.jar");
    }

    #[test]
    fn without_a_checksum_nothing_is_claimed() {
        assert_eq!(newer("3", &[file("a.jar", None, true)]), None);
        assert_eq!(newer("3", &[]), None);
    }

    #[test]
    fn sha1_matches_the_known_digest() {
        let path = std::env::temp_dir().join(format!("nova-sha1-{}.bin", std::process::id()));
        std::fs::write(&path, b"abc").unwrap();
        assert_eq!(sha1_of(&path).unwrap(), "a9993e364706816aba3e25717850c26c9cd0d89d");
        let _ = std::fs::remove_file(&path);
    }
}
