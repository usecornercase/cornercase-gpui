use gpui::{
    AppContext as _, ClickEvent, Context, Div, Entity, Focusable as _, IntoElement, ParentElement as _, Render,
    StatefulInteractiveElement as _, Styled as _, WeakEntity, Window, div, px,
};

use super::FlowsPanel;
use super::actions::{Imported, announce, parse_imported, run_stdin};
use super::dialog::{Submit, boxed, buttons, close_dialog, field};
use super::model::project_args;
use super::widgets::error_text;
use crate::dialogs;
use crate::store::{self, Store, args};
use crate::theme::colours;
use crate::ui::{self, TextField};

const CHECK_ISSUE: &str = "usecornercase/cornercase-terminal#466";

pub(super) struct ImportFlow {
    panel: WeakEntity<FlowsPanel>,
    project: Option<u64>,
    source: Entity<TextField>,
    name: Entity<TextField>,
    pasted: Option<String>,
    to_project: bool,
    replace: bool,
    busy: bool,
    error: Option<String>,
}

impl FlowsPanel {
    pub(super) fn open_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let panel = cx.entity().downgrade();
        let project = self.project;
        let view = cx.new(|cx| ImportFlow {
            panel,
            project,
            source: field("A file, https://… or owner/repo on GitHub", cx, window),
            name: field("Its own name (optional)", cx, window),
            pasted: None,
            to_project: false,
            replace: false,
            busy: false,
            error: None,
        });
        let focus = view.read(cx).source.read(cx).focus_handle(cx);
        Store::get(cx).update(cx, |s, cx| s.show_dialog(view.into(), cx));
        window.focus(&focus);
    }
}

pub(super) fn import_args(
    source: &str,
    name: &str,
    to_project: Option<u64>,
    project: bool,
    replace: bool,
) -> Vec<String> {
    let mut command = args(&["flow", "import", source, "--yes", "--json"]);
    if project {
        command.extend(["--to".into(), "project".into()]);
        command.extend(project_args(to_project));
    }
    if !name.trim().is_empty() {
        command.extend(["--name".into(), name.trim().to_string()]);
    }
    if replace {
        command.push("--replace".into());
    }
    command
}

impl Submit for ImportFlow {
    fn submit(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let source = self.source.read(cx).text().trim().to_string();
        let name = self.name.read(cx).text();
        let from = if self.pasted.is_some() { "-".to_string() } else { source };
        if from.is_empty() {
            self.error = Some("Give a file, an address or owner/repo, or paste the flow.".into());
            cx.notify();
            return;
        }
        let command = import_args(&from, &name, self.project, self.to_project, self.replace);
        self.busy = true;
        self.error = None;
        cx.notify();
        match self.pasted.clone() {
            Some(text) => run_stdin(cx, command, text, Self::imported),
            None => store::run(cx, command, Self::imported),
        }
    }
}

impl ImportFlow {
    fn paste(&mut self, cx: &mut Context<Self>) {
        match cx.read_from_clipboard().and_then(|item| item.text()) {
            Some(text) if !text.trim().is_empty() => {
                self.pasted = Some(text);
                self.error = None;
            }
            _ => self.error = Some("The clipboard holds no text.".into()),
        }
        cx.notify();
    }

    fn imported(&mut self, result: anyhow::Result<String>, cx: &mut Context<Self>) {
        self.busy = false;
        match parse_imported(result) {
            Ok(imported) => self.finish(&imported, cx),
            Err(e) => self.error = Some(e.to_string()),
        }
        cx.notify();
    }

    fn finish(&self, imported: &Imported, cx: &mut Context<Self>) {
        announce(imported.note.as_deref(), format!("Imported {}", imported.name), cx);
        close_dialog(cx);
        let _ = self.panel.update(cx, |panel, cx| {
            panel.select_imported(imported);
            panel.refresh(cx);
        });
    }

    fn source_control(&self, window: &Window, cx: &Context<Self>) -> Div {
        let row = div().flex().items_center().gap(px(8.));
        match &self.pasted {
            Some(text) => row
                .child(ui::dim(format!("{} lines from the clipboard", text.lines().count())).flex_1())
                .child(ui::button("import-unpaste", "Clear").on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.pasted = None;
                    cx.notify();
                }))),
            None => row.child(div().flex_1().child(boxed(&self.source, window, cx))).child(
                ui::button("import-paste", "Paste").on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.paste(cx))),
            ),
        }
    }

    fn place(&self, cx: &Context<Self>) -> Div {
        let choice = |id: &'static str, label: &'static str, to_project: bool| {
            ui::segment(id, label, self.to_project == to_project)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.to_project = to_project;
                    cx.notify();
                }))
                .into_any_element()
        };
        ui::segments(vec![choice("import-yours", "Yours", false), choice("import-project", "This project's", true)])
    }

    fn form(&self, window: &Window, cx: &Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(ui::field("Flow", self.source_control(window, cx)))
            .child(ui::field("Into", self.place(cx)))
            .child(ui::field("Name", boxed(&self.name, window, cx)))
            .child(ui::checkbox("import-replace", self.replace, "Replace the flow that has its name").on_click(
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.replace = !this.replace;
                    cx.notify();
                }),
            ))
            .child(div().text_size(px(12.)).text_color(colours::waiting()).child(
                "Every agent of a flow runs without permission prompts: read a flow's prompts, variables and \
                 arguments before importing it, above all one from the web.",
            ))
            .child(ui::dim(format!("Showing them here first needs {CHECK_ISSUE}.")))
            .children(self.error.clone().map(error_text))
    }
}

impl Render for ImportFlow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let label = if self.busy { "Importing…" } else { "Import" };
        dialogs::frame("Import a flow", self.form(window, cx), buttons(("import-cancel", "import-run"), label, cx))
            .w(px(520.))
    }
}

#[cfg(test)]
mod tests;
