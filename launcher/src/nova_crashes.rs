//! The Crash Analyzer page: Analyze Latest Log, the saved history (search, certainty and category
//! filters, newest or oldest first) and one crash's details with its exports. The findings come
//! from `crash_analyzer`, the saved entries from `nova_crash_history`.
use super::*;
use crate::servers_page::Field;
use crate::glass_ui::{self as ui, white};
use crate::nova_crash_history::{self as history, Certainty, Entry, Order};
use horizon_theme::glass::{ink, radius};

/// How many rows the list draws at once; the search narrows the rest.
const SHOWN: usize = 100;

#[derive(Default)]
pub(super) struct CrashPage {
    entries: Vec<Entry>,
    loaded: bool,
    /// History lines that could not be read.
    skipped: usize,
    certainty: Certainty,
    order: Order,
    category: Option<String>,
    /// The entry whose details are open.
    selected: Option<String>,
    confirm_clear: bool,
    confirm_delete: Option<String>,
    note: Option<Result<String, String>>,
}

impl CrashPage {
    /// The saved analyses, read from disk the first time.
    pub(super) fn snapshot(&mut self) -> Vec<Entry> {
        if !self.loaded {
            self.reload();
        }
        self.entries.clone()
    }

    pub(super) fn reload(&mut self) {
        let (entries, skipped) = history::load();
        self.entries = entries;
        self.skipped = skipped;
        self.loaded = true;
    }
}

fn card() -> Div {
    crate::glass::plate(radius::PANEL).flex().flex_col().p(px(16.)).gap(px(8.))
}

impl Launcher {
    /// Analyses the newest log or crash report, saves it in the history and opens it.
    pub(super) fn analyze_latest_log(&mut self) {
        let (minecraft, profile) = self.nova_profile_facts();
        let result = match crate::nova_pages::newest_log() {
            None => Err(tr("No log or crash report found yet. Start the game once first.").to_string()),
            Some(path) => crate::nova_pages::read_tail(&path, 512 * 1024).and_then(|text| {
                let analysis = horizon_launcher::crash_analyzer::analyze(&text);
                history::record(&path, &analysis, &minecraft, &profile)?;
                Ok(path)
            }),
        };
        self.crashes.reload();
        self.crashes.note = Some(match result {
            Ok(path) => {
                // Open the newest entry, which is the one just made (or the same finding's
                // earlier entry when it was saved a moment ago).
                self.crashes.selected = self
                    .crashes
                    .entries
                    .iter()
                    .rev()
                    .find(|e| e.source_path == path.to_string_lossy())
                    .map(|e| e.id.clone());
                Ok(trf("Analysed {file}.", &[("file", &path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())]))
            }
            Err(e) => Err(e),
        });
    }

    /// The version and name of the active profile, saved with each analysis.
    pub(super) fn nova_profile_facts(&self) -> (String, String) {
        match self.selected_instance() {
            Some(p) => {
                let version = match &p.java {
                    Some(java) => java.minecraft.clone(),
                    None => self.version_of(p).cloned().unwrap_or_default(),
                };
                (version, p.name.clone())
            }
            None => (String::new(), String::new()),
        }
    }

    pub(super) fn crashes_page(&mut self, cx: &mut Context<Self>) -> Div {
        if !self.crashes.loaded {
            self.crashes.reload();
        }
        let head = ui::page_head(
            tr("Crash Analyzer"),
            tr("Finds the likely cause of a crash from your own logs. Nothing leaves your computer. A diagnosis is a lead, not a guarantee."),
        );
        let note = self.crashes.note.clone().map(|n| {
            let (text, colour) = match n {
                Ok(t) => (t, ui::live()),
                Err(e) => (e, ui::danger()),
            };
            div()
                .mb(px(12.))
                .child(ui::text(13., FontWeight::MEDIUM, 1.).text_color(colour).child(text))
        });
        let body = match self.crashes.selected.clone() {
            Some(id) if self.crashes.entries.iter().any(|e| e.id == id) => self.crash_details(&id, cx),
            _ => self.crash_list(cx),
        };
        div().flex().flex_col().gap(px(12.)).w_full().child(head).children(note).child(body)
    }

    fn crash_list(&mut self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let look = ui::look();

        // Actions.
        let mut actions = div().flex().flex_wrap().items_center().gap(px(8.));
        actions = actions
            .child(button("crash-analyze", "Analyze Latest Log", Kind::Primary, &t).on_click(
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.analyze_latest_log();
                    cx.notify();
                }),
            ))
            .child(button("crash-open-folder", "Open Crash Reports Folder", Kind::Secondary, &t).on_click(
                |_, _, cx| {
                    let dir = backend::crash_reports_dir();
                    let _ = std::fs::create_dir_all(&dir);
                    crate::glass_toast::open(cx, &dir);
                },
            ));
        if self.crashes.confirm_clear {
            actions = actions
                .child(ui::card_note(
                    trf("Delete all {count} saved analyses? Your log files are not touched.", &[("count", &self.crashes.entries.len().to_string())]),
                ))
                .child(button("crash-clear-no", "Cancel", Kind::Secondary, &t).on_click(cx.listener(
                    |this, _: &ClickEvent, _, cx| {
                        this.crashes.confirm_clear = false;
                        cx.notify();
                    },
                )))
                .child(button("crash-clear-yes", "Clear History", Kind::Primary, &t).on_click(cx.listener(
                    |this, _: &ClickEvent, _, cx| {
                        this.crashes.confirm_clear = false;
                        this.crashes.note = Some(
                            history::clear_in(&history::path())
                                .map(|_| tr("History cleared.").to_string()),
                        );
                        this.crashes.selected = None;
                        this.crashes.reload();
                        cx.notify();
                    },
                )));
        } else if !self.crashes.entries.is_empty() {
            actions = actions.child(button("crash-clear", "Clear History", Kind::Ghost, &t).on_click(
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.crashes.confirm_clear = true;
                    cx.notify();
                }),
            ));
        }

        // Search, filters and order.
        let query = self.servers.value(Field::NovaCrashSearch).to_string();
        let field = self.field(Field::NovaCrashSearch, tr("Search crashes..."), 240., cx);
        let certainty = segments(
            "crash-certainty",
            &[(Certainty::Any, "All"), (Certainty::Confirmed, "Confirmed"), (Certainty::Possible, "Possible")],
            self.crashes.certainty,
            &t,
            {
                let this = cx.entity().downgrade();
                move |choice: Certainty, _, cx| {
                    let _ = this.update(cx, |this, cx| {
                        this.crashes.certainty = choice;
                        cx.notify();
                    });
                }
            },
        );
        let order = segments(
            "crash-order",
            &[(Order::Newest, "Newest first"), (Order::Oldest, "Oldest first")],
            self.crashes.order,
            &t,
            {
                let this = cx.entity().downgrade();
                move |choice: Order, _, cx| {
                    let _ = this.update(cx, |this, cx| {
                        this.crashes.order = choice;
                        cx.notify();
                    });
                }
            },
        );
        let categories = history::categories(&self.crashes.entries);
        let current = self.crashes.category.clone();
        let cycle = button(
            "crash-category",
            if let Some(c) = &current { format!("Category: {c}") } else { "Category: All".to_string() },
            Kind::Secondary,
            &t,
        )
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            let list = history::categories(&this.crashes.entries);
            this.crashes.category = match &this.crashes.category {
                None => list.first().cloned(),
                Some(c) => list.iter().position(|x| x == c).and_then(|i| list.get(i + 1)).cloned(),
            };
            cx.notify();
        }));
        let _ = categories;
        let filters = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(8.))
            .child(field)
            .child(certainty)
            .child(cycle)
            .child(order);

        // The list.
        let shown = history::select(
            &self.crashes.entries,
            &query,
            self.crashes.certainty,
            self.crashes.category.as_deref(),
            self.crashes.order,
        );
        let total_matching = shown.len();
        let mut list = card();
        if self.crashes.entries.is_empty() {
            list = list.child(ui::card_note(
                tr("No analyses yet. Press Analyze Latest Log, or crash the game: the launcher analyses it and saves the result here.").to_string(),
            ));
        } else if shown.is_empty() {
            list = list.child(ui::card_note(tr("Nothing matches the search and filters.").to_string()));
        }
        for (n, e) in shown.iter().take(SHOWN).enumerate() {
            let id = e.id.clone();
            let colour = if e.is_unknown() {
                white(ink::MUTED)
            } else if e.confirmed {
                ui::live()
            } else {
                ui::warn()
            };
            list = list.child(
                ui::row(format!("crash-row-{n}"), false, 10., &t)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(ui::text(14., FontWeight::SEMIBOLD, ink::FULL).child(tr(&e.category).to_string()))
                                    .child(ui::text(12., FontWeight::MEDIUM, 1.).text_color(colour).child(tr(e.certainty_text()).to_string())),
                            )
                            .child(ui::card_note(format!(
                                "{} · {}{}",
                                e.local_time(),
                                e.source,
                                if e.profile.is_empty() { String::new() } else { format!(" · {}", e.profile) },
                            )))
                            .child(ui::text(13., FontWeight::NORMAL, ink::NOTE).truncate().child(e.summary.clone())),
                    )
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.crashes.selected = Some(id.clone());
                        this.crashes.confirm_delete = None;
                        cx.notify();
                    })),
            );
        }
        if total_matching > SHOWN {
            list = list.child(ui::card_note(trf(
                "Showing the first {shown} of {total}. Narrow the search to see the rest.",
                &[("shown", &SHOWN.to_string()), ("total", &total_matching.to_string())],
            )));
        }
        if self.crashes.skipped > 0 {
            list = list.child(ui::card_note(trf(
                "{count} line(s) of the history file could not be read and were skipped.",
                &[("count", &self.crashes.skipped.to_string())],
            )));
        }
        let _ = look;
        div().flex().flex_col().gap(px(12.)).child(actions).child(filters).child(list)
    }

    fn crash_details(&mut self, id: &str, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let Some(e) = self.crashes.entries.iter().find(|e| e.id == id).cloned() else {
            return div();
        };
        let back = button("crash-back", "Back to history", Kind::Secondary, &t).on_click(cx.listener(
            |this, _: &ClickEvent, _, cx| {
                this.crashes.selected = None;
                cx.notify();
            },
        ));

        let colour = if e.is_unknown() {
            white(ink::MUTED)
        } else if e.confirmed {
            ui::live()
        } else {
            ui::warn()
        };
        let fact = |label: &str, value: String| {
            ui::setting_row(tr(label).to_string(), say_plain(value), None)
        };
        let mut facts = card()
            .child(fact("Date and time", e.local_time()))
            .child(fact("Log file", e.source.clone()))
            .child(fact(
                "Minecraft version",
                if e.minecraft.is_empty() { tr("unknown").to_string() } else { e.minecraft.clone() },
            ))
            .child(fact(
                "Profile when analysed",
                if e.profile.is_empty() { tr("unknown").to_string() } else { e.profile.clone() },
            ))
            .child(fact("Category", tr(&e.category).to_string()));

        let verdict = card()
            .child(ui::text(16., FontWeight::SEMIBOLD, ink::FULL).child(tr(&e.category).to_string()))
            .child(ui::text(13., FontWeight::MEDIUM, 1.).text_color(colour).child(format!(
                "{}{}",
                tr(e.certainty_text()),
                if e.is_unknown() { String::new() } else { format!(" · {}% {}", e.confidence, tr("confidence")) },
            )))
            .child(ui::card_note(e.summary.clone()))
            .child(ui::card_note(
                tr("This is an automatic diagnosis from the log's text. It is a lead, not a guarantee.").to_string(),
            ));

        if !e.suspect_mods.is_empty() {
            facts = facts.child(fact(
                "Mods named in the log",
                e.suspect_mods.join(", "),
            ));
        }

        let mut excerpt = card().child(ui::card_heading(tr("Relevant log excerpt")));
        if e.excerpt.is_empty() {
            excerpt = excerpt.child(ui::card_note(tr("No matching lines: nothing in the log pointed at a cause.").to_string()));
        }
        for line in &e.excerpt {
            excerpt = excerpt.child(
                div()
                    .font_family(theme::MONO)
                    .text_size(px(12.))
                    .text_color(white(ink::NOTE))
                    .child(line.clone()),
            );
        }

        let mut steps = card().child(ui::card_heading(tr("What to try, in order")));
        for (n, step) in e.suggestions.iter().enumerate() {
            steps = steps.child(ui::card_note(format!("{}. {step}", n + 1)));
        }

        // Actions: copy, export, open the original, delete.
        let confirming = self.crashes.confirm_delete.as_deref() == Some(id);
        let (copy_entry, txt_entry, json_entry, open_path) =
            (e.clone(), e.clone(), e.clone(), e.source_path.clone());
        let delete_id = e.id.clone();
        let mut actions = div().flex().flex_wrap().gap(px(8.));
        actions = actions
            .child(button("crash-copy", "Copy Report", Kind::Secondary, &t).on_click(cx.listener(
                move |this, _: &ClickEvent, _, cx| {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(history::report_txt(&copy_entry)));
                    this.crashes.note = Some(Ok(tr("Report copied.").to_string()));
                    cx.notify();
                },
            )))
            .child(button("crash-export-txt", "Export TXT", Kind::Secondary, &t).on_click(cx.listener(
                move |this, _: &ClickEvent, _, cx| {
                    this.crashes.note = Some(
                        history::export(&txt_entry, "txt", &history::report_txt(&txt_entry))
                            .map(|p| trf("Saved {path}", &[("path", &p.display().to_string())])),
                    );
                    cx.notify();
                },
            )))
            .child(button("crash-export-json", "Export JSON", Kind::Secondary, &t).on_click(cx.listener(
                move |this, _: &ClickEvent, _, cx| {
                    this.crashes.note = Some(
                        history::report_json(&json_entry)
                            .and_then(|json| history::export(&json_entry, "json", &json))
                            .map(|p| trf("Saved {path}", &[("path", &p.display().to_string())])),
                    );
                    cx.notify();
                },
            )))
            .child(button("crash-open-exports", "Open Exports Folder", Kind::Ghost, &t).on_click(|_, _, cx| {
                let dir = history::export_dir();
                let _ = std::fs::create_dir_all(&dir);
                crate::glass_toast::open(cx, &dir);
            }))
            .child(button("crash-open-log", "Open Original Log", Kind::Secondary, &t).on_click(cx.listener(
                move |this, _: &ClickEvent, _, cx| {
                    let path = std::path::PathBuf::from(&open_path);
                    if path.is_file() {
                        crate::glass_toast::open(cx, &path);
                    } else {
                        this.crashes.note = Some(Err(trf(
                            "The original file is gone: {path}",
                            &[("path", &open_path)],
                        )));
                    }
                    cx.notify();
                },
            )));
        actions = if confirming {
            actions
                .child(ui::card_note(tr("Delete this analysis? The log file is not touched.").to_string()))
                .child(button("crash-del-no", "Cancel", Kind::Secondary, &t).on_click(cx.listener(
                    |this, _: &ClickEvent, _, cx| {
                        this.crashes.confirm_delete = None;
                        cx.notify();
                    },
                )))
                .child(button("crash-del-yes", "Delete", Kind::Primary, &t).on_click(cx.listener(
                    move |this, _: &ClickEvent, _, cx| {
                        this.crashes.note = Some(match history::delete_from(&history::path(), &delete_id) {
                            Ok(_) => Ok(tr("Analysis deleted.").to_string()),
                            Err(e) => Err(e),
                        });
                        this.crashes.confirm_delete = None;
                        this.crashes.selected = None;
                        this.crashes.reload();
                        cx.notify();
                    },
                )))
        } else {
            let ask = id.to_string();
            actions.child(button("crash-delete", "Delete Entry", Kind::Ghost, &t).on_click(cx.listener(
                move |this, _: &ClickEvent, _, cx| {
                    this.crashes.confirm_delete = Some(ask.clone());
                    cx.notify();
                },
            )))
        };

        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(div().flex().child(back))
            .child(verdict)
            .child(facts)
            .child(excerpt)
            .child(steps)
            .child(actions)
    }
}

fn say_plain(s: String) -> Option<AnyElement> {
    Some(div().child(s).into_any_element())
}
