use std::path::PathBuf;

use anyhow::Context as _;
use gpui::{AppContext as _, Context, Div, PathPromptOptions, Stateful, div, prelude::*, px};
use serde::Deserialize;

use super::{SettingsPanel, widgets};
use crate::cli;
use crate::store::{Store, ToastKind, args};
use crate::theme::colours;
use crate::ui::{self, Icon};

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(super) struct Preview {
    pub(super) sections: Vec<Section>,
    pub(super) notes: Vec<Note>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(super) struct Section {
    pub(super) id: String,
    pub(super) changes: Vec<Change>,
    pub(super) notes: Vec<Note>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(super) struct Change {
    pub(super) label: String,
    pub(super) from: String,
    pub(super) to: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(super) struct Note {
    pub(super) kind: String,
    pub(super) text: String,
}

impl Note {
    pub(super) fn warns(&self) -> bool {
        matches!(self.kind.as_str(), "risk" | "secret")
    }
}

impl Preview {
    pub(super) fn changes(&self) -> usize {
        self.sections.iter().map(|s| s.changes.len()).sum()
    }

    pub(super) fn risky(&self) -> bool {
        self.notes.iter().chain(self.sections.iter().flat_map(|s| &s.notes)).any(Note::warns)
    }

    pub(super) fn shown(&self) -> impl Iterator<Item = &Section> {
        self.sections.iter().filter(|s| !s.changes.is_empty() || !s.notes.is_empty())
    }
}

pub(super) fn parse_preview(text: &str) -> anyhow::Result<Preview> {
    Ok(serde_json::from_str(text)?)
}

pub(super) fn import_args(preview: bool) -> Vec<String> {
    args(&["config", "import", "-", if preview { "--preview" } else { "--yes" }, "--json"])
}

pub(super) struct Import {
    name: String,
    text: String,
    preview: Option<Preview>,
    error: Option<String>,
    busy: bool,
}

impl Import {
    fn new(name: String, text: String) -> Self {
        Self { name, text, preview: None, error: None, busy: true }
    }
}

fn note_line(note: &Note) -> Div {
    if note.warns() {
        return ui::error_line(note.text.clone());
    }
    widgets::hint(note.text.clone())
}

fn change_row(change: &Change) -> Div {
    div()
        .flex()
        .items_start()
        .gap(px(10.))
        .py(px(3.))
        .text_size(px(12.))
        .child(div().w(px(190.)).flex_none().text_color(colours::soft()).child(widgets::capitalised(&change.label)))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(8.))
                .child(div().min_w_0().text_color(colours::faint()).child(change.from.clone()))
                .child(ui::icon(Icon::ArrowRight, 12.).text_color(colours::faint()))
                .child(div().min_w_0().text_color(colours::text()).child(change.to.clone())),
        )
}

fn section_block(section: &Section) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .pt(px(10.))
        .pb(px(6.))
        .border_t_1()
        .border_color(colours::border())
        .child(widgets::caption(widgets::capitalised(&section.id)).pb(px(4.)))
        .children(section.changes.iter().map(change_row))
        .children(section.notes.iter().map(note_line))
}

fn preview_body(preview: &Preview) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(4.))
        .pb(px(10.))
        .children(preview.notes.iter().map(note_line))
        .children(preview.shown().map(section_block))
}

fn summary(import: &Import) -> String {
    match import.preview.as_ref().map(Preview::changes) {
        None => String::new(),
        Some(0) => "Nothing would change: this setup matches yours.".to_string(),
        Some(1) => "1 setting would change".to_string(),
        Some(n) => format!("{n} settings would change"),
    }
}

fn read_setup(path: &PathBuf) -> anyhow::Result<(String, String)> {
    let text = std::fs::read_to_string(path).with_context(|| format!("could not read {}", path.display()))?;
    let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
    Ok((name, text))
}

impl SettingsPanel {
    pub(super) fn choose_import(cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Preview".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(path) = paths.into_iter().next()
            {
                let _ = this.update(cx, |this, cx| this.preview_import(&path, cx));
            }
        })
        .detach();
    }

    fn preview_import(&mut self, path: &PathBuf, cx: &mut Context<Self>) {
        match read_setup(path) {
            Ok((name, text)) => {
                self.import = Some(Import::new(name, text.clone()));
                Self::ask_import(true, text, cx);
            }
            Err(e) => Store::get(cx).update(cx, |s, cx| s.toast(e.to_string(), ToastKind::Error, cx)),
        }
        cx.notify();
    }

    fn ask_import(preview: bool, text: String, cx: &mut Context<Self>) {
        let task = cx.background_spawn(async move { cli::run_with_stdin(&import_args(preview), Some(&text)) });
        crate::panels::flows::deliver(cx, task, move |this: &mut Self, result, cx| {
            if preview {
                this.previewed(result.and_then(|out| parse_preview(&out)));
            } else {
                this.imported(result, cx);
            }
            cx.notify();
        });
    }

    fn previewed(&mut self, result: anyhow::Result<Preview>) {
        let Some(import) = self.import.as_mut() else { return };
        import.busy = false;
        match result {
            Ok(preview) => import.preview = Some(preview),
            Err(e) => import.error = Some(cli::reason(&e)),
        }
    }

    fn imported(&mut self, result: anyhow::Result<String>, cx: &mut Context<Self>) {
        match result {
            Ok(_) => {
                let changes = self.import.take().and_then(|i| i.preview).map_or(0, |p| p.changes());
                self.load(cx);
                let text = format!("Imported {changes} {}", if changes == 1 { "setting" } else { "settings" });
                Store::get(cx).update(cx, |s, cx| s.toast(text, ToastKind::Done, cx));
            }
            Err(e) => {
                if let Some(import) = self.import.as_mut() {
                    import.busy = false;
                    import.error = Some(cli::reason(&e));
                }
            }
        }
    }

    fn confirm_import(&mut self, cx: &mut Context<Self>) {
        let Some(import) = self.import.as_mut().filter(|i| !i.busy) else { return };
        import.busy = true;
        import.error = None;
        let text = import.text.clone();
        Self::ask_import(false, text, cx);
        cx.notify();
    }

    pub(super) fn import_preview(import: &Import, cx: &mut Context<Self>) -> Div {
        let body = match (&import.preview, &import.error) {
            (_, Some(e)) => widgets::under(ui::error_line(e.clone())),
            (Some(preview), None) => preview_body(preview),
            (None, None) => div().pb(px(10.)).child(ui::busy("import-reading", "Reading what would change…")),
        };
        let header = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .pt(px(12.))
            .pb(px(8.))
            .child(ui::icon(Icon::File, 14.).text_color(colours::accent()))
            .child(div().flex_none().font_weight(gpui::FontWeight::SEMIBOLD).child(import.name.clone()))
            .child(div().flex_1().min_w_0().text_size(px(12.)).text_color(colours::muted()).child(summary(import)));
        div()
            .flex()
            .flex_col()
            .border_t_1()
            .border_color(colours::border())
            .child(header)
            .child(body)
            .child(import_buttons(import, cx))
    }
}

fn import_buttons(import: &Import, cx: &mut Context<SettingsPanel>) -> Div {
    let cancel = ui::button("import-cancel", "Cancel").on_click(cx.listener(|this, _, _, cx| {
        this.import = None;
        cx.notify();
    }));
    let preview = import.preview.as_ref().filter(|p| p.changes() > 0);
    let go = match (preview, import.busy) {
        (Some(_), true) => Some(ui::busy_button("import-go", "Importing…", ui::Tone::Primary)),
        (Some(p), false) => Some(import_button(p, cx)),
        (None, _) => None,
    };
    div().flex().justify_end().gap(px(8.)).pb(px(14.)).child(cancel).children(go)
}

fn import_button(preview: &Preview, cx: &mut Context<SettingsPanel>) -> Stateful<Div> {
    let button = if preview.risky() { ui::danger_button } else { ui::primary_button };
    let label = format!("Import {}", if preview.changes() == 1 { "this setting" } else { "these settings" });
    button("import-go", label).on_click(cx.listener(|this, _, _, cx| this.confirm_import(cx)))
}

#[cfg(test)]
mod tests;
