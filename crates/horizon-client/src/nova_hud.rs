//! Nova's in-game HUD: a few lines of live numbers (FPS, frame time, memory, coordinates and the
//! profile's name) in a corner of the screen, switched on from the Nova launcher.
//!
//! The launcher passes its choices in the `NOVA_OVERLAY` environment variable as `;`-separated
//! tokens (`on;fps;frametime;memory;coords;profile;pos=top-right;size=normal`; F10 hides or shows it while playing) and the profile's
//! name in `NOVA_PROFILE`. Without `on` nothing is drawn. Ping is not shown: the client has no
//! ping reading to draw from here yet.
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

/// Whether F10 has hidden the HUD in this run (it starts shown).
static HIDDEN: AtomicBool = AtomicBool::new(false);

/// F10: hides the HUD, or shows it again.
pub fn toggle() {
    HIDDEN.fetch_xor(true, Ordering::Relaxed);
}

/// Whether F10 has hidden the HUD.
pub fn hidden() -> bool {
    HIDDEN.load(Ordering::Relaxed)
}

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

impl Size {
    /// How much bigger than the debug overlay's text the HUD's is drawn.
    pub fn factor(self) -> f32 {
        match self {
            Size::Small => 0.75,
            Size::Normal => 1.0,
            Size::Large => 1.5,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NovaHud {
    pub fps: bool,
    pub frame_time: bool,
    pub memory: bool,
    pub coords: bool,
    pub profile: Option<String>,
    pub show_profile: bool,
    pub corner: Corner,
    pub size: Size,
}

impl NovaHud {
    /// The HUD the launcher asked for; None while it is off or the variable is absent.
    pub fn parse(text: &str, profile: Option<String>) -> Option<NovaHud> {
        let mut hud = NovaHud {
            fps: false,
            frame_time: false,
            memory: false,
            coords: false,
            profile,
            show_profile: false,
            corner: Corner::TopRight,
            size: Size::Normal,
        };
        let mut on = false;
        for token in text.split(';').map(str::trim).filter(|t| !t.is_empty()) {
            match token {
                "on" => on = true,
                "fps" => hud.fps = true,
                "frametime" => hud.frame_time = true,
                "memory" => hud.memory = true,
                "coords" => hud.coords = true,
                "profile" => hud.show_profile = true,
                "pos=top-left" => hud.corner = Corner::TopLeft,
                "pos=top-right" => hud.corner = Corner::TopRight,
                "size=small" => hud.size = Size::Small,
                "size=normal" => hud.size = Size::Normal,
                "size=large" => hud.size = Size::Large,
                // An unknown token is ignored, so a newer launcher never breaks an older game.
                _ => {}
            }
        }
        on.then_some(hud)
    }

    /// The lines for these readings, in a fixed order; an element without a reading is left out.
    pub fn lines(&self, fps: f32, memory: Option<&str>, position: Option<[f64; 3]>) -> Vec<String> {
        let mut out = Vec::new();
        if self.fps {
            out.push(format!("{} FPS", fps.round() as i32));
        }
        if self.frame_time && fps > 0.0 {
            out.push(format!("{:.1} ms", 1000.0 / fps));
        }
        if self.memory {
            if let Some(memory) = memory {
                out.push(memory.to_string());
            }
        }
        if self.coords {
            if let Some([x, y, z]) = position {
                out.push(format!("XYZ: {x:.1} / {y:.1} / {z:.1}"));
            }
        }
        if self.show_profile {
            if let Some(profile) = &self.profile {
                out.push(format!("Profile: {profile}"));
            }
        }
        out
    }
}

/// The HUD for this run, read once from the environment.
pub fn config() -> Option<&'static NovaHud> {
    static CONFIG: OnceLock<Option<NovaHud>> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            let text = std::env::var("NOVA_OVERLAY").ok()?;
            let profile = std::env::var("NOVA_PROFILE").ok().filter(|p| !p.is_empty());
            NovaHud::parse(&text, profile)
        })
        .as_ref()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_is_off_without_the_on_token() {
        assert_eq!(NovaHud::parse("fps;memory", None), None);
        assert_eq!(NovaHud::parse("", None), None);
    }

    #[test]
    fn tokens_choose_elements_corner_and_size() {
        let hud = NovaHud::parse("on;fps;coords;pos=top-left;size=large;future=1", None).unwrap();
        assert!(hud.fps && hud.coords && !hud.memory);
        assert_eq!(hud.corner, Corner::TopLeft);
        assert_eq!(hud.size, Size::Large);
    }

    #[test]
    fn lines_follow_the_chosen_elements_only() {
        let hud = NovaHud::parse("on;fps;frametime;profile", Some("Survival".into())).unwrap();
        let lines = hud.lines(100.0, Some("Mem: 1%"), Some([1.0, 2.0, 3.0]));
        assert_eq!(lines, vec!["100 FPS", "10.0 ms", "Profile: Survival"]);
    }

    #[test]
    fn a_missing_reading_leaves_its_line_out() {
        let hud = NovaHud::parse("on;memory;coords;frametime", None).unwrap();
        assert!(hud.lines(0.0, None, None).is_empty());
    }
}
