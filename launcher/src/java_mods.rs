//! The game folder's Java mods (`mods-java/`): Fabric mods a game built with Java mods runs
//! (docs/java-bridge.md). The launcher lists them with their Fabric names, adds jars, sends them
//! to the trash, and reads from the game's last log which of them failed.
use std::io::Read;
use std::path::{Path, PathBuf};

/// The folder's name in the game folder, as the game reads it (`horizon_jvm::DIR`).
pub const DIR: &str = "mods-java";
/// The Minecraft version the bridge runs Fabric mods for: a jar whose `depends.minecraft`
/// excludes it can't start (Fabric refuses the whole set).
pub const MINECRAFT: &str = "26.3";

/// One jar of `mods-java/`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JavaMod {
    /// Its file name in the folder.
    pub file: String,
    /// `fabric.mod.json`'s id, name and version, when the jar has them.
    pub id: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    pub size: u64,
    /// `fabric.mod.json`'s `depends.minecraft`, as written, when it excludes [`MINECRAFT`]:
    /// a jar made for another Minecraft version. None when it fits or can't be told.
    pub wrong_minecraft: Option<String>,
    /// False for a `.jar.disabled` file: the game's folder scan skips it.
    pub enabled: bool,
    /// `depends.minecraft` was read and holds for [`MINECRAFT`]: the only evidence of a fit.
    pub minecraft_verified: bool,
    /// The mod ids `fabric.mod.json` depends on (`depends`), without the platform's own.
    pub depends: Vec<String>,
    /// The ids this jar also stands for (`provides`).
    pub provides: Vec<String>,
    /// Ids that must not be loaded beside it (`breaks`), and ones that may misbehave
    /// (`conflicts`).
    pub breaks: Vec<String>,
    pub conflicts: Vec<String>,
    /// The names in `authors` (a name, or an object with a `name`), as the jar states them.
    pub authors: Vec<String>,
    /// The mod loader the jar was made for, from the metadata files inside it.
    pub loader: Loader,
    /// The jar's own PNG icon (`icon` in `fabric.mod.json`), when it has one under 256 KiB.
    pub icon: Option<Vec<u8>>,
}

/// The loader a jar is made for. Only Fabric runs in the Java Bridge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Loader {
    Fabric,
    Quilt,
    Forge,
    NeoForge,
    /// No loader metadata at all (a library jar, or not a mod).
    Unknown,
}

impl Loader {
    pub fn label(self) -> &'static str {
        match self {
            Loader::Fabric => "Fabric",
            Loader::Quilt => "Quilt",
            Loader::Forge => "Forge",
            Loader::NeoForge => "NeoForge",
            Loader::Unknown => "unknown loader",
        }
    }
}

impl JavaMod {
    /// What to call it: its Fabric name, else its file name.
    pub fn title(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.file)
    }
}

/// What the game's last log says went wrong with its Java mods.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    /// The mod named, when the line names one ("[Java mods] <mod> failed in <where>: …").
    pub name: Option<String>,
    /// The line, without its time and thread.
    pub text: String,
}

/// `home`'s `mods-java/`.
pub fn dir(home: &Path) -> PathBuf {
    home.join(DIR)
}

/// The jars in `home`'s `mods-java/`, by file name.
pub fn list(home: &Path) -> Vec<JavaMod> {
    let mut mods: Vec<JavaMod> = std::fs::read_dir(dir(home))
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file() && (loaded_jar(p) || disabled_jar(p)))
                .map(|p| describe(&p))
                .collect()
        })
        .unwrap_or_default();
    mods.sort_by_key(|m| m.file.to_lowercase());
    mods
}

fn is_jar(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("jar"))
}

/// A jar the game loads from the folder: Fabric's folder scan takes a lower-case `.jar` only.
fn loaded_jar(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "jar")
}

/// A jar the player switched off: `Foo.jar.disabled`, which Fabric's folder scan ignores.
fn disabled_jar(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|n| n.to_string_lossy().ends_with(".jar.disabled"))
}

/// `name` with a lower-case `.jar`, as the folder keeps it (`Foo.JAR` → `Foo.jar`).
fn folder_name(name: &std::ffi::OsStr) -> std::ffi::OsString {
    Path::new(name).with_extension("jar").into_os_string()
}

/// What removing a jar says while the game holds it (Windows keeps a jar Java has open).
pub const CLOSE_GAME_FIRST: &str = "Close the game first: it has {mod} open.";

/// Dependencies the game itself satisfies, which no jar in the folder provides.
const PLATFORM: [&str; 3] = ["minecraft", "java", "fabricloader"];

/// The author names in a `fabric.mod.json`: strings, or objects with a `name`.
fn author_names(meta: Option<&serde_json::Value>) -> Vec<String> {
    meta.and_then(|m| m.get("authors"))
        .and_then(serde_json::Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|a| match a {
                    serde_json::Value::String(name) => Some(name.clone()),
                    serde_json::Value::Object(o) => o.get("name").and_then(serde_json::Value::as_str).map(str::to_string),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Which jars the Mods list shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Filter {
    #[default]
    All,
    Enabled,
    Disabled,
    /// Incompatible or missing a dependency.
    Problems,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sort {
    #[default]
    Name,
    /// Problems first, then warnings, then the rest, disabled last.
    Status,
}

/// The jars of `all` that match the search text and filter, in the chosen order. The text is
/// looked for (ignoring case) in the name, file name, id and authors.
pub fn view(all: &[JavaMod], query: &str, filter: Filter, sort: Sort) -> Vec<JavaMod> {
    let query = query.trim().to_lowercase();
    let rank = |m: &JavaMod| match status(m, all) {
        Status::Incompatible(_) | Status::MissingDependency(_) => 0,
        Status::Warning(_) => 1,
        Status::Compatible => 2,
        Status::Disabled => 3,
    };
    let mut out: Vec<JavaMod> = all
        .iter()
        .filter(|m| match filter {
            Filter::All => true,
            Filter::Enabled => m.enabled,
            Filter::Disabled => !m.enabled,
            Filter::Problems => rank(m) == 0,
        })
        .filter(|m| {
            query.is_empty()
                || m.title().to_lowercase().contains(&query)
                || m.file.to_lowercase().contains(&query)
                || m.id.as_deref().is_some_and(|id| id.to_lowercase().contains(&query))
                || m.authors.iter().any(|a| a.to_lowercase().contains(&query))
        })
        .cloned()
        .collect();
    match sort {
        Sort::Name => out.sort_by_key(|m| m.title().to_lowercase()),
        Sort::Status => out.sort_by_key(|m| (rank(m), m.title().to_lowercase())),
    }
    out
}

/// A jar's file name, size and Fabric facts.
fn describe(path: &Path) -> JavaMod {
    let (meta, loader, icon) = read_jar(path);
    let field = |key: &str| {
        meta.as_ref()
            .and_then(|m| m.get(key))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
    };
    let ids = |pointer: &str| -> Vec<String> {
        match meta.as_ref().and_then(|m| m.pointer(pointer)) {
            Some(serde_json::Value::Object(map)) => map.keys().cloned().collect(),
            Some(serde_json::Value::Array(list)) => list
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_string)
                .collect(),
            _ => Vec::new(),
        }
    };
    JavaMod {
        enabled: !disabled_jar(path),
        minecraft_verified: meta
            .as_ref()
            .and_then(|m| m.pointer("/depends/minecraft"))
            .is_some_and(|wants| runs_on(wants, MINECRAFT) == Some(true)),
        depends: ids("/depends")
            .into_iter()
            .filter(|id| !PLATFORM.contains(&id.as_str()))
            .collect(),
        provides: ids("/provides"),
        breaks: ids("/breaks"),
        conflicts: ids("/conflicts"),
        authors: author_names(meta.as_ref()),
        loader,
        icon,
        file: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        id: field("id"),
        name: field("name"),
        version: field("version"),
        size: std::fs::metadata(path).map(|m| m.len()).unwrap_or(0),
        wrong_minecraft: meta
            .as_ref()
            .and_then(|m| m.pointer("/depends/minecraft"))
            .filter(|wants| runs_on(wants, MINECRAFT) == Some(false))
            .map(|wants| match wants {
                serde_json::Value::Array(any) => any
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .collect::<Vec<_>>()
                    .join(" or "),
                other => other.as_str().unwrap_or_default().to_string(),
            }),
    }
}

/// Whether `file` is a Fabric mod's jar: a `.jar` with a `fabric.mod.json` that parses.
pub fn is_fabric_mod(file: &Path) -> bool {
    is_jar(file) && fabric_meta(file).is_some()
}

/// The mods two or more jars of `mods` are, by Fabric id: (the id, their file names). Fabric
/// refuses to start with two copies of a mod.
pub fn duplicates(mods: &[JavaMod]) -> Vec<(String, Vec<String>)> {
    let mut by_id: std::collections::BTreeMap<&str, Vec<String>> = Default::default();
    for m in mods.iter().filter(|m| m.enabled) {
        if let Some(id) = m.id.as_deref() {
            by_id.entry(id).or_default().push(m.file.clone());
        }
    }
    by_id
        .into_iter()
        .filter(|(_, files)| files.len() > 1)
        .map(|(id, files)| (id.to_string(), files))
        .collect()
}

/// Whether a mod whose `depends.minecraft` is `wants` (a version predicate, or a list of them
/// any of which will do) runs on Minecraft `version`, as Fabric's resolver reads it: `*`,
/// `26.3`, `=26.3`, `>=26.3`, `<27`, `~26.3` (26.3.*), `^26.3` (26.*), `26.3.x`, several
/// joined by spaces (all must hold). None when it can't be told (a form this doesn't read).
pub fn runs_on(wants: &serde_json::Value, version: &str) -> Option<bool> {
    let have = Version::parse(version)?;
    match wants {
        serde_json::Value::String(all) => {
            let mut out = Some(true);
            for term in all.split_whitespace() {
                match term_holds(term, &have) {
                    Some(false) => return Some(false),
                    None => out = None,
                    Some(true) => {}
                }
            }
            out
        }
        serde_json::Value::Array(any) => {
            let mut out = Some(false);
            for one in any {
                match runs_on(one, version) {
                    Some(true) => return Some(true),
                    None => out = None,
                    Some(false) => {}
                }
            }
            out
        }
        _ => None,
    }
}

/// A version as Fabric compares them: numbers, then a pre-release (`-rc.1`) before the release.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Version {
    numbers: Vec<u64>,
    pre: Option<String>,
}

impl Version {
    fn parse(text: &str) -> Option<Self> {
        let text = text.split('+').next()?;
        let (numbers, pre) = match text.split_once('-') {
            Some((n, p)) => (n, Some(p.to_string())),
            None => (text, None),
        };
        let numbers = numbers
            .split('.')
            .map(|n| n.parse().ok())
            .collect::<Option<Vec<u64>>>()?;
        Some(Self { numbers, pre })
    }

    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let len = self.numbers.len().max(other.numbers.len());
        let at = |v: &Self, i: usize| v.numbers.get(i).copied().unwrap_or(0);
        (0..len)
            .map(|i| at(self, i).cmp(&at(other, i)))
            .find(|o| o.is_ne())
            .unwrap_or_else(|| match (&self.pre, &other.pre) {
                (None, None) => std::cmp::Ordering::Equal,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (Some(_), None) => std::cmp::Ordering::Less,
                (Some(a), Some(b)) => a.cmp(b),
            })
    }

    /// The first version past every one starting with this one's first `len` numbers
    /// (`26.3`, 2 → `26.4`).
    fn bumped(&self, len: usize) -> Self {
        let mut numbers: Vec<u64> = (0..len.max(1))
            .map(|i| self.numbers.get(i).copied().unwrap_or(0))
            .collect();
        if let Some(last) = numbers.last_mut() {
            *last += 1;
        }
        Self { numbers, pre: None }
    }
}

/// One space-free predicate of `depends.minecraft` against `have`.
fn term_holds(term: &str, have: &Version) -> Option<bool> {
    use std::cmp::Ordering::*;
    if term == "*" || term.eq_ignore_ascii_case("x") {
        return Some(true);
    }
    let (op, rest) = [">=", "<=", ">", "<", "=", "~", "^"]
        .into_iter()
        .find_map(|op| term.strip_prefix(op).map(|r| (op, r)))
        .unwrap_or(("", term));
    // `26.3.x`, `26.x`, `1.21.*`: the numbers before the wildcard must match.
    let parts: Vec<&str> = rest.split('.').collect();
    if let Some(wild) = parts
        .iter()
        .position(|p| p.eq_ignore_ascii_case("x") || *p == "*")
    {
        if !op.is_empty() && op != "=" {
            return None;
        }
        let prefix = parts[..wild]
            .iter()
            .map(|n| n.parse().ok())
            .collect::<Option<Vec<u64>>>()?;
        return Some(
            prefix
                .iter()
                .enumerate()
                .all(|(i, n)| have.numbers.get(i).copied().unwrap_or(0) == *n),
        );
    }
    let want = Version::parse(rest)?;
    let order = have.cmp(&want);
    Some(match op {
        ">=" => order != Less,
        "<=" => order != Greater,
        ">" => order == Greater,
        "<" => order == Less,
        "" | "=" => order == Equal,
        // ~26.3: 26.3 up to 26.4; ~26: 26 up to 27.
        "~" => order != Less && have.cmp(&want.bumped(want.numbers.len().min(2))) == Less,
        // ^26.3: 26.3 up to 27; ^0.5.1: up to 0.6.
        _ => {
            let first = want
                .numbers
                .iter()
                .position(|n| *n != 0)
                .unwrap_or(want.numbers.len().saturating_sub(1));
            order != Less && have.cmp(&want.bumped(first + 1)) == Less
        }
    })
}

/// Largest icon kept, in bytes.
const ICON_MAX: u64 = 256 * 1024;

/// The loader whose metadata file `has` finds in a jar. Fabric wins when a jar carries several
/// (multi-loader jars), because Fabric is what the Java Bridge runs.
fn loader_from(has: impl Fn(&str) -> bool) -> Loader {
    if has("fabric.mod.json") {
        Loader::Fabric
    } else if has("quilt.mod.json") {
        Loader::Quilt
    } else if has("META-INF/neoforge.mods.toml") {
        Loader::NeoForge
    } else if has("META-INF/mods.toml") {
        Loader::Forge
    } else {
        Loader::Unknown
    }
}

/// The PNG named by `icon` in the metadata (a path, or sizes mapped to paths: the largest),
/// when the jar holds it and it is small enough.
fn read_icon(archive: &mut zip::ZipArchive<std::fs::File>, meta: &serde_json::Value) -> Option<Vec<u8>> {
    let path = match meta.get("icon")? {
        serde_json::Value::String(p) => p.clone(),
        serde_json::Value::Object(sizes) => sizes
            .iter()
            .filter_map(|(size, p)| Some((size.parse::<u32>().ok()?, p.as_str()?)))
            .max_by_key(|(size, _)| *size)?
            .1
            .to_string(),
        _ => return None,
    };
    let entry = archive.by_name(path.trim_start_matches('/')).ok()?;
    if entry.size() > ICON_MAX {
        return None;
    }
    let mut bytes = Vec::new();
    entry.take(ICON_MAX).read_to_end(&mut bytes).ok()?;
    bytes.starts_with(&[0x89, b'P', b'N', b'G']).then_some(bytes)
}

/// Opens the jar once: its `fabric.mod.json` (if it parses), its loader and its icon.
fn read_jar(path: &Path) -> (Option<serde_json::Value>, Loader, Option<Vec<u8>>) {
    let Some(mut archive) = std::fs::File::open(path).ok().and_then(|f| zip::ZipArchive::new(f).ok()) else {
        return (None, Loader::Unknown, None);
    };
    let loader = loader_from(|name| archive.index_for_name(name).is_some());
    let meta = parse_meta(&mut archive);
    let icon = meta.as_ref().and_then(|m| read_icon(&mut archive, m));
    (meta, loader, icon)
}

fn parse_meta(archive: &mut zip::ZipArchive<std::fs::File>) -> Option<serde_json::Value> {
    let entry = archive.by_name("fabric.mod.json").ok()?;
    let mut text = String::new();
    entry.take(1 << 20).read_to_string(&mut text).ok()?;
    // Some mods' files carry raw line breaks inside strings, which Fabric's reader accepts.
    let text: String = text
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect();
    serde_json::from_str(&text).ok()
}

/// The jar's `fabric.mod.json`, if it has one that parses.
fn fabric_meta(path: &Path) -> Option<serde_json::Value> {
    let mut archive = zip::ZipArchive::new(std::fs::File::open(path).ok()?).ok()?;
    let entry = archive.by_name("fabric.mod.json").ok()?;
    let mut text = String::new();
    entry.take(1 << 20).read_to_string(&mut text).ok()?;
    // Some mods' files carry raw line breaks inside strings, which Fabric's reader accepts.
    let text: String = text
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect();
    serde_json::from_str(&text).ok()
}

/// Copies the Fabric mod `jar` into `home`'s `mods-java/`. A jar without `fabric.mod.json` (not
/// a Fabric mod: a Forge or NeoForge one) is refused, and so is a file name already there.
pub fn add(home: &Path, jar: &Path) -> Result<JavaMod, String> {
    if !jar.is_file() || !is_jar(jar) {
        return Err(format!("{} is not a .jar file", jar.display()));
    }
    if fabric_meta(jar).is_none() {
        return Err(format!(
            "{} is not a Fabric mod (it has no fabric.mod.json): Horizon runs Fabric mods",
            jar.display()
        ));
    }
    let name = jar.file_name().ok_or("The jar has no file name")?;
    let folder = dir(home);
    std::fs::create_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    let name = folder_name(name);
    let target = folder.join(&name);
    if target.exists() {
        return Err(format!("{} is already in {DIR}", name.to_string_lossy()));
    }
    std::fs::copy(jar, &target).map_err(|e| format!("{}: {e}", target.display()))?;
    Ok(describe(&target))
}

/// What the Mods page says of one jar, from the Fabric metadata it can read. Never `Compatible`
/// without evidence: `depends.minecraft` must have been read and hold.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Compatible,
    /// Loads as far as can be told, but something could not be verified or may misbehave.
    Warning(Vec<String>),
    /// Fabric will refuse it: another Minecraft version, a duplicate or a mod it breaks with.
    Incompatible(Vec<String>),
    /// The ids it depends on that no enabled jar in the folder provides.
    MissingDependency(Vec<String>),
    Disabled,
}

impl Status {
    pub fn label(&self) -> &'static str {
        match self {
            Status::Compatible => "Compatible",
            Status::Warning(_) => "Warning",
            Status::Incompatible(_) => "Incompatible",
            Status::MissingDependency(_) => "Missing dependency",
            Status::Disabled => "Disabled",
        }
    }

    /// The reasons, one line each (empty for Compatible and Disabled).
    pub fn reasons(&self) -> &[String] {
        match self {
            Status::Warning(r) | Status::Incompatible(r) | Status::MissingDependency(r) => r,
            _ => &[],
        }
    }
}

/// `m`'s status among `all` the jars of the folder.
pub fn status(m: &JavaMod, all: &[JavaMod]) -> Status {
    if !m.enabled {
        return Status::Disabled;
    }
    let others = || all.iter().filter(|o| o.enabled && o.file != m.file);
    let mut bad = Vec::new();
    if !matches!(m.loader, Loader::Fabric | Loader::Unknown) {
        bad.push(format!(
            "This is a {} mod. The Java Bridge runs Fabric mods only.",
            m.loader.label()
        ));
    }
    if let Some(wants) = &m.wrong_minecraft {
        bad.push(format!("Made for Minecraft {wants}, not {MINECRAFT}."));
    }
    if let Some(id) = m.id.as_deref() {
        if let Some(twin) = others().find(|o| o.id.as_deref() == Some(id)) {
            bad.push(format!("{} is the same mod ({id}): Fabric won't start with both.", twin.file));
        }
    }
    for o in others() {
        let stands_for = |id: &String| o.id.as_ref() == Some(id) || o.provides.contains(id);
        if let Some(id) = m.breaks.iter().find(|id| stands_for(*id)) {
            bad.push(format!("It breaks with {} ({id}), which is also installed.", o.title()));
        }
    }
    if !bad.is_empty() {
        return Status::Incompatible(bad);
    }
    let missing: Vec<String> = m
        .depends
        .iter()
        .filter(|dep| {
            !others().any(|o| o.id.as_ref() == Some(*dep) || o.provides.contains(*dep))
                && m.id.as_ref() != Some(*dep)
        })
        .map(|dep| format!("Needs {dep}, which is not in {DIR}."))
        .collect();
    if !missing.is_empty() {
        return Status::MissingDependency(missing);
    }
    let mut warn = Vec::new();
    for o in others() {
        let stands_for = |id: &String| o.id.as_ref() == Some(id) || o.provides.contains(id);
        if let Some(id) = m.conflicts.iter().find(|id| stands_for(*id)) {
            warn.push(format!("It may conflict with {} ({id}).", o.title()));
        }
    }
    if m.loader == Loader::Unknown {
        warn.push("The jar has no mod metadata (no fabric.mod.json): it is a library or not a mod.".to_string());
    }
    if !m.minecraft_verified {
        warn.push(format!(
            "Nothing in the jar says it runs on Minecraft {MINECRAFT}, so it is not known to fit."
        ));
    }
    if warn.is_empty() {
        Status::Compatible
    } else {
        Status::Warning(warn)
    }
}

/// Switches a jar on or off by renaming it (`Foo.jar` <-> `Foo.jar.disabled`): the game loads
/// only `.jar` files. `file` is the name in the folder.
pub fn set_enabled(home: &Path, file: &str, enabled: bool) -> Result<JavaMod, String> {
    let from = dir(home).join(file);
    if !from.is_file() {
        return Err(format!("No Java mod {file} in {DIR}"));
    }
    let to_name = match (enabled, file.strip_suffix(".disabled")) {
        (true, Some(jar)) => jar.to_string(),
        (false, None) if file.ends_with(".jar") => format!("{file}.disabled"),
        (true, None) | (false, Some(_)) => return Ok(describe(&from)),
        (false, None) => return Err(format!("{file} is not a .jar file")),
    };
    let to = dir(home).join(&to_name);
    if to.exists() {
        return Err(format!("{to_name} is already in {DIR}"));
    }
    if held_open(&from) {
        return Err(crate::i18n::trf(CLOSE_GAME_FIRST, &[("mod", file)]));
    }
    std::fs::rename(&from, &to).map_err(|e| format!("{}: {e}", from.display()))?;
    Ok(describe(&to))
}

/// Puts the downloaded jar `new` in `home`'s `mods-java/` in place of `old_file`: the new file takes
/// its name there (over the old one when the names match), then the old jar is deleted. Refused
/// while the game has the old jar open.
pub fn replace_with(home: &Path, old_file: &str, new: &Path) -> Result<JavaMod, String> {
    let old = dir(home).join(old_file);
    if !old.is_file() {
        return Err(format!("No Java mod {old_file} in {DIR}"));
    }
    if held_open(&old) {
        return Err(crate::i18n::trf(CLOSE_GAME_FIRST, &[("mod", old_file)]));
    }
    let name = new.file_name().ok_or("The download has no file name")?;
    let target = dir(home).join(name);
    if target != old && target.exists() {
        return Err(format!("{} is already in {DIR}", target.display()));
    }
    std::fs::rename(new, &target).map_err(|e| format!("{}: {e}", target.display()))?;
    if target != old {
        std::fs::remove_file(&old).map_err(|e| format!("{}: {e}", old.display()))?;
    }
    Ok(describe(&target))
}

/// Sends `file` (a file name in `mods-java/`, or the mod's Fabric id or name) to the trash.
pub fn remove(home: &Path, which: &str) -> Result<JavaMod, String> {
    let found = list(home)
        .into_iter()
        .find(|m| {
            m.file == which
                || m.id.as_deref() == Some(which)
                || m.name
                    .as_deref()
                    .is_some_and(|n| n.eq_ignore_ascii_case(which))
        })
        .ok_or_else(|| format!("No Java mod {which} in {DIR}"))?;
    let path = dir(home).join(&found.file);
    let held = || crate::i18n::trf(CLOSE_GAME_FIRST, &[("mod", found.title())]);
    if held_open(&path) {
        return Err(held());
    }
    crate::desktop_seams::to_trash(&path).map_err(|e| if held_open(&path) { held() } else { e })?;
    Ok(found)
}

/// Whether another program (the game's Java) has `path` open: on Windows, which then refuses to
/// move it, the file can't be opened for itself alone. Elsewhere a held file moves freely.
fn held_open(path: &Path) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        let alone = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(path);
        matches!(alone, Err(e) if crate::java_variant::in_use(&e))
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        false
    }
}

/// What went wrong with the Java mods in the game's last run (`logs/latest.log` in `home`):
/// the mods that failed while it ran, as the game told the players, and a start that failed.
pub fn failures(home: &Path) -> Vec<Failure> {
    let log = std::fs::read_to_string(home.join("logs").join("latest.log")).unwrap_or_default();
    failures_in(&log)
}

fn failures_in(log: &str) -> Vec<Failure> {
    let mut out: Vec<Failure> = Vec::new();
    // Inside Fabric's resolution report, whose lines after its first ("Incompatible mods
    // found!") carry no time or thread: "\t - Mod 'Name' (id) 1.0 requires …".
    let mut report = false;
    for line in log.lines() {
        let record = line.starts_with('[') && line.contains("]: ");
        if report && !record {
            if let Some(failure) = report_line(line) {
                if !out.contains(&failure) {
                    out.push(failure);
                }
            }
            continue;
        }
        report = false;
        // "[12:00:00] [Server thread/INFO]: ..." → the message.
        let message = line.split_once("]: ").map_or(line, |(_, m)| m).trim();
        if message.contains("Incompatible mods found")
            || message.contains("Mod resolution failed")
            || message.contains("incompatible mod set")
        {
            report = true;
        }
        let failure = if let Some(rest) = message.split_once("[Java mods] ").map(|(_, r)| r) {
            let Some((name, _)) = rest.split_once(" failed in ") else {
                continue;
            };
            Failure {
                name: Some(name.trim().to_string()),
                text: rest.trim().to_string(),
            }
        } else if message.starts_with("Java mods: not started")
            || message.starts_with("Java mods: Fabric can't start")
            || message.contains("Incompatible mods found")
            || message.contains("Mod resolution failed")
        {
            Failure {
                name: None,
                text: message.to_string(),
            }
        } else {
            continue;
        };
        if !out.contains(&failure) {
            out.push(failure);
        }
    }
    out
}

/// A line of Fabric's resolution report worth showing: "- Mod 'Name' (id) …" names the mod
/// (its row in the tab is marked), "- Install fabric-api, any version." and the like don't.
fn report_line(line: &str) -> Option<Failure> {
    let text = line.trim().strip_prefix("- ")?.trim();
    if text.is_empty() {
        return None;
    }
    let name = text
        .strip_prefix("Mod '")
        .and_then(|rest| rest.split_once("' (").map(|(name, _)| name.to_string()));
    Some(Failure {
        name,
        text: text.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jar(path: &Path, meta: Option<&str>) {
        let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        if let Some(meta) = meta {
            zip.start_file("fabric.mod.json", options).unwrap();
            std::io::Write::write_all(&mut zip, meta.as_bytes()).unwrap();
        }
        zip.start_file("a/B.class", options).unwrap();
        zip.finish().unwrap();
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("hz-java-mods-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn adds_lists_and_removes_fabric_jars() {
        let root = scratch("add");
        let home = root.join("home");
        let fd = root.join("FarmersDelight.jar");
        jar(
            &fd,
            Some(r#"{"id":"farmersdelight","name":"Farmer's Delight","version":"3.6"}"#),
        );
        let forge = root.join("forge-only.jar");
        jar(&forge, None);
        let added = add(&home, &fd).unwrap();
        assert_eq!(added.title(), "Farmer's Delight");
        assert!(add(&home, &fd).unwrap_err().contains("already"));
        assert!(add(&home, &forge).unwrap_err().contains("not a Fabric mod"));
        let listed = list(&home);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id.as_deref(), Some("farmersdelight"));
        // SAFETY of the test: the trash is a scratch folder (desktop_seams).
        unsafe { std::env::set_var(crate::desktop_seams::TRASH_ENV, root.join("trash")) };
        remove(&home, "farmersdelight").unwrap();
        assert!(list(&home).is_empty());
        assert!(root.join("trash/FarmersDelight.jar").is_file());
        assert!(remove(&home, "farmersdelight").is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Fabric's folder scan skips `Foo.JAR`: added, it is renamed `Foo.jar`, and one put there
    /// by hand is not listed as a mod the game runs.
    #[test]
    fn an_upper_case_jar_is_added_as_the_game_loads_it() {
        let root = scratch("upper");
        let home = root.join("home");
        let upper = root.join("Upper.JAR");
        jar(&upper, Some(r#"{"id":"upper"}"#));
        assert_eq!(add(&home, &upper).unwrap().file, "Upper.jar");
        assert!(dir(&home).join("Upper.jar").is_file());
        std::fs::copy(&upper, dir(&home).join("Other.JAR")).unwrap();
        let files: Vec<String> = list(&home).into_iter().map(|m| m.file).collect();
        assert_eq!(files, ["Upper.jar"]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn reads_failures_from_the_log() {
        let log = "\
[10:00:00] [Server thread/INFO]: System chat: [Java mods] Farmer's Delight failed in the HUD: NullPointerException: x (the game goes on; the log has more)
[10:00:01] [Server thread/INFO]: System chat: [Java mods] Farmer's Delight failed in the HUD: NullPointerException: x (the game goes on; the log has more)
[10:00:02] [Render thread/WARN]: Java mods: not started: no Java 25
[10:00:03] [Server thread/INFO]: System chat: hello
";
        let found = failures_in(log);
        assert_eq!(found.len(), 2);
        // The owner gate's refusal stops the game the same way (`not_started`): the tab says why.
        let refused = "[10:00:00] [main/WARN]: Java mods: off: Fabric 26.3 mods run only when Horizon's launcher starts the game (no owner record from the launcher)
[10:00:00] [main/ERROR]: Java mods: not started: Fabric 26.3 mods run only when Horizon's launcher starts the game (no owner record from the launcher)
";
        let gate = failures_in(refused);
        assert_eq!(gate.len(), 1, "{gate:?}");
        assert!(gate[0].text.contains("no owner record"));
        assert_eq!(found[0].name.as_deref(), Some("Farmer's Delight"));
        assert!(found[1].text.starts_with("Java mods: not started"));
    }

    /// Fabric's report spreads over lines with no time or thread: each "- Mod '…'" names its
    /// mod, a suggestion is kept unnamed, and the next log record ends the report.
    #[test]
    fn reads_the_mods_fabrics_resolution_report_names() {
        let log = "\
[10:00:00] [Java mods/WARN]: Java mods: Fabric can't start: Incompatible mods found!
net.fabricmc.loader.impl.FormattedException: Some of your mods are incompatible with the game or each other!
A potential solution has been determined, this may resolve your problem:
\t - Install fabric-language-kotlin, version 1.13.0 or later.
More details:
\t - Mod 'Farmer's Delight' (farmersdelight) 3.6 requires version 1.21.1 of 'Minecraft' (minecraft), but only the wrong version is present: 26.3!
\t - Mod 'Croptopia' (croptopia) 4.0 requires any version of fabric-language-kotlin, which is missing!
[10:00:01] [Render thread/INFO]: - Mod 'Not a report' (no) 1.0
";
        let found = failures_in(log);
        let names: Vec<_> = found.iter().map(|f| f.name.as_deref()).collect();
        assert_eq!(
            names,
            [None, None, Some("Farmer's Delight"), Some("Croptopia")],
            "{found:#?}"
        );
        assert!(found[1].text.starts_with("Install fabric-language-kotlin"));
        assert!(found[2].text.contains("requires version 1.21.1"));
    }

    fn on_26_3(wants: serde_json::Value) -> Option<bool> {
        runs_on(&wants, "26.3")
    }

    #[test]
    fn reads_which_minecraft_versions_a_mod_runs_on() {
        use serde_json::json;
        for yes in [
            "*",
            "26.3",
            "=26.3",
            "26.3.0",
            ">=26.3",
            ">=26.3-alpha.1",
            ">26.2",
            "<=26.3",
            "<27",
            "~26.3",
            "~26",
            "^26.1",
            "26.3.x",
            "26.x",
            ">=26.2 <26.4",
            ">=1.21",
        ] {
            assert_eq!(on_26_3(json!(yes)), Some(true), "{yes}");
        }
        for no in [
            "1.21.x",
            "1.21.1",
            "~1.21",
            "^1.21",
            ">26.3",
            "<26.3",
            ">=26.4",
            "~26.4",
            "26.4.x",
            ">=1.21 <1.22",
            "26.3-rc.1",
        ] {
            assert_eq!(on_26_3(json!(no)), Some(false), "{no}");
        }
        // A list: any of them will do.
        assert_eq!(on_26_3(json!(["1.21.x", "~26.3"])), Some(true));
        assert_eq!(on_26_3(json!(["1.21.x", "1.20.1"])), Some(false));
        // What this can't read is never called wrong.
        assert_eq!(on_26_3(json!("weird-thing")), None);
        assert_eq!(on_26_3(json!([">=1.21.x"])), None);
        assert_eq!(on_26_3(json!(3)), None);
    }

    #[test]
    fn marks_jars_for_another_minecraft_and_two_copies_of_a_mod() {
        let root = scratch("versions");
        let home = root.join("home");
        let old = root.join("old.jar");
        jar(
            &old,
            Some(r#"{"id":"old","depends":{"minecraft":["1.21.x","1.20.1"]}}"#),
        );
        let new = root.join("new.jar");
        jar(
            &new,
            Some(r#"{"id":"new","depends":{"minecraft":"~26.3"}}"#),
        );
        let copy = root.join("new-copy.jar");
        jar(&copy, Some(r#"{"id":"new","depends":{"minecraft":"*"}}"#));
        assert_eq!(
            add(&home, &old).unwrap().wrong_minecraft.as_deref(),
            Some("1.21.x or 1.20.1")
        );
        assert_eq!(add(&home, &new).unwrap().wrong_minecraft, None);
        add(&home, &copy).unwrap();
        assert!(is_fabric_mod(&new));
        assert!(!is_fabric_mod(&root.join("missing.jar")));
        assert_eq!(
            duplicates(&list(&home)),
            [(
                "new".to_string(),
                vec!["new-copy.jar".to_string(), "new.jar".to_string()]
            )]
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn disabling_renames_the_jar_and_enabling_restores_it() {
        let home = scratch("toggle");
        std::fs::create_dir_all(dir(&home)).unwrap();
        jar(&dir(&home).join("a.jar"), Some(r#"{"id":"a","version":"1"}"#));
        let off = set_enabled(&home, "a.jar", false).unwrap();
        assert!(!off.enabled);
        assert!(dir(&home).join("a.jar.disabled").is_file());
        assert!(!dir(&home).join("a.jar").exists());
        assert_eq!(list(&home).len(), 1);
        let on = set_enabled(&home, "a.jar.disabled", true).unwrap();
        assert!(on.enabled);
        assert!(dir(&home).join("a.jar").is_file());
    }

    #[test]
    fn a_missing_dependency_is_named() {
        let home = scratch("deps");
        std::fs::create_dir_all(dir(&home)).unwrap();
        jar(
            &dir(&home).join("a.jar"),
            Some(r#"{"id":"a","depends":{"minecraft":"*","fabric-api":"*","fabricloader":"*"}}"#),
        );
        let all = list(&home);
        match status(&all[0], &all) {
            Status::MissingDependency(why) => assert!(why[0].contains("fabric-api")),
            other => panic!("{other:?}"),
        }
        jar(&dir(&home).join("api.jar"), Some(r#"{"id":"fabric-api"}"#));
        let all = list(&home);
        let a = all.iter().find(|m| m.file == "a.jar").unwrap();
        assert_eq!(status(a, &all), Status::Compatible);
    }

    #[test]
    fn compatible_needs_evidence() {
        let home = scratch("evidence");
        std::fs::create_dir_all(dir(&home)).unwrap();
        jar(&dir(&home).join("b.jar"), Some(r#"{"id":"b"}"#));
        let all = list(&home);
        assert!(matches!(status(&all[0], &all), Status::Warning(_)));
    }

    #[test]
    fn authors_are_read_from_strings_and_objects() {
        let home = scratch("authors");
        std::fs::create_dir_all(dir(&home)).unwrap();
        jar(
            &dir(&home).join("a.jar"),
            Some(r#"{"id":"a","authors":["Ann",{"name":"Bob"},7]}"#),
        );
        assert_eq!(list(&home)[0].authors, ["Ann", "Bob"]);
    }

    #[test]
    fn the_view_searches_filters_and_sorts() {
        let home = scratch("view");
        std::fs::create_dir_all(dir(&home)).unwrap();
        jar(&dir(&home).join("b.jar"), Some(r#"{"id":"b","name":"Beta","authors":["Zed"]}"#));
        jar(&dir(&home).join("a.jar"), Some(r#"{"id":"a","name":"Alpha","depends":{"ghost":"*"}}"#));
        jar(&dir(&home).join("c.jar.disabled"), Some(r#"{"id":"c","name":"Gamma"}"#));
        let all = list(&home);
        let names = |v: Vec<JavaMod>| v.iter().map(|m| m.title().to_string()).collect::<Vec<_>>();
        assert_eq!(names(view(&all, "", Filter::All, Sort::Name)), ["Alpha", "Beta", "Gamma"]);
        assert_eq!(names(view(&all, "zed", Filter::All, Sort::Name)), ["Beta"]);
        assert_eq!(names(view(&all, "", Filter::Disabled, Sort::Name)), ["Gamma"]);
        assert_eq!(names(view(&all, "", Filter::Problems, Sort::Name)), ["Alpha"]);
        assert_eq!(names(view(&all, "", Filter::Enabled, Sort::Status))[0], "Alpha");
    }

    fn zip_with(path: &Path, entries: &[(&str, Vec<u8>)]) {
        let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        for (name, bytes) in entries {
            zip.start_file(*name, options).unwrap();
            std::io::Write::write_all(&mut zip, bytes).unwrap();
        }
        zip.finish().unwrap();
    }

    fn bytes(text: &str) -> Vec<u8> {
        text.as_bytes().to_vec()
    }

    #[test]
    fn the_loader_comes_from_the_metadata_files() {
        let home = scratch("loader");
        std::fs::create_dir_all(dir(&home)).unwrap();
        zip_with(&dir(&home).join("forge.jar"), &[("META-INF/mods.toml", bytes("x"))]);
        zip_with(&dir(&home).join("neo.jar"), &[("META-INF/neoforge.mods.toml", bytes("x"))]);
        zip_with(&dir(&home).join("quilt.jar"), &[("quilt.mod.json", bytes("{}"))]);
        zip_with(&dir(&home).join("plain.jar"), &[("a/B.class", bytes("x"))]);
        zip_with(
            &dir(&home).join("multi.jar"),
            &[
                ("fabric.mod.json", bytes(r#"{"id":"multi"}"#)),
                ("META-INF/mods.toml", bytes("x")),
            ],
        );
        let all = list(&home);
        let by = |f: &str| all.iter().find(|m| m.file == f).unwrap();
        assert_eq!(by("forge.jar").loader, Loader::Forge);
        assert_eq!(by("neo.jar").loader, Loader::NeoForge);
        assert_eq!(by("quilt.jar").loader, Loader::Quilt);
        assert_eq!(by("plain.jar").loader, Loader::Unknown);
        assert_eq!(by("multi.jar").loader, Loader::Fabric);
        match status(by("forge.jar"), &all) {
            Status::Incompatible(why) => assert!(why[0].contains("Forge mod")),
            other => panic!("{other:?}"),
        }
        assert!(matches!(status(by("plain.jar"), &all), Status::Warning(_)));
    }

    #[test]
    fn the_icon_is_read_when_it_is_a_small_png() {
        let home = scratch("icon");
        std::fs::create_dir_all(dir(&home)).unwrap();
        let png: Vec<u8> = vec![0x89, b'P', b'N', b'G', 1, 2, 3];
        zip_with(
            &dir(&home).join("a.jar"),
            &[
                ("fabric.mod.json", bytes(r#"{"id":"a","icon":{"16":"s.png","128":"big.png"}}"#)),
                ("big.png", png.clone()),
                ("s.png", bytes("nope")),
            ],
        );
        zip_with(
            &dir(&home).join("b.jar"),
            &[
                ("fabric.mod.json", bytes(r#"{"id":"b","icon":"not-png.png"}"#)),
                ("not-png.png", bytes("GIF89a")),
            ],
        );
        let all = list(&home);
        assert_eq!(all[0].icon.as_deref(), Some(png.as_slice()));
        assert_eq!(all[1].icon, None);
    }

    #[test]
    fn a_disabled_twin_is_not_a_duplicate() {
        let home = scratch("twin");
        std::fs::create_dir_all(dir(&home)).unwrap();
        jar(&dir(&home).join("c.jar"), Some(r#"{"id":"c"}"#));
        jar(&dir(&home).join("c2.jar.disabled"), Some(r#"{"id":"c"}"#));
        assert!(duplicates(&list(&home)).is_empty());
    }
}
