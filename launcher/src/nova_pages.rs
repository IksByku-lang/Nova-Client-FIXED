//! Nova Client's own pages: Profiles, Performance and About.
//!
//! Profiles are the launcher's instances (`horizon_launcher::instances`): each has its own game
//! folder, mods, version and (for Java instances) memory and JVM arguments, saved in
//! `instances.json`. Nothing here keeps a second copy of that data.
//!
//! Performance only shows what the launcher can really apply: the graphics API
//! (`backend::set_graphics_api`, read by the game at the next start) and, for Minecraft: Java
//! Edition profiles, the heap size (`-Xmx`). Options the launcher does not write (FPS limit,
//! VSync, distances, fullscreen) are listed as not available, not faked.
use super::*;
use crate::servers_page::Field;
use crate::glass_ui::{self as ui, white};
use horizon_theme::glass::{ink, radius};

pub(super) const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The sidebar's navigation, as the concept has it: page, label, icon.
pub(super) const SIDEBAR_NAV: [(Page, &str, &str); 7] = [
    (Page::Play, "Home", "game-controller-01"),
    (Page::Mods, "Mods", "puzzle"),
    (Page::Profiles, "Profiles", "user-multiple-02"),
    (Page::Performance, "Performance", "sliders-horizontal"),
    (Page::Crashes, "Crash Analyzer", "ai-chat-02"),
    (Page::Settings, "Settings", "computer"),
    (Page::About, "About", "information-circle"),
];

/// The hero artwork cropped from the concept image (the castle and the player on the right).
static HERO: std::sync::LazyLock<std::sync::Arc<gpui::Image>> = std::sync::LazyLock::new(|| {
    std::sync::Arc::new(gpui::Image::from_bytes(
        gpui::ImageFormat::Png,
        include_bytes!("../assets/nova-hero.png").to_vec(),
    ))
});

/// The artwork, drawn faded on the right of Home's head, behind its text.
pub(super) fn nova_hero_art() -> Div {
    div()
        .absolute()
        .top_0()
        .right_0()
        .w(px(380.))
        .h(px(440.))
        .rounded(px(radius::PANEL))
        .overflow_hidden()
        .opacity(0.55)
        .child(
            gpui::img(HERO.clone())
                .size_full()
                .object_fit(gpui::ObjectFit::Cover),
        )
}

const HORIZON_REPO: &str = "https://github.com/horizon-rust/horizon";
const BRIDGE_REPO: &str = "https://github.com/iaxivers/horizon-java-bridge";

// ----- Logic (no window needed, covered by the tests at the bottom) -----------------------------

/// A memory preset for a profile's Java heap.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Preset {
    Low,
    Balanced,
    High,
    /// Any other value: set by hand.
    Custom,
}

impl Preset {
    pub(super) const ALL: [(Preset, &'static str); 4] = [
        (Preset::Low, "Low"),
        (Preset::Balanced, "Balanced"),
        (Preset::High, "High"),
        (Preset::Custom, "Custom"),
    ];

    /// The heap in MiB this preset sets on a computer with `total_mb` of memory, never more than
    /// three quarters of it (and never under 1 GiB). Custom sets nothing.
    pub(super) fn memory_mb(self, total_mb: u32) -> Option<u32> {
        let cap = ((total_mb / 4 * 3) / 256 * 256).max(1024);
        let wanted = match self {
            Preset::Low => 2048,
            Preset::Balanced => 4096,
            Preset::High => 8192,
            Preset::Custom => return None,
        };
        Some(wanted.min(cap))
    }

    /// The preset whose heap is `memory_mb`; Custom when none is.
    pub(super) fn detect(memory_mb: u32, total_mb: u32) -> Preset {
        [Preset::Low, Preset::Balanced, Preset::High]
            .into_iter()
            .find(|p| p.memory_mb(total_mb) == Some(memory_mb))
            .unwrap_or(Preset::Custom)
    }
}

/// A name for a copy of `base` that no profile in `taken` has: `Base copy`, `Base copy 2`, ...
pub(super) fn copy_name(taken: &[String], base: &str) -> String {
    let has = |name: &str| taken.iter().any(|t| t.eq_ignore_ascii_case(name));
    let first = format!("{base} copy");
    if !has(&first) {
        return first;
    }
    (2..)
        .map(|n| format!("{base} copy {n}"))
        .find(|name| !has(name))
        .expect("an unbounded range always ends")
}

/// A memory size as the pages show it: `4 GiB`, or `1536 MiB` when it is not whole GiB.
pub(super) fn memory_label(mb: u32) -> String {
    if mb % 1024 == 0 {
        format!("{} GiB", mb / 1024)
    } else {
        format!("{mb} MiB")
    }
}

// ----- State -----------------------------------------------------------------------------------

/// What the three pages remember between frames.
#[derive(Default)]
pub(super) struct NovaUi {
    /// The profile whose Delete waits for its confirmation.
    pub(super) confirm_delete: Option<String>,
    /// What the last action did, or why it could not.
    pub(super) note: Option<Result<String, String>>,
    /// The in-game HUD's choices, read from `nova-overlay.txt` when first needed.
    pub(super) overlay: Option<horizon_launcher::nova_overlay::Overlay>,
    /// What the dashboard's analyzer found in the newest log or crash report, or why not.
    pub(super) analysis: Option<Result<(std::path::PathBuf, horizon_launcher::crash_analyzer::CrashAnalysis), String>>,
    /// The Home Mods card's Modrinth search: the results, why it failed, whether one is running
    /// (and which, so a late answer is dropped) and what is being installed.
    pub(super) results: Vec<horizon_launcher::browser::Project>,
    pub(super) search_error: Option<String>,
    pub(super) searching: bool,
    pub(super) generation: u64,
    pub(super) installing: Option<String>,
    pub(super) install_note: Option<Result<String, String>>,
    /// The profile whose serious mod problems Play has already warned about once.
    pub(super) warned_for: Option<String>,
    /// Reset settings waits for its confirmation.
    pub(super) confirm_reset: bool,
    /// What the last reset did.
    pub(super) reset_note: Option<Result<String, String>>,
    /// The active profile's Java mods, read once per profile (each jar is opened to read it);
    /// Refresh reads them again.
    pub(super) mods: Option<(String, Vec<horizon_launcher::java_mods::JavaMod>)>,
}

/// The newest crash report or log file, with `.gz` archives left out.
pub(super) fn newest_log() -> Option<std::path::PathBuf> {
    let mut best: Option<(std::time::SystemTime, std::path::PathBuf)> = None;
    for dir in [backend::crash_reports_dir(), backend::logs_dir()] {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            let archived = path.extension().is_some_and(|e| e == "gz");
            let Ok(when) = meta.modified() else {
                continue;
            };
            if meta.is_file() && !archived && best.as_ref().is_none_or(|(t, _)| when > *t) {
                best = Some((when, path));
            }
        }
    }
    best.map(|(_, path)| path)
}

/// The last `max` bytes of a file as text (a log's end is where the failure is).
pub(super) fn read_tail(path: &std::path::Path, max: usize) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let start = bytes.len().saturating_sub(max);
    Ok(String::from_utf8_lossy(&bytes[start..]).into_owned())
}

/// The analysis as plain text for the clipboard or a bug report.
fn report_text(file: &std::path::Path, a: &horizon_launcher::crash_analyzer::CrashAnalysis) -> String {
    let mut out = format!(
        "Nova crash report\nFile: {}\nCategory: {}\n{} ({}% confidence)\nSummary: {}\n",
        file.display(),
        a.kind.label(),
        if a.confirmed { "Confirmed by the log" } else { "Possible cause, not confirmed" },
        a.confidence,
        a.summary,
    );
    if !a.excerpt.is_empty() {
        out.push_str("\nLog excerpt:\n");
        for line in &a.excerpt {
            out.push_str(&format!("  {line}\n"));
        }
    }
    out.push_str("\nSuggested fixes:\n");
    for fix in &a.suggestions {
        out.push_str(&format!("  - {fix}\n"));
    }
    out
}

/// A profile's mod list as plain text, for sharing or a bug report.
fn mod_list_text(
    profile: &str,
    mods: &[horizon_launcher::java_mods::JavaMod],
) -> String {
    use horizon_launcher::java_mods::{status, MINECRAFT};
    let mut out = format!(
        "Nova Client mod list\nProfile: {profile}\nMinecraft (Java Bridge): {MINECRAFT}\nMods: {}\n\n",
        mods.len()
    );
    for m in mods {
        out.push_str(&format!(
            "- {} {} [{}] ({}): {}\n",
            m.title(),
            m.version.as_deref().unwrap_or("unknown version"),
            if m.enabled { "enabled" } else { "disabled" },
            m.file,
            status(m, mods).label(),
        ));
    }
    out
}

/// The same list as JSON.
fn mod_list_json(
    profile: &str,
    mods: &[horizon_launcher::java_mods::JavaMod],
) -> Result<String, String> {
    use horizon_launcher::java_mods::{status, MINECRAFT};
    let list: Vec<_> = mods
        .iter()
        .map(|m| {
            serde_json::json!({
                "file": m.file,
                "id": m.id,
                "name": m.title(),
                "version": m.version,
                "enabled": m.enabled,
                "depends": m.depends,
                "status": status(m, mods).label(),
            })
        })
        .collect();
    serde_json::to_string_pretty(&serde_json::json!({
        "profile": profile,
        "minecraft": MINECRAFT,
        "mods": list,
    }))
    .map_err(|e| e.to_string())
}

/// A section's card, as the Settings page draws its own.
fn card() -> Div {
    crate::glass::plate(radius::PANEL).flex().flex_col().p(px(8.))
}

fn say(s: &str) -> Option<AnyElement> {
    Some(div().child(tr(s).to_string()).into_any_element())
}

fn say_owned(s: String) -> Option<AnyElement> {
    Some(div().child(s).into_any_element())
}

impl Launcher {
    /// What the last action on these pages did, as a line under the title.
    fn nova_note(&self) -> Option<Div> {
        let (text, colour) = match self.nova.note.as_ref()? {
            Ok(said) => (said.clone(), ui::live()),
            Err(error) => (error.clone(), ui::danger()),
        };
        Some(
            div()
                .mb(px(16.))
                .px(px(16.))
                .child(ui::text(13., FontWeight::MEDIUM, 1.).text_color(colour).child(text)),
        )
    }

    // ----- Sidebar and Home --------------------------------------------------------------------

    /// The logo and name over the navigation list, at the top of the sidebar.
    pub(super) fn nova_brand_and_nav(&mut self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let look = ui::look();
        let brand = div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(6.))
            .pt(px(20.))
            .pb(px(12.))
            .child(ui::icon("nova", 44., ui::color(look.accent_soft)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .child(ui::text(22., FontWeight::BOLD, ink::FULL).child("NOVA"))
                    .child(
                        ui::text(17., FontWeight::BOLD, 1.)
                            .text_color(ui::color(look.accent_soft))
                            .child("CLIENT"),
                    ),
            );
        let mut nav = div().flex().flex_col().gap(px(2.)).px(px(10.));
        for (page, label, icon) in SIDEBAR_NAV {
            let current = self.page == page;
            nav = nav.child(
                ui::row(format!("side-nav-{label}"), current, 8., &t)
                    .child(ui::icon(
                        icon,
                        18.,
                        if current { white(1.) } else { white(ink::STRONG) },
                    ))
                    .child(
                        ui::text(15., FontWeight::MEDIUM, ink::FULL)
                            .child(tr(label).to_string()),
                    )
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.go(page);
                        cx.notify();
                    })),
            );
        }
        div().flex().flex_col().flex_none().child(brand).child(nav)
    }

    /// The foot of the sidebar's navigation: the signed-in player (head, name, Microsoft account;
    /// a click opens Settings to manage accounts) or, while nobody is signed in, a Sign in with
    /// Microsoft button that starts the launcher's real sign-in. Under it, the Nova slogan.
    pub(super) fn nova_account_block(&mut self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let look = ui::look();
        let who = match self.accounts.selected().map(|a| (a.uuid.clone(), a.name.clone())) {
            Some((uuid, name)) => {
                let head = self.profile_heads.draw(Some(uuid.as_str()), 34., self.scale, &t);
                div()
                    .id("side-account")
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .p(px(8.))
                    .rounded(px(radius::ROW))
                    .bg(white(0.06))
                    .hover(|s| s.bg(white(0.10)))
                    .cursor_pointer()
                    .child(div().size(px(34.)).flex_none().rounded(px(8.)).overflow_hidden().child(head))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .min_w_0()
                            .child(ui::text(14., FontWeight::SEMIBOLD, ink::FULL).truncate().child(name))
                            .child(ui::card_note(tr("Microsoft account").to_string())),
                    )
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.go(Page::Settings);
                        cx.notify();
                    }))
            }
            None => div()
                .id("side-sign-in")
                .flex()
                .items_center()
                .justify_center()
                .gap(px(10.))
                .h(px(44.))
                .rounded(px(radius::ROW))
                .bg(white(0.08))
                .hover(|s| s.bg(white(0.14)))
                .cursor_pointer()
                .child(crate::microsoft::logo(16.))
                .child(ui::text(14., FontWeight::MEDIUM, ink::FULL).child(tr("Sign in with Microsoft").to_string()))
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.start_sign_in();
                    cx.notify();
                })),
        };
        let slogan = div()
            .flex()
            .items_center()
            .gap(px(10.))
            .p(px(10.))
            .rounded(px(radius::ROW))
            .bg(white(0.04))
            .child(ui::icon("nova", 24., ui::color(look.accent_soft)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(ui::text(14., FontWeight::SEMIBOLD, ink::FULL).child("Nova Client"))
                    .child(
                        ui::text(12., FontWeight::NORMAL, 1.)
                            .text_color(ui::color(look.accent_soft))
                            .child(tr("Play. Optimize. Enjoy.").to_string()),
                    ),
            );
        div()
            .flex()
            .flex_col()
            .flex_none()
            .gap(px(8.))
            .px(px(10.))
            .pb(px(10.))
            .child(who)
            .child(slogan)
    }

    /// Home's status tiles and Quick Actions (the concept's row under Play): the selected
    /// profile, the Java Bridge and the game version, read from the profile itself.
    pub(super) fn nova_home_strip(&mut self, tag: &str, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let Some(profile) = self.selected_instance().cloned() else {
            return div();
        };
        let version = match &profile.java {
            Some(java) => java.minecraft.clone(),
            None if tag.is_empty() => tr("not installed").to_string(),
            None => tag.to_string(),
        };
        let mods_count = self.installed_mods(&profile);
        let (bridge, tone) = if profile.java_mods {
            (tr("Enabled"), ui::Tone::Live)
        } else {
            (tr("Off for this profile"), ui::Tone::Off)
        };
        let tile = |label: &str, value: AnyElement| {
            crate::glass::plate(radius::LARGE)
                .flex()
                .flex_col()
                .flex_1()
                .min_w(px(150.))
                .px(px(16.))
                .py(px(12.))
                .child(ui::card_note(label.to_string()))
                .child(value)
        };
        let value = |text: String| {
            ui::text(16., FontWeight::SEMIBOLD, ink::FULL)
                .truncate()
                .child(text)
                .into_any_element()
        };
        let tiles = div()
            .mt(px(24.))
            .flex()
            .flex_wrap()
            .gap(px(10.))
            .child(tile(tr("Selected profile"), value(profile.name.clone())))
            .child(tile(
                tr("Java Bridge"),
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(ui::dot(tone))
                    .child(ui::text(16., FontWeight::SEMIBOLD, ink::FULL).child(bridge.to_string()))
                    .into_any_element(),
            ))
            .child(tile(tr("Game version"), value(version)))
            .child(tile(
                tr("Mods"),
                value(trf("{count} installed", &[("count", &mods_count.to_string())])),
            ));
        let _ = (t, cx);
        div().flex().flex_col().child(tiles)
    }

    // ----- Settings: reset ---------------------------------------------------------------------

    /// "Reset settings" with its confirmation, for the Settings page. It sets the launcher's own
    /// options back to their defaults and keeps the selected game version and profile; profiles,
    /// mods, worlds and accounts are not touched. Some options show their new value only after
    /// the launcher restarts.
    pub(super) fn nova_reset_row(&mut self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let note = match (&self.nova.reset_note, self.nova.confirm_reset) {
            (_, true) => tr("Reset every launcher setting to its default? Profiles, mods, worlds and accounts are kept.").to_string(),
            (Some(Ok(said)), _) => said.clone(),
            (Some(Err(error)), _) => error.clone(),
            (None, _) => tr("Sets the launcher's options back to their defaults.").to_string(),
        };
        let controls = if self.nova.confirm_reset {
            div()
                .flex()
                .gap(px(8.))
                .child(
                    button("nova-reset-cancel", "Cancel", Kind::Secondary, &t).on_click(
                        cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.nova.confirm_reset = false;
                            cx.notify();
                        }),
                    ),
                )
                .child(
                    button("nova-reset-confirm", "Reset", Kind::Primary, &t).on_click(
                        cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.reset_settings();
                            cx.notify();
                        }),
                    ),
                )
        } else {
            div().child(
                button("nova-reset", "Reset settings", Kind::Secondary, &t).on_click(
                    cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.nova.confirm_reset = true;
                        this.nova.reset_note = None;
                        cx.notify();
                    }),
                ),
            )
        };
        ui::setting_row(
            tr("Reset settings"),
            say_owned(note),
            Some(controls.into_any_element()),
        )
    }

    fn reset_settings(&mut self) {
        self.nova.confirm_reset = false;
        // The game and server folders are where the profiles live: resetting them would make every
        // profile seem to vanish, so they are kept with the selected version and profile.
        let (selected, instance) = (self.settings.selected.clone(), self.settings.instance.clone());
        let (minecraft_dir, server_dir) = (self.settings.minecraft_dir.clone(), self.settings.server_dir.clone());
        self.settings = backend::Settings {
            selected,
            instance,
            minecraft_dir,
            server_dir,
            ..Default::default()
        };
        self.nova.reset_note = Some(match self.settings.save() {
            Ok(()) => Ok(tr("Settings were reset. Restart the launcher to apply all of them.").to_string()),
            Err(e) => Err(trf("Could not save the reset settings: {error}", &[("error", &e)])),
        });
    }

    // ----- Profiles ----------------------------------------------------------------------------

    pub(super) fn profiles_page(&mut self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let selected = self.selected_instance().map(|i| i.id.clone());
        let list = self.instances.list.clone();
        let names: Vec<String> = list.iter().map(|i| i.name.clone()).collect();
        let mut rows = card();
        for (n, instance) in list.iter().enumerate() {
            let active = selected.as_deref() == Some(instance.id.as_str());
            let version = match &instance.java {
                Some(java) => match &java.loader {
                    Some(loader) => format!("Minecraft {} · {}", java.minecraft, loader.kind.id()),
                    None => format!("Minecraft {} (Java Edition)", java.minecraft),
                },
                None => match self.version_of(instance) {
                    Some(tag) => format!("Horizon {tag}"),
                    None => tr("Horizon (version not installed)").to_string(),
                },
            };
            let memory = instance
                .java
                .as_ref()
                .map(|j| j.memory_mb.unwrap_or(instances::DEFAULT_MEMORY_MB))
                .map(|mb| format!(" · {}", memory_label(mb)))
                .unwrap_or_default();
            let mods_folder = instance.home.join("mods");
            let confirming = self.nova.confirm_delete.as_deref() == Some(instance.id.as_str());
            let is_default = instance.kind == instances::Kind::Default;

            let mut controls = div().flex().flex_wrap().items_center().gap(px(8.));
            if confirming {
                let (id, name) = (instance.id.clone(), instance.name.clone());
                controls = controls
                    .child(
                        button(format!("nova-cancel-{n}"), "Cancel", Kind::Secondary, &t).on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.nova.confirm_delete = None;
                                cx.notify();
                            }),
                        ),
                    )
                    .child(
                        button(format!("nova-confirm-{n}"), "Delete", Kind::Primary, &t).on_click(
                            cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.delete_profile(&id, &name);
                                cx.notify();
                            }),
                        ),
                    );
            } else {
                if !active {
                    let picked = instance.clone();
                    controls = controls.child(
                        button(format!("nova-use-{n}"), "Use", Kind::Primary, &t).on_click(
                            cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.select_instance(&picked);
                                this.nova.note = Some(Ok(trf(
                                    "{name} is now the profile Play starts.",
                                    &[("name", &picked.name)],
                                )));
                                cx.notify();
                            }),
                        ),
                    );
                }
                let edited = instance.clone();
                controls = controls.child(
                    button(format!("nova-edit-{n}"), "Edit", Kind::Secondary, &t).on_click(
                        cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.open_instance_dialog(Some(edited.clone()));
                            cx.notify();
                        }),
                    ),
                );
                let (from, taken) = (instance.clone(), names.clone());
                controls = controls.child(
                    button(format!("nova-copy-{n}"), "Duplicate", Kind::Secondary, &t).on_click(
                        cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.duplicate_profile(&from, &taken);
                            cx.notify();
                        }),
                    ),
                );
                let exported = instance.clone();
                controls = controls.child(
                    button(format!("nova-export-{n}"), "Export", Kind::Ghost, &t).on_click(
                        cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.nova.note = Some(
                                crate::nova_profile_io::export(&exported).map(|path| {
                                    trf("Profile saved in {path}. It holds settings only, not mods or worlds.", &[("path", &path.display().to_string())])
                                }),
                            );
                            cx.notify();
                        }),
                    ),
                );
                controls = controls.child(
                    button(format!("nova-mods-{n}"), "Mods folder", Kind::Ghost, &t).on_click({
                        let dir = mods_folder.clone();
                        move |_, _, cx| {
                            if let Err(e) = std::fs::create_dir_all(&dir) {
                                eprintln!("Nova: {}: {e}", dir.display());
                                return;
                            }
                            crate::glass_toast::open(cx, &dir);
                        }
                    }),
                );
                if !is_default {
                    let id = instance.id.clone();
                    controls = controls.child(
                        button(format!("nova-del-{n}"), "Delete", Kind::Ghost, &t).on_click(
                            cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.nova.confirm_delete = Some(id.clone());
                                cx.notify();
                            }),
                        ),
                    );
                }
            }
            let note = if confirming {
                format!(
                    "{} {}",
                    trf("Delete {name}?", &[("name", &instance.name)]),
                    tr("Its folder, worlds and mods go to the Trash.")
                )
            } else if is_default {
                format!("{version}{memory} · {}", tr("the game's own folder, cannot be deleted"))
            } else {
                format!("{version}{memory}")
            };
            let title = if active {
                format!("{} ({})", instance.name, tr("active"))
            } else {
                instance.name.clone()
            };
            rows = rows.child(ui::setting_row(
                title,
                say_owned(note),
                Some(controls.into_any_element()),
            ));
        }
        let status = self.status_card(cx);
        let create = button("nova-new-profile", "New profile", Kind::Primary, &t).on_click(
            cx.listener(|this, _: &ClickEvent, _, cx| {
                this.open_instance_dialog(None);
                cx.notify();
            }),
        );
        div()
            .flex()
            .flex_col()
            .w_full()
            .child(ui::page_head(
                tr("Profiles"),
                tr("Each profile has its own game folder, mods, version and settings. Changing one never touches another."),
            ))
            .children(self.nova_note())
            .child(status)
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .mb(px(16.))
                    .child(create)
                    .child(
                        button("nova-import-profile", "Import Profile", Kind::Secondary, &t).on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.choose_profile_import(cx);
                            }),
                        ),
                    ),
            )
            .child(rows)
    }

    /// The active profile at a glance: its version, the Java Bridge and what its mods folder holds.
    fn status_card(&mut self, cx: &mut Context<Self>) -> Div {
        use horizon_launcher::java_mods;
        let t = self.theme;
        let Some(profile) = self.selected_instance().cloned() else {
            return div();
        };
        let reload = self.nova.mods.as_ref().is_none_or(|(id, _)| *id != profile.id);
        if reload {
            let list = if profile.java_mods {
                java_mods::list(&profile.home)
            } else {
                Vec::new()
            };
            self.nova.mods = Some((profile.id.clone(), list));
        }
        let mods = self.nova.mods.as_ref().map(|(_, m)| m.clone()).unwrap_or_default();
        let enabled = mods.iter().filter(|m| m.enabled).count();
        let disabled = mods.len() - enabled;
        let problems = mods
            .iter()
            .filter(|m| {
                matches!(
                    java_mods::status(m, &mods),
                    java_mods::Status::Incompatible(_) | java_mods::Status::MissingDependency(_)
                )
            })
            .count();
        let version = match &profile.java {
            Some(java) => format!("Minecraft {}", java.minecraft),
            None => match self.version_of(&profile) {
                Some(tag) => format!("Horizon {tag}"),
                None => tr("not installed").to_string(),
            },
        };
        let bridge = if profile.java_mods {
            trf(
                "On: {enabled} mods enabled, {disabled} disabled, {problems} with problems",
                &[
                    ("enabled", &enabled.to_string()),
                    ("disabled", &disabled.to_string()),
                    ("problems", &problems.to_string()),
                ],
            )
        } else {
            tr("Off for this profile (Fabric mods are optional)").to_string()
        };
        let refresh = button("nova-status-refresh", "Refresh", Kind::Ghost, &t).on_click(
            cx.listener(|this, _: &ClickEvent, _, cx| {
                this.nova.mods = None;
                cx.notify();
            }),
        );
        div().mb(px(16.)).child(
            card()
                .child(ui::setting_row(
                    trf("Active profile: {name}", &[("name", &profile.name)]),
                    say_owned(version),
                    Some(refresh.into_any_element()),
                ))
                .child(ui::setting_row(
                    tr("Java Bridge"),
                    say_owned(bridge),
                    None,
                )),
        )
    }

    /// Asks for a profile file, then creates the profile it describes.
    fn choose_profile_import(&mut self, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(tr("Import").into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = paths.await {
                let _ = this.update(cx, |this, cx| {
                    if let Some(path) = paths.first() {
                        this.import_profile(path);
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn import_profile(&mut self, path: &std::path::Path) {
        let names: Vec<String> = self.instances.list.iter().map(|i| i.name.clone()).collect();
        let result = std::fs::read_to_string(path)
            .map_err(|e| format!("{}: {e}", path.display()))
            .and_then(|text| crate::nova_profile_io::parse(&text))
            .and_then(|file| crate::nova_profile_io::import(&file, &names));
        self.nova.note = Some(match result {
            Ok(created) => {
                self.reload_instances();
                Ok(trf(
                    "Imported {name}. Add its mods again from the Mods page.",
                    &[("name", &created.name)],
                ))
            }
            Err(e) => Err(e),
        });
    }

    fn duplicate_profile(&mut self, from: &instances::Instance, taken: &[String]) {
        let name = copy_name(taken, &from.name);
        self.nova.note = Some(match instances::create(&name, Some(&from.id)) {
            Ok(copy) => {
                self.reload_instances();
                Ok(trf("Created {name}.", &[("name", &copy.name)]))
            }
            Err(e) => Err(trf("Could not duplicate {name}: {error}", &[("name", &from.name), ("error", &e)])),
        });
    }

    fn delete_profile(&mut self, id: &str, name: &str) {
        self.nova.confirm_delete = None;
        self.nova.note = Some(match instances::delete_with_selection(id) {
            Ok((_, saved)) => {
                if self.settings.instance.as_deref() == Some(id) {
                    self.settings.instance = None;
                }
                self.reload_instances();
                match saved {
                    Ok(()) => Ok(trf("{name} is in the Trash.", &[("name", name)])),
                    Err(e) => Err(trf(
                        "{name} was deleted, but the selection could not be saved: {error}",
                        &[("name", name), ("error", &e)],
                    )),
                }
            }
            Err(e) => Err(e),
        });
    }

    // ----- Performance -------------------------------------------------------------------------

    pub(super) fn performance_page(&mut self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let total = horizon_launcher::launch::total_memory_mb();
        let profile = self.selected_instance().cloned();

        let mut profile_card = card();
        match profile.as_ref().and_then(|p| p.java.as_ref().map(|j| (p, j))) {
            Some((profile, java)) => {
                let memory = java.memory_mb.unwrap_or(instances::DEFAULT_MEMORY_MB);
                let current = Preset::detect(memory, total);
                let id = profile.id.clone();
                let picker = segments("nova-memory-preset", &Preset::ALL, current, &t, {
                    let this = cx.entity().downgrade();
                    move |choice: Preset, _, cx| {
                        let _ = this.update(cx, |this, cx| {
                            this.apply_memory_preset(&id, choice, total);
                            cx.notify();
                        });
                    }
                });
                profile_card = profile_card
                    .children(self.glass_memory_row(cx))
                    .child(ui::setting_row(
                        trf("Memory for {name}", &[("name", &profile.name)]),
                        say_owned(format!(
                            "{} {} {}",
                            memory_label(memory),
                            tr("allocated (-Xmx) of"),
                            memory_label(total)
                        ) + &format!(
                            ". {}",
                            tr("Custom is any other value: drag the slider above. Applies at the next start.")
                        )),
                        Some(picker.into_any_element()),
                    ))
                    .child(ui::setting_row(
                        tr("Java arguments"),
                        say("Extra JVM arguments of this profile. Change them in Edit on the Profiles page."),
                        Some(
                            div()
                                .font_family(theme::MONO)
                                .text_size(px(12.))
                                .text_color(white(ink::BODY))
                                .max_w(px(260.))
                                .truncate()
                                .child(if java.jvm_args.is_empty() {
                                    tr("none").to_string()
                                } else {
                                    java.jvm_args.clone()
                                })
                                .into_any_element(),
                        ),
                    ));
            }
            None => {
                profile_card = profile_card.child(ui::setting_row(
                    tr("Memory and Java arguments"),
                    say("The active profile plays Horizon, which is not a Java program: it has no heap to size. These settings apply to Minecraft: Java Edition profiles."),
                    Some(self.unavailable_pill()),
                ));
            }
        }

        let options = self.video_card(cx);
        let graphics = self.graphics_cards(cx);
        let overlay = self.overlay_card(cx);
        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .w_full()
            .child(ui::page_head(
                tr("Performance"),
                tr("Everything here is written to a file the game reads at its next start."),
            ))
            .children(self.nova_note())
            .child(ui::card_heading(tr("Renderer")))
            .child(graphics)
            .child(ui::card_note(tr(
                "The renderer is chosen when the game starts: if it is running, close it and press Play again.",
            )))
            .child(ui::card_heading(tr("Active profile")))
            .child(profile_card)
            .child(ui::card_heading(tr("Nova in-game HUD")))
            .child(overlay)
            .child(ui::card_note(tr(
                "Shown over the world while you play (not with F3 open or F1 pressed). Applies the next time the game starts.",
            )))
            .child(ui::card_heading(tr("Video options")))
            .child(options)
            .child(ui::card_note(tr(
                "Saved in the active profile's options.txt, a copy of the original kept once as options.txt.nova-backup. Close the game first; they apply the next time it starts.",
            )))
    }

    /// The HUD's switches: on or off, each element, the corner and the size.
    fn overlay_card(&mut self, cx: &mut Context<Self>) -> Div {
        use horizon_launcher::nova_overlay::{Corner, Overlay, Size, ELEMENTS};
        let t = self.theme;
        let look = ui::look();
        let current = self
            .nova
            .overlay
            .get_or_insert_with(Overlay::load)
            .clone();
        let switch = |id: String, on: bool, change: fn(&mut Overlay, &str), key: &'static str| {
            ui::toggle(id, on, look.accent, look.on_accent, &t).on_click(cx.listener(
                move |this, _: &ClickEvent, _, cx| {
                    this.change_overlay(|o| change(o, key));
                    cx.notify();
                },
            ))
        };
        let mut rows = card().child(ui::setting_row(
            tr("Show the HUD"),
            say("FPS, frame time, memory, coordinates and the profile's name, drawn by the game itself. Press F10 in the game to hide or show it."),
            Some(
                switch(
                    "nova-hud-on".into(),
                    current.enabled,
                    |o, _| o.enabled = !o.enabled,
                    "",
                )
                .into_any_element(),
            ),
        ));
        for (token, label) in ELEMENTS {
            rows = rows.child(ui::setting_row(
                tr(label),
                None,
                Some(
                    switch(
                        format!("nova-hud-{token}"),
                        current.elements.contains(token),
                        |o, key| {
                            if !o.elements.remove(key) {
                                o.elements.insert(key.to_string());
                            }
                        },
                        token,
                    )
                    .into_any_element(),
                ),
            ));
        }
        let corner = segments(
            "nova-hud-corner",
            &[(Corner::TopLeft, "Top left"), (Corner::TopRight, "Top right")],
            current.corner,
            &t,
            {
                let this = cx.entity().downgrade();
                move |choice: Corner, _, cx| {
                    let _ = this.update(cx, |this, cx| {
                        this.change_overlay(|o| o.corner = choice);
                        cx.notify();
                    });
                }
            },
        );
        let size = segments(
            "nova-hud-size",
            &[(Size::Small, "Small"), (Size::Normal, "Normal"), (Size::Large, "Large")],
            current.size,
            &t,
            {
                let this = cx.entity().downgrade();
                move |choice: Size, _, cx| {
                    let _ = this.update(cx, |this, cx| {
                        this.change_overlay(|o| o.size = choice);
                        cx.notify();
                    });
                }
            },
        );
        rows.child(ui::setting_row(
            tr("Position"),
            say("Ping is not offered: the game has no ping reading to show yet."),
            Some(corner.into_any_element()),
        ))
        .child(ui::setting_row(tr("Size"), None, Some(size.into_any_element())))
    }

    fn change_overlay(&mut self, change: impl FnOnce(&mut horizon_launcher::nova_overlay::Overlay)) {
        let mut overlay = self
            .nova
            .overlay
            .take()
            .unwrap_or_else(horizon_launcher::nova_overlay::Overlay::load);
        change(&mut overlay);
        self.nova.note = Some(match overlay.save() {
            Ok(()) => Ok(tr("HUD settings saved. They apply the next time the game starts.").to_string()),
            Err(e) => Err(trf("Could not save the HUD settings: {error}", &[("error", &e)])),
        });
        self.nova.overlay = Some(overlay);
    }

    /// The graphics API as a list of radio choices: Auto, Vulkan, OpenGL (and DirectX 12 on
    /// Windows). Choosing one writes what the game reads at its next start
    /// (`backend::set_graphics_api`); the list is the same one the Settings page offers.
    fn graphics_cards(&mut self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let look = ui::look();
        let home = self.client_home.clone().unwrap_or_else(backend::game_home);
        let current = backend::selected_graphics_api(&home).unwrap_or_else(|_| "auto".into());
        let options: &[(&'static str, &'static str, &'static str)] = if cfg!(target_os = "macos") {
            &[("auto", "Automatic (Metal)", "The only renderer on this system.")]
        } else if cfg!(windows) {
            &[
                ("auto", "Auto", "Lets the game pick the best renderer for your graphics card."),
                ("vulkan", "Vulkan", "Modern, low-overhead renderer."),
                ("dx12", "DirectX 12", "Windows' own modern renderer."),
                ("opengl", "OpenGL", "Older renderer; try it if Vulkan gives trouble."),
            ]
        } else {
            &[
                ("auto", "Auto", "Lets the game pick the best renderer for your graphics card."),
                ("vulkan", "Vulkan", "Modern, low-overhead renderer."),
                ("opengl", "OpenGL", "Older renderer; try it if Vulkan gives trouble."),
            ]
        };
        let mut rows = card();
        for (api, label, about) in options {
            let selected = *api == current;
            let radio = div()
                .flex()
                .items_center()
                .justify_center()
                .flex_none()
                .size(px(20.))
                .rounded_full()
                .border_2()
                .border_color(if selected { ui::color(look.accent_soft) } else { white(0.35) })
                .child(div().size(px(10.)).rounded_full().bg(if selected {
                    ui::color(look.accent_soft)
                } else {
                    white(0.0)
                }));
            let api = *api;
            let home = home.clone();
            rows = rows.child(
                ui::row(format!("nova-api-{api}"), selected, 12., &t)
                    .child(radio)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(ui::text(15., FontWeight::MEDIUM, ink::FULL).child(label.to_string()))
                            .child(ui::card_note(tr(about).to_string())),
                    )
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.nova.note = Some(match backend::set_graphics_api(&home, api) {
                            Ok(()) if this.client.is_some() => Ok(tr(
                                "Saved. The game is running: close it and press Play to use the new renderer.",
                            )
                            .to_string()),
                            Ok(()) => Ok(tr("Saved. The new renderer is used the next time the game starts.").to_string()),
                            Err(e) => Err(e),
                        });
                        cx.notify();
                    })),
            );
        }
        rows
    }

    /// FPS limit, VSync, render and simulation distance, fullscreen and a distance preset, read
    /// from and written to the active profile's `options.txt` (`nova_options`).
    fn video_card(&mut self, cx: &mut Context<Self>) -> Div {
        use crate::nova_options as o;
        let t = self.theme;
        let look = ui::look();
        let Some(profile) = self.selected_instance().cloned() else {
            return card().child(ui::card_note(tr("No profile selected.").to_string()));
        };
        let Ok(text) = std::fs::read_to_string(profile.home.join("options.txt")) else {
            return card().child(ui::setting_row(
                tr("Video options"),
                say("This profile has no options.txt yet. Start the game once so it creates its options, then change them here."),
                None,
            ));
        };
        // A key the file lacks reads as the game's own default.
        let fps = o::get_int(&text, o::MAX_FPS).unwrap_or(120);
        let render = o::get_int(&text, o::RENDER).unwrap_or(12);
        let simulation = o::get_int(&text, o::SIMULATION).unwrap_or(12);
        let vsync = o::get_bool(&text, o::VSYNC).unwrap_or(true);
        let fullscreen = o::get_bool(&text, o::FULLSCREEN).unwrap_or(false);

        let pick = |id: &'static str, options: &[(i32, &'static str)], current: i32, key: &'static str| {
            segments(id, options, current, &t, {
                let this = cx.entity().downgrade();
                move |value: i32, _, cx| {
                    let _ = this.update(cx, |this, cx| {
                        this.set_video_option(key, value.to_string());
                        cx.notify();
                    });
                }
            })
        };
        let preset = segments(
            "nova-video-preset",
            &[("Low", "Low"), ("Balanced", "Balanced"), ("High", "High"), ("Custom", "Custom")],
            o::detect(render, simulation),
            &t,
            {
                let this = cx.entity().downgrade();
                move |choice: &'static str, _, cx| {
                    let _ = this.update(cx, |this, cx| {
                        match o::distances(choice) {
                            Some((r, s)) => {
                                this.set_video_option(o::RENDER, r.to_string());
                                this.set_video_option(o::SIMULATION, s.to_string());
                            }
                            None => {
                                this.nova.note = Some(Ok(tr("Custom: set the distances below to any values.").to_string()));
                            }
                        }
                        cx.notify();
                    });
                }
            },
        );
        let switch = |id: &'static str, on: bool, key: &'static str| {
            ui::toggle(id, on, look.accent, look.on_accent, &t).on_click(cx.listener(
                move |this, _: &ClickEvent, _, cx| {
                    this.set_video_option(key, (!on).to_string());
                    cx.notify();
                },
            ))
        };
        card()
            .child(ui::setting_row(
                tr("Distance preset"),
                say("Low 8/6, Balanced 12/12, High 20/16 (render / simulation chunks)."),
                Some(preset.into_any_element()),
            ))
            .child(ui::setting_row(
                tr("FPS limit"),
                None,
                Some(pick(
                    "nova-fps",
                    &[(30, "30"), (60, "60"), (120, "120"), (240, "240"), (o::FPS_UNLIMITED, "Unlimited")],
                    fps,
                    o::MAX_FPS,
                ).into_any_element()),
            ))
            .child(ui::setting_row(
                tr("VSync"),
                None,
                Some(switch("nova-vsync", vsync, o::VSYNC).into_any_element()),
            ))
            .child(ui::setting_row(
                tr("Render distance"),
                None,
                Some(pick(
                    "nova-render",
                    &[(6, "6"), (8, "8"), (12, "12"), (16, "16"), (24, "24")],
                    render,
                    o::RENDER,
                ).into_any_element()),
            ))
            .child(ui::setting_row(
                tr("Simulation distance"),
                None,
                Some(pick(
                    "nova-simulation",
                    &[(5, "5"), (8, "8"), (12, "12"), (16, "16"), (24, "24")],
                    simulation,
                    o::SIMULATION,
                ).into_any_element()),
            ))
            .child(ui::setting_row(
                tr("Fullscreen"),
                None,
                Some(switch("nova-fullscreen", fullscreen, o::FULLSCREEN).into_any_element()),
            ))
    }

    /// Writes one video option into the active profile's options.txt. Refused while the game
    /// runs: it rewrites the file when it closes and would undo the change.
    fn set_video_option(&mut self, key: &'static str, value: String) {
        if self.client.is_some() || self.starting_client.is_some() {
            self.nova.note = Some(Err(tr("Close the game first: it rewrites its options when it closes.").to_string()));
            return;
        }
        let Some(profile) = self.selected_instance().cloned() else {
            return;
        };
        self.nova.note = Some(
            crate::nova_options::change(&profile.home, key, &value)
                .map(|_| tr("Saved in this profile's options.txt. It applies the next time the game starts.").to_string()),
        );
    }

    fn dash_activity(&mut self) -> Div {
        let crashes = self.crashes.snapshot();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let items = crate::nova_activity::recent(&self.instances.list, &crashes, 5);
        let mut card = dash_card("Recent Activity", "folder-01", None, 260.);
        if items.is_empty() {
            card = card.child(ui::card_note(
                tr("Nothing yet. Play a profile or analyse a log and it shows up here.").to_string(),
            ));
        }
        for item in items {
            card = card.child(
                div()
                    .flex()
                    .flex_col()
                    .child(ui::text(14., FontWeight::MEDIUM, ink::FULL).truncate().child(item.text))
                    .child(ui::card_note(crate::nova_activity::ago(now, item.at))),
            );
        }
        card
    }

    fn unavailable_pill(&self) -> AnyElement {
        div()
            .px(px(12.))
            .h(px(28.))
            .flex()
            .items_center()
            .rounded(px(radius::CONTROL))
            .bg(white(0.06))
            .text_size(px(13.))
            .text_color(white(ink::MUTED))
            .child(tr("Not available").to_string())
            .into_any_element()
    }

    fn apply_memory_preset(&mut self, id: &str, preset: Preset, total_mb: u32) {
        let Some(memory) = preset.memory_mb(total_mb) else {
            self.nova.note = Some(Ok(tr(
                "Custom keeps the current value. Drag the memory slider in the profile's Play settings to change it.",
            )
            .to_string()));
            return;
        };
        let saved = instances::edit(id, |m| {
            if let Some(java) = &mut m.java {
                java.memory_mb = Some(memory);
            }
        });
        self.nova.note = Some(match saved {
            Ok(i) => {
                self.reload_instances();
                Ok(trf(
                    "{name} now gets {memory}. It applies at the next start.",
                    &[("name", &i.name), ("memory", &memory_label(memory))],
                ))
            }
            Err(e) => Err(e),
        });
    }

    // ----- About -------------------------------------------------------------------------------

    pub(super) fn about_page(&mut self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let link = |id: &'static str, label: &str, url: &'static str| {
            ui::link(id, label.to_string()).on_click(move |_, _, cx| cx.open_url(url))
        };
        // The Nova mark (`nova.svg` in the glass icons) on the accent.
        let logo = div()
            .flex()
            .items_center()
            .justify_center()
            .size(px(72.))
            .rounded(px(20.))
            .bg(ui::color(ui::look().accent))
            .child(ui::icon("nova", 44., ui::color(ui::look().on_accent)));
        let identity = card().child(
            div()
                .flex()
                .items_center()
                .gap(px(20.))
                .p(px(16.))
                .child(logo)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .child(ui::text(26., FontWeight::SEMIBOLD, ink::FULL).child("Nova Client"))
                        .child(ui::card_note(format!("{} {VERSION}", tr("Version"))))
                        .child(ui::card_note(tr("A Minecraft client built on Horizon."))),
                ),
        );
        let credits = card()
            .child(ui::setting_row(
                "Horizon",
                say("The Rust Minecraft client Nova Client is built on, by Ludovic Marie and the Horizon contributors. MIT licence."),
                Some(link("about-horizon", "Repository", HORIZON_REPO).into_any_element()),
            ))
            .child(ui::setting_row(
                "Horizon Java Bridge",
                say("Optional bridge that runs Fabric mods inside Horizon, by iaxivers, with Windows support by martipops. Not every Fabric API is supported: see the bridge's known limitations."),
                Some(link("about-bridge", "Repository", BRIDGE_REPO).into_any_element()),
            ))
            .child(ui::setting_row(
                tr("Licences"),
                say("Nova Client keeps Horizon's MIT licence and every notice in THIRD_PARTY_NOTICES.md. Dependencies keep their own licences."),
                Some(
                    button("about-licences", "Open licence folder", Kind::Secondary, &t)
                        .on_click(|_, _, cx| {
                            let dir = std::env::current_exe()
                                .ok()
                                .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                                .unwrap_or_default();
                            crate::glass_toast::open(cx, &dir);
                        })
                        .into_any_element(),
                ),
            ))
            .child(ui::setting_row(
                tr("Logs"),
                say("Launcher logs and crash reports."),
                Some(
                    button("about-logs", "Open logs", Kind::Secondary, &t)
                        .on_click(|_, _, cx| {
                            let dir = backend::logs_dir();
                            let _ = std::fs::create_dir_all(&dir);
                            crate::glass_toast::open(cx, &dir);
                        })
                        .into_any_element(),
                ),
            ));
        let _ = cx;
        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .w_full()
            .child(ui::page_head(
                tr("About"),
                tr("Credits, licences and where the code lives."),
            ))
            .child(identity)
            .child(credits)
    }
}

// ----- Home dashboard (the concept's cards under the status tiles) ------------------------------

/// A dashboard card: the plate with its icon, title and an optional element on the right.
fn dash_card(title: &str, icon: &'static str, right: Option<AnyElement>, min_w: f32) -> Div {
    crate::glass::plate(radius::LARGE)
        .flex()
        .flex_col()
        .flex_1()
        .min_w(px(min_w))
        .p(px(16.))
        .gap(px(10.))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .child(ui::icon(icon, 20., ui::color(ui::look().accent_soft)))
                        .child(ui::text(16., FontWeight::SEMIBOLD, ink::FULL).child(tr(title).to_string())),
                )
                .children(right),
        )
}

impl Launcher {
    pub(super) fn nova_dashboard(&mut self, cx: &mut Context<Self>) -> Div {
        let top = div()
            .flex()
            .flex_wrap()
            .gap(px(12.))
            .child(self.dash_mods(cx))
            .child(self.dash_profiles(cx))
            .child(self.dash_performance(cx));
        let bottom = div()
            .flex()
            .flex_wrap()
            .gap(px(12.))
            .child(self.dash_overlay(cx))
            .child(self.dash_analyzer(cx))
            .child(self.dash_quick(cx))
            .child(self.dash_activity());
        div().mt(px(16.)).flex().flex_col().gap(px(12.)).child(top).child(bottom)
    }

    /// The active profile's Java mods, read once per profile (see `status_card`).
    fn cached_mods(&mut self, profile: &instances::Instance) -> Vec<horizon_launcher::java_mods::JavaMod> {
        let reload = self.nova.mods.as_ref().is_none_or(|(id, _)| *id != profile.id);
        if reload {
            let list = if profile.java_mods {
                horizon_launcher::java_mods::list(&profile.home)
            } else {
                Vec::new()
            };
            self.nova.mods = Some((profile.id.clone(), list));
        }
        self.nova.mods.as_ref().map(|(_, m)| m.clone()).unwrap_or_default()
    }

    /// How many mod jars the active profile has installed (enabled or not).
    pub(super) fn installed_mods(&mut self, profile: &instances::Instance) -> usize {
        if profile.java_mods {
            self.cached_mods(profile).len()
        } else {
            std::fs::read_dir(profile.home.join("mods"))
                .map(|d| {
                    d.flatten()
                        .filter(|e| e.path().extension().is_some_and(|x| x == "jar"))
                        .count()
                })
                .unwrap_or(0)
        }
    }

    /// What the active profile can take from the mod browser, and where: the version and loader
    /// its mods must be made for, its folder, and whether they go in the Java Bridge's
    /// `mods-java/` (a Horizon profile with Fabric mods on) or `mods/` (a Java Edition profile).
    fn nova_mod_target(&self) -> Option<(horizon_launcher::browser::java_instance::Target, std::path::PathBuf, bool)> {
        let profile = self.selected_instance()?;
        if let Some(java) = &profile.java {
            return Some((java.target(), profile.home.clone(), false));
        }
        profile.java_mods.then(|| {
            (
                horizon_launcher::browser::java_instance::Target {
                    minecraft: horizon_launcher::java_mods::MINECRAFT.to_string(),
                    loader: Some("fabric".into()),
                },
                profile.home.clone(),
                true,
            )
        })
    }

    /// Searches Modrinth, in the background, for mods that run in the active profile.
    pub(super) fn nova_search_start(&mut self) {
        use horizon_launcher::browser::{modrinth::Modrinth, Kind as ModKind, Source};
        let text = self.servers.value(Field::NovaModSearch).trim().to_string();
        self.nova.search_error = None;
        if text.is_empty() {
            self.nova.results.clear();
            return;
        }
        let Some((target, _, _)) = self.nova_mod_target() else {
            self.nova.search_error = Some(tr("This profile takes no mods yet. Turn on Fabric mods for it, or use a Java Edition profile.").to_string());
            return;
        };
        if let Err(e) = target.folder(ModKind::Mod) {
            self.nova.search_error = Some(e);
            return;
        }
        self.nova.generation += 1;
        let generation = self.nova.generation;
        self.nova.searching = true;
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let result = Modrinth.search(&target.query(&text, ModKind::Mod));
            let _ = tx.send(Msg::NovaSearch(generation, result));
        });
    }

    pub(super) fn nova_on_search(
        &mut self,
        generation: u64,
        result: Result<Vec<horizon_launcher::browser::Project>, String>,
    ) {
        if generation != self.nova.generation {
            return;
        }
        self.nova.searching = false;
        match result {
            Ok(list) => {
                self.nova.results = list;
                self.nova.search_error = None;
            }
            Err(e) => {
                self.nova.results.clear();
                self.nova.search_error = Some(e);
            }
        }
    }

    /// Adds `project` (and the mods it requires) from Modrinth, in the background. The download is
    /// checked against the checksum Modrinth publishes before it is kept.
    fn nova_install(&mut self, project: horizon_launcher::browser::Project) {
        use horizon_launcher::browser::{java_instance, modrinth::Modrinth, Source};
        if self.nova.installing.is_some() {
            return;
        }
        let Some((target, home, bridge)) = self.nova_mod_target() else {
            return;
        };
        self.nova.installing = Some(project.id.clone());
        self.nova.install_note = None;
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let result = (|| -> Result<String, String> {
                let files = if bridge {
                    let dir = horizon_launcher::java_mods::dir(&home);
                    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
                    let steps = java_instance::plan(&Modrinth, &target, &project)?;
                    for step in &steps {
                        let file = step
                            .version
                            .file()
                            .ok_or_else(|| format!("{} has no file to download.", step.project.title))?;
                        Modrinth.fetch(file, &dir, &|_| {})?;
                    }
                    steps.len()
                } else {
                    java_instance::install(&Modrinth, &target, &project, &home, &|_, _| {})?.len()
                };
                Ok(match files {
                    1 => format!("Added {}.", project.title),
                    n => format!("Added {} and {} required mod(s).", project.title, n - 1),
                })
            })();
            let _ = tx.send(Msg::NovaInstall(result));
        });
    }

    pub(super) fn nova_on_install(&mut self, result: Result<String, String>) {
        self.nova.installing = None;
        self.nova.mods = None;
        self.java_mods.stale();
        self.nova.install_note = Some(result);
    }

    /// Before Play: a Fabric-mods profile with serious problems (an incompatible jar or a missing
    /// dependency) is warned about once; pressing Play again starts it anyway. Whether this press
    /// was held back.
    pub(super) fn nova_prelaunch_blocked(&mut self) -> bool {
        use horizon_launcher::java_mods::{self, Status};
        let Some(profile) = self.selected_instance().cloned() else {
            return false;
        };
        if !profile.java_mods || profile.java.is_some() {
            return false;
        }
        let mods = java_mods::list(&profile.home);
        let problems: Vec<String> = mods
            .iter()
            .filter(|m| m.enabled)
            .filter_map(|m| match java_mods::status(m, &mods) {
                Status::Incompatible(why) | Status::MissingDependency(why) => {
                    Some(format!("{}: {}", m.title(), why.first().cloned().unwrap_or_default()))
                }
                _ => None,
            })
            .collect();
        if problems.is_empty() || self.nova.warned_for.as_deref() == Some(profile.id.as_str()) {
            self.nova.warned_for = None;
            return false;
        }
        self.nova.warned_for = Some(profile.id.clone());
        crate::glass_toast::say(format!(
            "{} mod problem(s), the game may not start. First: {} Press Play again to start anyway, or fix them on the Mods page.",
            problems.len(),
            problems[0]
        ));
        true
    }

    /// Writes the active profile's Java mod list as `.txt` and `.json` in the Nova exports folder.
    pub(super) fn export_mod_list(&mut self) {
        let Some(profile) = self.selected_instance().cloned() else {
            return;
        };
        let mods = horizon_launcher::java_mods::list(&profile.home);
        let dir = crate::nova_crash_history::export_dir();
        let clean: String = profile
            .name
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        let write = || -> Result<std::path::PathBuf, String> {
            std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            let txt = dir.join(format!("{clean}-mods.txt"));
            std::fs::write(&txt, mod_list_text(&profile.name, &mods)).map_err(|e| format!("{}: {e}", txt.display()))?;
            let json = dir.join(format!("{clean}-mods.json"));
            std::fs::write(&json, mod_list_json(&profile.name, &mods)?).map_err(|e| format!("{}: {e}", json.display()))?;
            Ok(dir.clone())
        };
        self.nova.install_note = Some(write().map(|d| {
            trf("Mod list saved in {path}.", &[("path", &d.display().to_string())])
        }));
    }

    fn dash_mods(&mut self, cx: &mut Context<Self>) -> Div {
        use horizon_launcher::java_mods::{self, Status};
        let t = self.theme;
        let look = ui::look();
        let all = button("dash-mods-all", "View All", Kind::Ghost, &t).on_click(cx.listener(
            |this, _: &ClickEvent, _, cx| {
                this.go(Page::Mods);
                cx.notify();
            },
        ));
        let mut card = dash_card("Mods", "puzzle", Some(all.into_any_element()), 300.);
        let Some(profile) = self.selected_instance().cloned() else {
            return card.child(ui::card_note(tr("No profile selected.").to_string()));
        };
        if self.nova_mod_target().is_none() {
            return card.child(ui::card_note(
                tr("Fabric mods are off for this profile. Turn them on in the profile's settings to search and manage them here.").to_string(),
            ));
        }

        // Search Modrinth and add: the field, the results, and what the last add did.
        let field = self.field(Field::NovaModSearch, tr("Search Modrinth..."), 220., cx);
        let go = button("dash-mods-search", "Search", Kind::Secondary, &t).on_click(cx.listener(
            |this, _: &ClickEvent, _, cx| {
                this.nova_search_start();
                cx.notify();
            },
        ));
        card = card.child(div().flex().items_center().gap(px(8.)).child(field).child(go));
        if self.nova.searching {
            card = card.child(ui::card_note(tr("Searching Modrinth...").to_string()));
        }
        if let Some(e) = &self.nova.search_error {
            card = card.child(ui::text(13., FontWeight::MEDIUM, 1.).text_color(ui::danger()).child(e.clone()));
        }
        match &self.nova.install_note {
            Some(Ok(said)) => {
                card = card.child(ui::text(13., FontWeight::MEDIUM, 1.).text_color(ui::live()).child(said.clone()));
            }
            Some(Err(e)) => {
                card = card.child(ui::text(13., FontWeight::MEDIUM, 1.).text_color(ui::danger()).child(e.clone()));
            }
            None => {}
        }
        let results = self.nova.results.clone();
        for (n, project) in results.iter().take(5).enumerate() {
            let busy = self.nova.installing.as_deref() == Some(project.id.as_str());
            let picked = project.clone();
            let add = button(
                format!("dash-mod-add-{n}"),
                if busy { "Adding..." } else { "Add" },
                Kind::Primary,
                &t,
            )
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.nova_install(picked.clone());
                cx.notify();
            }));
            card = card.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .min_w_0()
                            .child(ui::text(14., FontWeight::MEDIUM, ink::FULL).truncate().child(project.title.clone()))
                            .child(ui::card_note(format!(
                                "{} · {} {}",
                                project.author,
                                project.downloads,
                                tr("downloads")
                            ))),
                    )
                    .child(add),
            );
        }
        card = card.child(ui::card_note(tr("Source: Modrinth. CurseForge is not available yet: its API needs a developer key.").to_string()));

        if profile.java_mods {
            card = card.child(
                button("dash-mods-export", "Export Mod List", Kind::Ghost, &t).on_click(cx.listener(
                    |this, _: &ClickEvent, _, cx| {
                        this.export_mod_list();
                        cx.notify();
                    },
                )),
            );
        }
        // What is installed (the Java Bridge's mods-java folder); Java Edition profiles list
        // theirs on the Mods page.
        if !profile.java_mods {
            return card.child(ui::card_note(
                tr("Installed mods of a Java Edition profile are listed on the Mods page.").to_string(),
            ));
        }
        let mods = self.cached_mods(&profile);
        if mods.is_empty() {
            return card.child(ui::card_note(
                tr("No Java mods installed yet.").to_string(),
            ));
        }
        for (n, m) in mods.iter().take(4).enumerate() {
            let status = java_mods::status(m, &mods);
            let colour = match status {
                Status::Compatible => ui::live(),
                Status::Warning(_) => ui::warn(),
                Status::Disabled => white(ink::MUTED),
                _ => ui::danger(),
            };
            let (home, file, enabled) = (profile.home.clone(), m.file.clone(), m.enabled);
            let switch = ui::toggle(format!("dash-mod-{n}"), enabled, look.accent, look.on_accent, &t)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.toggle_java_mod(home.clone(), file.clone(), !enabled);
                    this.nova.mods = None;
                    cx.notify();
                }));
            card = card.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .min_w_0()
                            .child(ui::text(14., FontWeight::MEDIUM, ink::FULL).truncate().child(m.title().to_string()))
                            .child(
                                ui::text(12., FontWeight::NORMAL, 1.)
                                    .text_color(colour)
                                    .child(tr(status.label()).to_string()),
                            ),
                    )
                    .child(switch),
            );
        }
        if mods.len() > 4 {
            card = card.child(ui::card_note(trf(
                "{count} more on the Mods page.",
                &[("count", &(mods.len() - 4).to_string())],
            )));
        }
        card
    }

    fn dash_profiles(&mut self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let create = button("dash-profile-new", "Create Profile", Kind::Secondary, &t).on_click(
            cx.listener(|this, _: &ClickEvent, _, cx| {
                this.open_instance_dialog(None);
                cx.notify();
            }),
        );
        let mut cards = div().flex().gap(px(8.));
        let selected = self.selected_instance().map(|i| i.id.clone());
        let list = self.instances.list.clone();
        for (n, instance) in list.iter().take(3).enumerate() {
            let active = selected.as_deref() == Some(instance.id.as_str());
            let version = match &instance.java {
                Some(java) => format!("Minecraft {}", java.minecraft),
                None => self.version_of(instance).map_or_else(
                    || tr("not installed").to_string(),
                    |tag| format!("Horizon {tag}"),
                ),
            };
            let picked = instance.clone();
            let choose = button(
                format!("dash-profile-{n}"),
                if active { "Selected" } else { "Select" },
                if active { Kind::Primary } else { Kind::Secondary },
                &t,
            )
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.select_instance(&picked);
                cx.notify();
            }));
            cards = cards.child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .gap(px(6.))
                    .p(px(10.))
                    .rounded(px(radius::ROW))
                    .bg(white(if active { 0.10 } else { 0.04 }))
                    .child(ui::text(14., FontWeight::SEMIBOLD, ink::FULL).truncate().child(instance.name.clone()))
                    .child(ui::card_note(version))
                    .child(choose),
            );
        }
        dash_card("Profiles", "user-multiple-02", Some(create.into_any_element()), 320.).child(cards)
    }

    fn dash_performance(&mut self, cx: &mut Context<Self>) -> Div {
        let mut card = dash_card("Performance", "sliders-horizontal", None, 320.)
            .child(self.graphics_api_row(cx));
        match self.glass_memory_row(cx) {
            Some(slider) => card = card.child(slider),
            None => {
                card = card.child(ui::card_note(
                    tr("RAM applies to Minecraft: Java Edition profiles. The active profile plays Horizon, which has no heap to size.").to_string(),
                ));
            }
        }
        card
    }

    fn dash_overlay(&mut self, cx: &mut Context<Self>) -> Div {
        use horizon_launcher::nova_overlay::{Overlay, ELEMENTS};
        let t = self.theme;
        let overlay = self.nova.overlay.get_or_insert_with(Overlay::load).clone();
        let status = div()
            .flex()
            .items_center()
            .gap(px(6.))
            .child(ui::dot(if overlay.enabled { ui::Tone::Live } else { ui::Tone::Off }))
            .child(
                ui::text(13., FontWeight::MEDIUM, ink::FULL)
                    .child(tr(if overlay.enabled { "Active" } else { "Off" }).to_string()),
            );
        let mut card = dash_card("In-Game Overlay", "computer", Some(status.into_any_element()), 300.);
        let shown: Vec<&str> = ELEMENTS
            .iter()
            .filter(|(token, _)| overlay.elements.contains(*token))
            .map(|(_, label)| *label)
            .collect();
        card = card.child(ui::card_note(if shown.is_empty() {
            tr("No elements selected.").to_string()
        } else {
            shown.iter().map(|l| tr(l)).collect::<Vec<_>>().join(", ")
        }));
        card.child(
            button("dash-overlay-config", "Configure Overlay", Kind::Secondary, &t).on_click(
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.go(Page::Performance);
                    cx.notify();
                }),
            ),
        )
    }

    fn dash_analyzer(&mut self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let mut card = dash_card("Crash Analyzer", "ai-chat-02", None, 320.).child(ui::card_note(
            tr("Reads your newest log or crash report and looks for the cause. Nothing leaves your computer.").to_string(),
        ));
        match &self.nova.analysis {
            None => {}
            Some(Err(e)) => {
                card = card.child(ui::text(13., FontWeight::MEDIUM, 1.).text_color(ui::danger()).child(e.clone()));
            }
            Some(Ok((file, a))) => {
                let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                card = card
                    .child(ui::text(14., FontWeight::SEMIBOLD, ink::FULL).child(tr(a.kind.label()).to_string()))
                    .child(ui::card_note(format!(
                        "{} · {}% {} · {name}",
                        tr(if a.confirmed { "Confirmed by the log" } else { "Possible cause, not confirmed" }),
                        a.confidence,
                        tr("confidence"),
                    )))
                    .child(ui::card_note(a.summary.clone()));
                for line in &a.excerpt {
                    card = card.child(
                        div()
                            .font_family(theme::MONO)
                            .text_size(px(11.))
                            .text_color(white(ink::NOTE))
                            .truncate()
                            .child(line.clone()),
                    );
                }
                for fix in a.suggestions.iter().take(2) {
                    card = card.child(ui::card_note(format!("• {fix}")));
                }
                let (copy_text, open_path) = (report_text(file, a), file.clone());
                card = card.child(
                    div()
                        .flex()
                        .gap(px(8.))
                        .child(button("dash-analyzer-copy", "Copy report", Kind::Ghost, &t).on_click(
                            move |_, _, cx| {
                                cx.write_to_clipboard(gpui::ClipboardItem::new_string(copy_text.clone()));
                            },
                        ))
                        .child(button("dash-analyzer-open", "Open log", Kind::Ghost, &t).on_click(
                            move |_, _, cx| crate::glass_toast::open(cx, &open_path),
                        )),
                );
            }
        }
        card.child(
            div().flex().flex_wrap().gap(px(8.))
                .child(button("dash-analyzer-run", "Analyze Latest", Kind::Primary, &t).on_click(cx.listener(
                    |this, _: &ClickEvent, _, cx| {
                        this.run_analyzer();
                        cx.notify();
                    },
                )))
                .child(button("dash-analyzer-history", "Open History", Kind::Secondary, &t).on_click(cx.listener(
                    |this, _: &ClickEvent, _, cx| {
                        this.go(Page::Crashes);
                        cx.notify();
                    },
                ))),
        )
    }

    fn run_analyzer(&mut self) {
        let (minecraft, profile) = self.nova_profile_facts();
        self.nova.analysis = Some(match newest_log() {
            None => Err(tr("No log or crash report found yet. Start the game once first.").to_string()),
            Some(path) => read_tail(&path, 512 * 1024).map(|text| {
                let analysis = horizon_launcher::crash_analyzer::analyze(&text);
                if let Err(error) = crate::nova_crash_history::record(&path, &analysis, &minecraft, &profile) {
                    eprintln!("Nova crash history: {error}");
                }
                (path, analysis)
            }),
        });
    }

    fn dash_quick(&mut self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let mods_dir = self.selected_instance().map(|i| {
            if i.java_mods {
                horizon_launcher::java_mods::dir(&i.home)
            } else {
                i.home.join("mods")
            }
        });
        let action = |id: &'static str, label: &'static str| button(id, label, Kind::Secondary, &t).w_full();
        dash_card("Quick Actions", "play", None, 260.)
            .child(action("dash-open-mods", "Open Mods Folder").on_click(move |_, _, cx| {
                if let Some(dir) = &mods_dir {
                    if std::fs::create_dir_all(dir).is_ok() {
                        crate::glass_toast::open(cx, dir);
                    }
                }
            }))
            .child(action("dash-open-logs", "Open Logs Folder").on_click(|_, _, cx| {
                let dir = backend::logs_dir();
                let _ = std::fs::create_dir_all(&dir);
                crate::glass_toast::open(cx, &dir);
            }))
            .child(action("dash-updates", "Check for Updates").on_click(cx.listener(
                |this, _: &ClickEvent, _, cx| {
                    this.check_updates();
                    this.nova.note = Some(Ok(tr("Checking for updates. The result appears in Settings.").to_string()));
                    cx.notify();
                },
            )))
    }
}

#[cfg(test)]
mod dashboard_tests {
    use super::*;

    fn jar(file: &str, version: &str, enabled: bool) -> horizon_launcher::java_mods::JavaMod {
        horizon_launcher::java_mods::JavaMod {
            file: file.into(),
            id: Some(file.trim_end_matches(".jar").into()),
            name: Some(file.into()),
            version: Some(version.into()),
            size: 1,
            wrong_minecraft: None,
            enabled,
            minecraft_verified: true,
            depends: vec![],
            provides: vec![],
            breaks: vec![],
            conflicts: vec![],
            authors: vec![],
            loader: horizon_launcher::java_mods::Loader::Fabric,
            icon: None,
        }
    }

    #[test]
    fn the_mod_list_names_each_mod_its_state_and_status() {
        let mods = vec![jar("a.jar", "1.0", true), jar("b.jar", "2.0", false)];
        let text = mod_list_text("Survival", &mods);
        assert!(text.contains("Profile: Survival"));
        assert!(text.contains("a.jar 1.0 [enabled]"));
        assert!(text.contains("b.jar 2.0 [disabled]"));
        let json = mod_list_json("Survival", &mods).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["mods"].as_array().unwrap().len(), 2);
        assert_eq!(value["mods"][1]["status"], "Disabled");
    }
    use horizon_launcher::crash_analyzer::analyze;

    #[test]
    fn the_report_names_its_certainty_and_cause() {
        let a = analyze("java.lang.OutOfMemoryError: Java heap space");
        let text = report_text(std::path::Path::new("latest.log"), &a);
        assert!(text.contains("Confirmed by the log"));
        assert!(text.contains("Out of memory"));
        assert!(text.contains("Suggested fixes"));
    }

    #[test]
    fn an_unknown_log_is_not_called_confirmed() {
        let a = analyze("nothing recognisable");
        let text = report_text(std::path::Path::new("x.log"), &a);
        assert!(text.contains("not confirmed"));
    }

    #[test]
    fn only_the_end_of_a_big_log_is_read() {
        let path = std::env::temp_dir().join("nova-tail-test.log");
        std::fs::write(&path, "a".repeat(100) + "END").unwrap();
        let tail = read_tail(&path, 10).unwrap();
        assert_eq!(tail.len(), 10);
        assert!(tail.ends_with("END"));
        let _ = std::fs::remove_file(&path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_are_capped_by_the_computers_memory() {
        assert_eq!(Preset::Low.memory_mb(16384), Some(2048));
        assert_eq!(Preset::Balanced.memory_mb(16384), Some(4096));
        assert_eq!(Preset::High.memory_mb(16384), Some(8192));
        // 4 GiB machine: three quarters is 3 GiB.
        assert_eq!(Preset::High.memory_mb(4096), Some(3072));
        // Tiny machine: never under 1 GiB.
        assert_eq!(Preset::Low.memory_mb(1024), Some(1024));
        assert_eq!(Preset::Custom.memory_mb(16384), None);
    }

    #[test]
    fn a_preset_is_detected_from_its_memory_else_custom() {
        assert_eq!(Preset::detect(4096, 16384), Preset::Balanced);
        assert_eq!(Preset::detect(5000, 16384), Preset::Custom);
    }

    #[test]
    fn copies_get_a_free_name() {
        let taken = vec!["Survival".to_string()];
        assert_eq!(copy_name(&taken, "Survival"), "Survival copy");
        let taken = vec!["Survival".into(), "survival copy".into()];
        assert_eq!(copy_name(&taken, "Survival"), "Survival copy 2");
    }

    #[test]
    fn memory_reads_in_gib_when_whole() {
        assert_eq!(memory_label(4096), "4 GiB");
        assert_eq!(memory_label(1536), "1536 MiB");
    }
}
