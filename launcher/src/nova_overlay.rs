//! The Nova HUD's choices (what to show, where, how big): kept in `nova-overlay.txt` in the
//! launcher's data folder and handed to the game as `NOVA_OVERLAY` and `NOVA_PROFILE` when it
//! starts (`instances::apply`). The game side is `horizon_client::nova_hud`, which reads the same
//! tokens.
use crate::backend;
use std::collections::BTreeSet;
use std::process::Command;

const FILE: &str = "nova-overlay.txt";

/// The elements the HUD can show: token and label. Ping is not here: the game has no ping to draw.
pub const ELEMENTS: [(&str, &str); 5] = [
    ("fps", "FPS"),
    ("frametime", "Frame time"),
    ("memory", "Memory usage"),
    ("coords", "Coordinates"),
    ("profile", "Current profile"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corner {
    TopLeft,
    TopRight,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Size {
    Small,
    Normal,
    Large,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Overlay {
    pub enabled: bool,
    pub elements: BTreeSet<String>,
    pub corner: Corner,
    pub size: Size,
}

impl Default for Overlay {
    fn default() -> Self {
        Overlay {
            enabled: false,
            elements: ["fps", "frametime"].into_iter().map(String::from).collect(),
            corner: Corner::TopRight,
            size: Size::Normal,
        }
    }
}

impl Overlay {
    pub fn parse(text: &str) -> Overlay {
        let mut out = Overlay {
            enabled: false,
            elements: BTreeSet::new(),
            corner: Corner::TopRight,
            size: Size::Normal,
        };
        for token in text.split(';').map(str::trim) {
            match token {
                "on" => out.enabled = true,
                "pos=top-left" => out.corner = Corner::TopLeft,
                "pos=top-right" => out.corner = Corner::TopRight,
                "size=small" => out.size = Size::Small,
                "size=normal" => out.size = Size::Normal,
                "size=large" => out.size = Size::Large,
                other if ELEMENTS.iter().any(|(t, _)| *t == other) => {
                    out.elements.insert(other.to_string());
                }
                _ => {}
            }
        }
        out
    }

    /// The tokens the game reads, whether or not the HUD is on (`on` is first when it is).
    pub fn to_text(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if self.enabled {
            parts.push("on".into());
        }
        parts.extend(self.elements.iter().cloned());
        parts.push(
            match self.corner {
                Corner::TopLeft => "pos=top-left",
                Corner::TopRight => "pos=top-right",
            }
            .into(),
        );
        parts.push(
            match self.size {
                Size::Small => "size=small",
                Size::Normal => "size=normal",
                Size::Large => "size=large",
            }
            .into(),
        );
        parts.join(";")
    }

    pub fn load() -> Overlay {
        std::fs::read_to_string(backend::data_dir().join(FILE))
            .map(|t| Overlay::parse(&t))
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), String> {
        let dir = backend::data_dir();
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let path = dir.join(FILE);
        std::fs::write(&path, self.to_text()).map_err(|e| format!("{}: {e}", path.display()))
    }
}

/// Gives the game's command the HUD's choices and the profile's name. An off HUD sets nothing.
pub fn apply(cmd: &mut Command, profile: &str) {
    let overlay = Overlay::load();
    if overlay.enabled {
        cmd.env("NOVA_OVERLAY", overlay.to_text())
            .env("NOVA_PROFILE", profile);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choices_survive_a_round_trip() {
        let mut o = Overlay::default();
        o.enabled = true;
        o.corner = Corner::TopLeft;
        o.size = Size::Large;
        o.elements.insert("memory".into());
        assert_eq!(Overlay::parse(&o.to_text()), o);
    }

    #[test]
    fn unknown_tokens_are_dropped() {
        let o = Overlay::parse("on;fps;bogus;ping");
        assert!(o.enabled);
        assert_eq!(o.elements.len(), 1);
    }

    #[test]
    fn it_starts_off() {
        assert!(!Overlay::default().enabled);
        assert!(!Overlay::parse("").enabled);
    }
}
