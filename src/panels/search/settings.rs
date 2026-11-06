use gpui::{AnyElement, App, ClickEvent, Context, Div, IntoElement as _, SharedString, div, prelude::*, px, relative};

use super::names::find;
use super::{DEBOUNCE, SearchPanel, highlighted, labelled_chip};
use crate::cli;
use crate::panels::settings::{FoundSetting, parse_found, sheet_of};
use crate::store::{self, Store, args};
use crate::theme::colours;
use crate::ui::{self, Icon};

const LOOKING: &str = "Looking in the settings…";
const STALE: f32 = 0.55;

fn ask_args(query: &str) -> Vec<String> {
    args(&["config", "search", "--json", "--", query])
}

fn settings_count(n: usize) -> String {
    format!("{n} setting{}", if n == 1 { "" } else { "s" })
}

fn note_of(found: &FoundSetting) -> String {
    if found.is_tab() {
        return if sheet_of(&found.page).is_some() { "the sheet" } else { "the page" }.into();
    }
    if found.note.starts_with("enter ") { String::new() } else { found.note.clone() }
}

#[derive(Default)]
pub(super) struct Asked {
    generation: u64,
    answers: String,
    asking: bool,
    hits: Vec<FoundSetting>,
    error: Option<String>,
    enter: bool,
}

impl Asked {
    pub(super) fn hits(&self) -> &[FoundSetting] {
        &self.hits
    }

    pub(super) fn asking(&self) -> bool {
        self.asking
    }

    pub(super) fn failed(&self) -> bool {
        self.error.is_some()
    }

    pub(super) fn forget_enter(&mut self) {
        self.enter = false;
    }

    pub(super) fn waiting_for(&self, query: &str) -> bool {
        self.error.is_none() && self.answers != query
    }

    pub(super) fn status(&self, listing: String, query: &str, names: usize) -> String {
        if let Some(error) = &self.error {
            return format!("{listing} · {error}");
        }
        if self.waiting_for(query) {
            return if names == 0 { LOOKING.into() } else { listing };
        }
        match (names, self.hits.len()) {
            (_, 0) => listing,
            (0, n) => settings_count(n),
            (_, n) => format!("{listing} · {}", settings_count(n)),
        }
    }
}

impl SearchPanel {
    pub(super) fn schedule_settings(&mut self, cx: &mut Context<Self>) {
        let query = self.query(cx);
        let settings = &mut self.settings;
        settings.generation += 1;
        settings.asking = false;
        settings.enter = false;
        settings.error = None;
        if query.is_empty() {
            *settings = Asked { generation: settings.generation, ..Asked::default() };
            return;
        }
        if query == settings.answers {
            return;
        }
        let generation = settings.generation;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(DEBOUNCE).await;
            let _ = this.update(cx, |this, cx| this.ask_settings(query, generation, cx));
        })
        .detach();
    }

    fn ask_settings(&mut self, query: String, generation: u64, cx: &mut Context<Self>) {
        if self.settings.generation != generation {
            return;
        }
        self.settings.asking = true;
        cx.notify();
        store::run(cx, ask_args(&query), move |this, answer, cx| {
            this.settings_answered(query, generation, answer.and_then(|text| parse_found(&text)), cx);
        });
    }

    fn settings_answered(
        &mut self,
        query: String,
        generation: u64,
        answer: anyhow::Result<Vec<FoundSetting>>,
        cx: &mut Context<Self>,
    ) {
        if self.settings.generation != generation {
            return;
        }
        self.settings.asking = false;
        match answer {
            Ok(hits) => {
                self.settings.answers = query;
                self.settings.hits = hits;
                if std::mem::take(&mut self.settings.enter) {
                    self.submit(cx);
                }
            }
            Err(e) => {
                self.settings.answers.clear();
                self.settings.hits.clear();
                self.settings.error = Some(cli::reason(&e));
            }
        }
        cx.notify();
    }

    pub(super) fn submit_setting(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.settings.waiting_for(&self.query(cx)) {
            self.settings.enter = true;
            return;
        }
        if let Some(found) = self.settings.hits.get(index) {
            open_setting(found.clone(), cx);
        }
    }
}

fn open_setting(found: FoundSetting, cx: &mut App) {
    Store::get(cx).update(cx, |s, cx| match sheet_of(&found.page) {
        Some(kind) => s.open_sheet(kind, cx),
        None => s.land_on(found, cx),
    });
}

pub(super) struct SettingRow {
    pub(super) index: usize,
    pub(super) selected: bool,
    pub(super) stale: bool,
}

fn side(colour: gpui::Rgba) -> Div {
    div().flex().text_size(px(12.)).text_color(colour)
}

impl SettingRow {
    pub(super) fn render(&self, found: FoundSetting, query: &str, cx: &mut Context<SearchPanel>) -> AnyElement {
        let title = highlighted(found.title.clone(), find(&found.title, query).into_iter().collect());
        let (ink, note_ink) =
            if found.hidden { (colours::muted(), colours::muted()) } else { (colours::text(), colours::faint()) };
        let (value, note) = (found.shown.trim().to_string(), note_of(&found));
        ui::row(SharedString::from(format!("setting-{}", self.index)), self.selected)
            .when(self.stale, |d| d.opacity(STALE))
            .child(labelled_chip("setting", Icon::Settings, note_ink.into()))
            .child(div().flex().flex_none().max_w(relative(0.5)).text_color(ink).child(title))
            .child(side(colours::muted()).flex_1().min_w(px(0.)).child(ui::one_line(value)))
            .child(side(note_ink).flex_none().max_w(relative(0.4)).child(ui::one_line(note)))
            .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| open_setting(found.clone(), cx)))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests;
