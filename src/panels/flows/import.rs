//! Adding a flow someone shared (`cornercase flow import`): from a file, an https:// address, a
//! GitHub `owner/repo`, or the TOML on the clipboard; into yours or the project's, under its name
//! or another, replacing a flow of that name when asked. The CLI asks before importing a flow that
//! sets variables or comes from the web, which a window cannot answer, so this dialog says what
//! the question would and passes `--yes`; showing the flow's own risks first waits on #466.

use gpui::{
    App, AppContext as _, ClickEvent, Context, Entity, Focusable as _, IntoElement, ParentElement as _, Render,
    SharedString, StatefulInteractiveElement as _, Styled as _, WeakEntity, Window, div, prelude::FluentBuilder as _,
    px,
};

use super::FlowsPanel;
use super::actions::{Imported, run_stdin};
use crate::dialogs;
use crate::store::{self, Store, ToastKind, args};
use crate::theme::colours;
use crate::ui::{self, TextField, TextFieldEvent};

const CHECK_ISSUE: &str = "usecornercase/cornercase-terminal#466";

pub(super) struct ImportFlow {
    panel: WeakEntity<FlowsPanel>,
    project: Option<u64>,
    source: Entity<TextField>,
    name: Entity<TextField>,
    /// Text taken from the clipboard, imported through standard input instead of `source`.
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

fn field(placeholder: &str, cx: &mut Context<ImportFlow>, window: &mut Window) -> Entity<TextField> {
    let field = cx.new(|cx| TextField::new(placeholder.to_string(), cx));
    cx.subscribe_in(&field, window, |this, _, event, _, cx| match event {
        TextFieldEvent::Submit => this.submit(cx),
        TextFieldEvent::Cancel => close(cx),
        TextFieldEvent::Changed => cx.notify(),
    })
    .detach();
    field
}

fn close(cx: &mut App) {
    Store::get(cx).update(cx, |s, cx| s.close_dialog(cx));
}

/// `flow import` arguments after the source.
pub(super) fn import_args(
    source: &str,
    name: &str,
    to_project: Option<u64>,
    project: bool,
    replace: bool,
) -> Vec<String> {
    let mut a = args(&["flow", "import", source, "--yes", "--json"]);
    if project {
        a.extend(["--to".into(), "project".into()]);
        if let Some(p) = to_project {
            a.extend(["--project".into(), p.to_string()]);
        }
    }
    if !name.trim().is_empty() {
        a.extend(["--name".into(), name.trim().to_string()]);
    }
    if replace {
        a.push("--replace".into());
    }
    a
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
        let a = import_args(&from, &name, self.project, self.to_project, self.replace);
        self.busy = true;
        self.error = None;
        cx.notify();
        let then = |this: &mut Self, result: anyhow::Result<String>, cx: &mut Context<Self>| {
            this.busy = false;
            match result.and_then(|out| Ok(serde_json::from_str::<Imported>(&out)?)) {
                Ok(imported) => {
                    Store::get(cx).update(cx, |s, cx| {
                        if let Some(note) = &imported.note {
                            s.toast(note.clone(), ToastKind::Info, cx);
                        }
                        s.toast(format!("Imported {}", imported.name), ToastKind::Done, cx);
                        s.close_dialog(cx);
                    });
                    let _ = this.panel.update(cx, |panel, cx| {
                        panel.select_imported(&imported);
                        panel.refresh(cx);
                    });
                }
                Err(e) => this.error = Some(e.to_string()),
            }
            cx.notify();
        };
        match self.pasted.clone() {
            Some(text) => run_stdin(cx, a, text, then),
            None => store::run(cx, a, then),
        }
    }
}

impl Render for ImportFlow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let boxed = |field: &Entity<TextField>, window: &Window, cx: &App| {
            ui::input_box(field.clone(), field.read(cx).focus_handle(cx).is_focused(window))
        };
        let source = match &self.pasted {
            Some(text) => div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(ui::dim(format!("{} lines from the clipboard", text.lines().count())).flex_1())
                .child(ui::button("import-unpaste", "Clear").on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.pasted = None;
                    cx.notify();
                }))),
            None => div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(div().flex_1().child(boxed(&self.source, window, cx)))
                .child(
                    ui::button("import-paste", "Paste")
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.paste(cx))),
                ),
        };
        let place = ui::segments(vec![
            ui::segment("import-yours", "Yours", !self.to_project)
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.to_project = false;
                    cx.notify();
                }))
                .into_any_element(),
            ui::segment("import-project", "This project's", self.to_project)
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.to_project = true;
                    cx.notify();
                }))
                .into_any_element(),
        ]);
        let body = div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(ui::field("Flow", source))
            .child(ui::field("Into", place))
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
            .when_some(self.error.clone(), |d, e| {
                d.child(div().text_size(px(12.)).text_color(colours::failed()).child(SharedString::from(e)))
            });
        dialogs::frame(
            "Import a flow",
            body,
            div()
                .flex()
                .gap(px(8.))
                .child(ui::button("import-cancel", "Cancel").on_click(|_: &ClickEvent, _, cx| close(cx)))
                .child(
                    ui::primary_button("import-run", if self.busy { "Importing…" } else { "Import" })
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.submit(cx))),
                ),
        )
        .w(px(520.))
    }
}

#[cfg(test)]
mod tests {
    use super::import_args;

    #[test]
    fn builds_import_arguments() {
        assert_eq!(import_args("q.toml", " ", None, false, false), ["flow", "import", "q.toml", "--yes", "--json"]);
        assert_eq!(
            import_args("-", "Mine", Some(3), true, true),
            [
                "flow",
                "import",
                "-",
                "--yes",
                "--json",
                "--to",
                "project",
                "--project",
                "3",
                "--name",
                "Mine",
                "--replace"
            ]
        );
    }
}
