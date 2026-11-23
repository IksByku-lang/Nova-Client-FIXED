//! The Home "Recent activity" list: when profiles were last played and when crashes were analysed,
//! newest first. It reads only what the launcher already keeps.
use crate::nova_crash_history::Entry;
use horizon_launcher::instances::Instance;
use std::time::SystemTime;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    /// Seconds since 1970.
    pub at: u64,
    pub text: String,
}

/// "just now", "5 min ago", "3 h ago", "2 d ago" for something `at` seconds since 1970 when now
/// is `now`. A time in the future reads as "just now".
pub fn ago(now: u64, at: u64) -> String {
    let secs = now.saturating_sub(at);
    match secs {
        0..=59 => "just now".into(),
        60..=3599 => format!("{} min ago", secs / 60),
        3600..=86399 => format!("{} h ago", secs / 3600),
        _ => format!("{} d ago", secs / 86400),
    }
}

/// The newest `limit` events from the profiles' last play times and the crash history.
pub fn recent(instances: &[Instance], crashes: &[Entry], limit: usize) -> Vec<Item> {
    let mut items: Vec<Item> = Vec::new();
    for i in instances {
        if let Some(at) = i
            .last_played
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
        {
            items.push(Item { at, text: format!("Played {}", i.name) });
        }
    }
    for e in crashes {
        if let Ok(t) = chrono::DateTime::parse_from_rfc3339(&e.timestamp) {
            if let Ok(at) = u64::try_from(t.timestamp()) {
                items.push(Item {
                    at,
                    text: format!("Analysed {}: {}", e.source, e.category),
                });
            }
        }
    }
    items.sort_by(|a, b| b.at.cmp(&a.at));
    items.truncate(limit);
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_read_in_the_largest_fitting_unit() {
        assert_eq!(ago(1000, 1000), "just now");
        assert_eq!(ago(1000, 2000), "just now");
        assert_eq!(ago(1000, 940), "1 min ago");
        assert_eq!(ago(10_000, 10_000 - 7200), "2 h ago");
        assert_eq!(ago(1_000_000, 1_000_000 - 3 * 86400), "3 d ago");
    }

    #[test]
    fn crashes_are_listed_newest_first_and_limited() {
        let mk = |ts: &str, src: &str| Entry {
            id: ts.into(),
            timestamp: ts.into(),
            source: src.into(),
            source_path: String::new(),
            category: "Out of memory".into(),
            summary: String::new(),
            confidence: 0,
            confirmed: false,
            excerpt: vec![],
            suggestions: vec![],
            suspect_mods: vec![],
            minecraft: String::new(),
            profile: String::new(),
        };
        let crashes = vec![
            mk("2026-11-20T10:00:00Z", "old.log"),
            mk("2026-11-21T10:00:00Z", "new.log"),
            mk("garbage", "bad.log"),
        ];
        let items = recent(&[], &crashes, 1);
        assert_eq!(items.len(), 1);
        assert!(items[0].text.contains("new.log"));
    }
}
