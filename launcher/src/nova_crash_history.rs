//! Nova's crash history: every analysis the launcher makes is kept in `crash-history.jsonl`
//! (one JSON object per line) in the Nova data folder, so the Crash Analyzer page can list,
//! search, filter, export and delete them. Raw logs are never copied or uploaded: an entry holds
//! the analyzer's findings, a few matching log lines and the path of the original file.
use chrono::{DateTime, Local, SecondsFormat, Utc};
use horizon_launcher::crash_analyzer::CrashAnalysis;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// One saved analysis. Every field but the time has a default, so a line written by an older
/// build (fewer fields) still loads.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    #[serde(default)]
    pub id: String,
    /// When the analysis was made: RFC 3339, UTC, fixed width, so the text sorts by time.
    pub timestamp: String,
    /// The analysed file's name.
    #[serde(default)]
    pub source: String,
    /// Its full path, to open the original later.
    #[serde(default)]
    pub source_path: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub confidence: u8,
    /// True only when the log held an explicit error message for this cause.
    #[serde(default)]
    pub confirmed: bool,
    #[serde(default)]
    pub excerpt: Vec<String>,
    #[serde(default)]
    pub suggestions: Vec<String>,
    /// Mod ids the log names; named, not proven guilty.
    #[serde(default)]
    pub suspect_mods: Vec<String>,
    /// The version of the profile that was active when the analysis was made.
    #[serde(default)]
    pub minecraft: String,
    #[serde(default)]
    pub profile: String,
}

impl Entry {
    pub fn new(source: &Path, analysis: &CrashAnalysis, minecraft: &str, profile: &str) -> Entry {
        let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Nanos, true);
        Entry {
            id: timestamp.clone(),
            timestamp,
            source: source
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            source_path: source.to_string_lossy().into_owned(),
            category: analysis.kind.label().to_string(),
            summary: analysis.summary.clone(),
            confidence: analysis.confidence,
            confirmed: analysis.confirmed,
            excerpt: analysis.excerpt.clone(),
            suggestions: analysis.suggestions.clone(),
            suspect_mods: analysis.probable_mods.clone(),
            minecraft: minecraft.to_string(),
            profile: profile.to_string(),
        }
    }

    /// The time as the player's clock shows it (`2026-11-21 14:05`), or the stored text when it
    /// cannot be read.
    pub fn local_time(&self) -> String {
        DateTime::parse_from_rfc3339(&self.timestamp)
            .map(|t| t.with_timezone(&Local).format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|_| self.timestamp.clone())
    }

    /// Whether the cause is unknown (nothing matched).
    pub fn is_unknown(&self) -> bool {
        self.category == horizon_launcher::crash_analyzer::CrashKind::Unknown.label()
    }

    /// The certainty in words, so an uncertain finding is never read as a fact.
    pub fn certainty_text(&self) -> &'static str {
        if self.is_unknown() {
            "No cause could be identified"
        } else if self.confirmed {
            "Confirmed by the log"
        } else {
            "Possible cause, not confirmed"
        }
    }
}

/// The history file.
pub fn path() -> PathBuf {
    let base = dirs::data_local_dir().unwrap_or_else(std::env::temp_dir);
    base.join("NovaClient").join("crash-history.jsonl")
}

/// Where exported reports go, beside the history.
pub fn export_dir() -> PathBuf {
    path().with_file_name("exports")
}

/// The entries in `file` and how many lines could not be read. A missing file is an empty
/// history, not an error.
pub fn load_from(file: &Path) -> (Vec<Entry>, usize) {
    let Ok(text) = std::fs::read_to_string(file) else {
        return (Vec::new(), 0);
    };
    let (mut entries, mut skipped) = (Vec::new(), 0);
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        match serde_json::from_str::<Entry>(line) {
            Ok(mut entry) => {
                if entry.id.is_empty() {
                    entry.id = format!("{}|{}", entry.timestamp, entry.source);
                }
                entries.push(entry);
            }
            Err(_) => skipped += 1,
        }
    }
    (entries, skipped)
}

pub fn load() -> (Vec<Entry>, usize) {
    load_from(&path())
}

fn same_finding(a: &Entry, b: &Entry) -> bool {
    let close = match (
        DateTime::parse_from_rfc3339(&a.timestamp),
        DateTime::parse_from_rfc3339(&b.timestamp),
    ) {
        (Ok(x), Ok(y)) => (x - y).num_seconds().abs() < 60,
        _ => false,
    };
    close && a.source_path == b.source_path && a.category == b.category && a.summary == b.summary
}

/// Adds `entry` to `file` unless the newest entry is the same finding made under a minute ago
/// (the launcher analyses a crash on its own, and the player may press Analyze at once).
/// Whether it was added.
pub fn append_to(file: &Path, entry: &Entry) -> Result<bool, String> {
    let (existing, _) = load_from(file);
    if existing.last().is_some_and(|last| same_finding(last, entry)) {
        return Ok(false);
    }
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    use std::io::Write;
    let mut out = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(file)
        .map_err(|e| format!("{}: {e}", file.display()))?;
    let line = serde_json::to_string(entry).map_err(|e| e.to_string())?;
    writeln!(out, "{line}").map_err(|e| format!("{}: {e}", file.display()))?;
    Ok(true)
}

/// Saves an analysis of `source` in the history.
pub fn record(source: &Path, analysis: &CrashAnalysis, minecraft: &str, profile: &str) -> Result<bool, String> {
    append_to(&path(), &Entry::new(source, analysis, minecraft, profile))
}

/// Replaces the file's contents with `entries` (written beside it, then renamed, so a failure
/// never leaves a half-written history).
pub fn rewrite(file: &Path, entries: &[Entry]) -> Result<(), String> {
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    let mut text = String::new();
    for entry in entries {
        text.push_str(&serde_json::to_string(entry).map_err(|e| e.to_string())?);
        text.push('\n');
    }
    let temp = file.with_extension("jsonl.tmp");
    std::fs::write(&temp, text).map_err(|e| format!("{}: {e}", temp.display()))?;
    std::fs::rename(&temp, file).map_err(|e| format!("{}: {e}", file.display()))
}

/// Removes the entry with `id`. Whether there was one.
pub fn delete_from(file: &Path, id: &str) -> Result<bool, String> {
    let (mut entries, _) = load_from(file);
    let before = entries.len();
    entries.retain(|e| e.id != id);
    if entries.len() == before {
        return Ok(false);
    }
    rewrite(file, &entries)?;
    Ok(true)
}

pub fn clear_in(file: &Path) -> Result<(), String> {
    rewrite(file, &[])
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Certainty {
    #[default]
    Any,
    Confirmed,
    Possible,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Order {
    #[default]
    Newest,
    Oldest,
}

/// The entries that match the search text, certainty and category, in the chosen order. The text
/// is looked for (ignoring case) in the file name, category, summary, profile, version and the
/// named mods.
pub fn select<'a>(
    entries: &'a [Entry],
    query: &str,
    certainty: Certainty,
    category: Option<&str>,
    order: Order,
) -> Vec<&'a Entry> {
    let query = query.trim().to_lowercase();
    let mut out: Vec<&Entry> = entries
        .iter()
        .filter(|e| match certainty {
            Certainty::Any => true,
            Certainty::Confirmed => e.confirmed && !e.is_unknown(),
            Certainty::Possible => !e.confirmed && !e.is_unknown(),
        })
        .filter(|e| category.is_none_or(|c| e.category == c))
        .filter(|e| {
            query.is_empty()
                || [&e.source, &e.category, &e.summary, &e.profile, &e.minecraft]
                    .iter()
                    .any(|f| f.to_lowercase().contains(&query))
                || e.suspect_mods.iter().any(|m| m.to_lowercase().contains(&query))
        })
        .collect();
    out.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    if order == Order::Newest {
        out.reverse();
    }
    out
}

/// The categories present, sorted, for the filter.
pub fn categories(entries: &[Entry]) -> Vec<String> {
    let mut list: Vec<String> = entries.iter().map(|e| e.category.clone()).collect();
    list.sort();
    list.dedup();
    list
}

/// The report as plain text, for the clipboard and `.txt` export.
pub fn report_txt(e: &Entry) -> String {
    let mut out = format!(
        "Nova Client crash report\n========================\nDate: {}\nLog file: {}\nMinecraft version: {}\nProfile: {}\n\nCategory: {}\nCertainty: {} ({}% confidence)\nSummary: {}\n",
        e.local_time(),
        e.source,
        if e.minecraft.is_empty() { "unknown" } else { &e.minecraft },
        if e.profile.is_empty() { "unknown" } else { &e.profile },
        e.category,
        e.certainty_text(),
        e.confidence,
        e.summary,
    );
    if !e.suspect_mods.is_empty() {
        out.push_str(&format!(
            "\nMods named in the log (named, not proven to be the cause): {}\n",
            e.suspect_mods.join(", ")
        ));
    }
    if !e.excerpt.is_empty() {
        out.push_str("\nLog excerpt:\n");
        for line in &e.excerpt {
            out.push_str(&format!("  {line}\n"));
        }
    }
    if !e.suggestions.is_empty() {
        out.push_str("\nWhat to try, in order:\n");
        for (n, step) in e.suggestions.iter().enumerate() {
            out.push_str(&format!("  {}. {step}\n", n + 1));
        }
    }
    out.push_str("\nThis is an automatic diagnosis from the log's text. It is a lead, not a guarantee.\n");
    out
}

/// The report as JSON, for `.json` export.
pub fn report_json(e: &Entry) -> Result<String, String> {
    serde_json::to_string_pretty(e).map_err(|e| e.to_string())
}

/// Writes `text` in the export folder as `<log name>-<id>.<extension>`; the path.
pub fn export(e: &Entry, extension: &str, text: &str) -> Result<PathBuf, String> {
    let clean = |s: &str| -> String {
        s.chars()
            .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '-' | '_') { c } else { '-' })
            .collect()
    };
    let dir = export_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let file = dir.join(format!("{}-{}.{extension}", clean(&e.source), clean(&e.id)));
    std::fs::write(&file, text).map_err(|e| format!("{}: {e}", file.display()))?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use horizon_launcher::crash_analyzer::analyze;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nova-history-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("crash-history.jsonl")
    }

    fn entry(log: &str, source: &str, at: &str) -> Entry {
        let mut e = Entry::new(Path::new(source), &analyze(log), "26.3", "Survival");
        e.timestamp = at.to_string();
        e.id = at.to_string();
        e
    }

    #[test]
    fn an_entry_survives_a_save_and_load() {
        let file = temp("roundtrip");
        let e = entry("java.lang.OutOfMemoryError: heap", "crash-1.txt", "2026-11-21T10:00:00.000000000Z");
        assert!(append_to(&file, &e).unwrap());
        let (loaded, skipped) = load_from(&file);
        assert_eq!((loaded.len(), skipped), (1, 0));
        assert_eq!(loaded[0], e);
        assert_eq!(loaded[0].minecraft, "26.3");
        assert!(loaded[0].confirmed);
    }

    #[test]
    fn old_and_broken_lines_do_not_break_the_history() {
        let file = temp("legacy");
        std::fs::write(
            &file,
            "{\"timestamp\":\"2026-11-20T09:00:00Z\",\"source\":\"latest.log\",\"category\":\"Out of memory\",\"summary\":\"s\",\"confidence\":96,\"confirmed\":true}\nnot json\n\n",
        )
        .unwrap();
        let (entries, skipped) = load_from(&file);
        assert_eq!((entries.len(), skipped), (1, 1));
        assert!(!entries[0].id.is_empty());
        assert!(entries[0].excerpt.is_empty());
    }

    #[test]
    fn the_same_finding_twice_in_a_minute_is_saved_once() {
        let file = temp("dedupe");
        let a = entry("java.lang.OutOfMemoryError", "c.txt", "2026-11-21T10:00:00.000000000Z");
        let b = entry("java.lang.OutOfMemoryError", "c.txt", "2026-11-21T10:00:30.000000000Z");
        let later = entry("java.lang.OutOfMemoryError", "c.txt", "2026-11-21T10:05:00.000000000Z");
        assert!(append_to(&file, &a).unwrap());
        assert!(!append_to(&file, &b).unwrap());
        assert!(append_to(&file, &later).unwrap());
        assert_eq!(load_from(&file).0.len(), 2);
    }

    #[test]
    fn deleting_one_entry_keeps_the_others_and_clearing_empties() {
        let file = temp("delete");
        for (i, log) in ["java.lang.OutOfMemoryError", "VK_ERROR_DEVICE_LOST", "MixinApplyError"].iter().enumerate() {
            append_to(&file, &entry(log, "x.log", &format!("2026-11-21T10:0{i}:00.000000000Z"))).unwrap();
        }
        assert!(delete_from(&file, "2026-11-21T10:01:00.000000000Z").unwrap());
        assert!(!delete_from(&file, "nope").unwrap());
        let (left, _) = load_from(&file);
        assert_eq!(left.len(), 2);
        assert!(left.iter().all(|e| e.category != "Vulkan initialization or device failure"));
        clear_in(&file).unwrap();
        assert!(load_from(&file).0.is_empty());
    }

    #[test]
    fn search_filter_and_order_work_together() {
        let all = vec![
            entry("java.lang.OutOfMemoryError", "a.log", "2026-11-21T10:00:00.000000000Z"),
            entry("VK_ERROR_DEVICE_LOST", "b.log", "2026-11-21T11:00:00.000000000Z"),
            entry("nothing known here", "c.log", "2026-11-21T12:00:00.000000000Z"),
        ];
        let newest: Vec<_> = select(&all, "", Certainty::Any, None, Order::Newest).iter().map(|e| e.source.clone()).collect();
        assert_eq!(newest, ["c.log", "b.log", "a.log"]);
        let oldest = select(&all, "", Certainty::Any, None, Order::Oldest);
        assert_eq!(oldest[0].source, "a.log");
        assert_eq!(select(&all, "vulkan", Certainty::Any, None, Order::Newest).len(), 1);
        assert_eq!(select(&all, "SURVIVAL", Certainty::Any, None, Order::Newest).len(), 3);
        // Unknown findings are neither confirmed nor possible.
        assert_eq!(select(&all, "", Certainty::Confirmed, None, Order::Newest).len(), 2);
        assert_eq!(select(&all, "", Certainty::Possible, None, Order::Newest).len(), 0);
        assert_eq!(select(&all, "", Certainty::Any, Some("Out of memory"), Order::Newest).len(), 1);
    }

    #[test]
    fn the_report_never_presents_a_guess_as_certain() {
        let unknown = entry("nothing known here", "c.log", "2026-11-21T12:00:00.000000000Z");
        let text = report_txt(&unknown);
        assert!(text.contains("No cause could be identified"));
        assert!(text.contains("not a guarantee"));
        let sure = entry("java.lang.OutOfMemoryError", "a.log", "2026-11-21T10:00:00.000000000Z");
        assert!(report_txt(&sure).contains("Confirmed by the log"));
        let json = report_json(&sure).unwrap();
        assert!(serde_json::from_str::<Entry>(&json).unwrap() == sure);
    }
}
