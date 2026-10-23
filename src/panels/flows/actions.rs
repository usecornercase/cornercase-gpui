//! What the flow page's buttons do, through `cornercase flow`: set the default, order, copy as
//! TOML (share), duplicate, edit the file (a built-in is customised first: exported and imported
//! as yours), delete, hide and restore; and the question asked before what cannot be undone.

use gpui::{
    App, AppContext as _, ClickEvent, Context, Div, FocusHandle, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, WeakEntity, Window, div, px,
};
use serde::Deserialize;

use super::model::{self, FlowInfo};
use super::{FlowsPanel, Selected};
use crate::cli;
use crate::dialogs;
use crate::store::{self, Store, ToastKind, args};
use crate::theme::colours;
use crate::ui;

/// The form editor waits on this: no command saves a flow from a draft.
pub(super) const SAVE_ISSUE: &str = "usecornercase/cornercase-terminal#465";

/// `flow export --json`.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Exported {
    text: String,
    note: Option<String>,
}

/// `flow import --json`.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(super) struct Imported {
    pub name: String,
    pub origin: String,
    pub path: Option<String>,
    pub note: Option<String>,
}

/// Runs a command with `stdin` off the main thread, then hands its output to `then`.
pub(super) fn run_stdin<T: 'static>(
    cx: &mut Context<T>,
    args: Vec<String>,
    stdin: String,
    then: impl FnOnce(&mut T, anyhow::Result<String>, &mut Context<T>) + 'static,
) {
    let task = cx.background_spawn(async move { cli::run_with_stdin(&args, Some(&stdin)) });
    cx.spawn(async move |this, cx| {
        let result = task.await;
        let _ = this.update(cx, |this, cx| then(this, result, cx));
    })
    .detach();
}

fn toast(text: impl Into<String>, kind: ToastKind, cx: &mut App) {
    let text = text.into();
    Store::get(cx).update(cx, |s, cx| s.toast(text, kind, cx));
}

impl FlowsPanel {
    /// Runs a management command, says how it went and reads the flows again.
    pub(super) fn run_cmd(&mut self, args: Vec<String>, done: &str, cx: &mut Context<Self>) {
        let done = done.to_string();
        store::run(cx, args, move |this, result, cx| {
            match result {
                Ok(_) => toast(done, ToastKind::Done, cx),
                Err(e) => toast(e.to_string(), ToastKind::Error, cx),
            }
            this.refresh(cx);
        });
    }

    /// Asks before running a command that cannot be undone.
    pub(super) fn ask(
        &mut self,
        title: &str,
        message: impl Into<String>,
        button: &str,
        command: Vec<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let panel = cx.entity().downgrade();
        let view = cx.new(|cx| Ask {
            panel,
            title: title.to_string(),
            message: message.into(),
            button: button.to_string(),
            command,
            focus: cx.focus_handle(),
        });
        let focus = view.read(cx).focus.clone();
        Store::get(cx).update(cx, |s, cx| s.show_dialog(view.into(), cx));
        window.focus(&focus);
    }

    fn project_args(&self) -> Vec<String> {
        self.project.map(|p| vec!["--project".to_string(), p.to_string()]).unwrap_or_default()
    }

    fn taken_names(&self) -> Vec<String> {
        let Some(listing) = &self.listing else { return Vec::new() };
        listing.flows.iter().chain(&listing.hidden).map(|f| f.name.clone()).chain(listing.builtins.clone()).collect()
    }

    // -----------------------------------------------------------------------------------------

    fn set_default(&mut self, flow: &FlowInfo, cx: &mut Context<Self>) {
        let mut a = args(&["flow", "default"]);
        if flow.default {
            a.push("--clear".into());
        } else {
            a.push(flow.name.clone());
        }
        a.extend(self.project_args());
        let done = if flow.default {
            "No default flow: the start dialog opens on the first built-in that can start".to_string()
        } else {
            format!("The start dialog opens on {} now", flow.name)
        };
        self.run_cmd(a, &done, cx);
    }

    fn move_flow(&mut self, flow: &FlowInfo, delta: isize, cx: &mut Context<Self>) {
        let Some(listing) = &self.listing else { return };
        let names: Vec<String> = listing.flows.iter().map(|f| f.name.clone()).collect();
        let Some(index) = listing.flows.iter().position(|f| f.key() == flow.key()) else { return };
        let Some(order) = model::moved(&names, index, delta) else { return };
        let mut a = args(&["flow", "order"]);
        a.extend(order);
        a.extend(self.project_args());
        store::run(cx, a, |this, result, cx| {
            if let Err(e) = result {
                toast(e.to_string(), ToastKind::Error, cx);
            }
            this.refresh(cx);
        });
    }

    /// Exports the flow, then hands its TOML to `then`.
    fn export(
        &mut self,
        flow: &FlowInfo,
        cx: &mut Context<Self>,
        then: impl FnOnce(&mut Self, String, &mut Context<Self>) + 'static,
    ) {
        let mut a = args(&["flow", "export", &flow.name, "--json"]);
        a.extend(self.project_args());
        store::run(cx, a, move |this, result, cx| {
            match result.and_then(|out| Ok(serde_json::from_str::<Exported>(&out)?)) {
                Ok(exported) => {
                    if let Some(note) = exported.note {
                        toast(note, ToastKind::Info, cx);
                    }
                    then(this, exported.text, cx);
                }
                Err(e) => toast(e.to_string(), ToastKind::Error, cx),
            }
        });
    }

    /// Share: the flow's TOML on the clipboard, to paste to a teammate or commit.
    fn copy_toml(&mut self, flow: &FlowInfo, cx: &mut Context<Self>) {
        let name = flow.name.clone();
        self.export(flow, cx, move |_, text, cx| {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
            toast(format!("Copied {name} as TOML: `cornercase flow import -` or Import… adds it"), ToastKind::Done, cx);
        });
    }

    /// Imports exported text as a new flow (`--name`) or over yours (`--replace`).
    fn import_text(&mut self, text: String, extra: Vec<String>, open: bool, cx: &mut Context<Self>) {
        let mut a = args(&["flow", "import", "-", "--yes", "--json"]);
        a.extend(extra);
        run_stdin(cx, a, text, move |this, result, cx| {
            match result.and_then(|out| Ok(serde_json::from_str::<Imported>(&out)?)) {
                Ok(imported) => {
                    if let Some(note) = &imported.note {
                        toast(note.clone(), ToastKind::Info, cx);
                    }
                    toast(format!("Saved {}", imported.name), ToastKind::Done, cx);
                    if open && let Some(path) = &imported.path {
                        cx.open_with_system(std::path::Path::new(path));
                    }
                    this.select_imported(&imported);
                }
                Err(e) => toast(e.to_string(), ToastKind::Error, cx),
            }
            this.refresh(cx);
        });
    }

    /// Selects a flow just saved, once the list has it.
    pub(super) fn select_imported(&mut self, imported: &Imported) {
        let key = FlowInfo {
            name: imported.name.clone(),
            origin: imported.origin.clone(),
            path: imported.path.clone(),
            ..FlowInfo::default()
        }
        .key();
        self.selected = Some(Selected::Flow(key));
    }

    fn duplicate(&mut self, flow: &FlowInfo, cx: &mut Context<Self>) {
        let taken = self.taken_names();
        let name = model::unique_name(&flow.name, &taken.iter().map(String::as_str).collect::<Vec<_>>());
        let mut extra = vec!["--name".to_string(), name];
        if flow.origin == "project" {
            extra.extend(["--to".into(), "project".into()]);
            extra.extend(self.project_args());
        }
        self.export(flow, cx, move |this, text, cx| this.import_text(text, extra, false, cx));
    }

    /// Opens the flow's file in the user's editor; a built-in becomes yours first, as editing
    /// one in Settings › Flows does.
    fn edit(&mut self, flow: &FlowInfo, cx: &mut Context<Self>) {
        match &flow.path {
            Some(path) => cx.open_with_system(std::path::Path::new(path)),
            None => self.export(flow, cx, |this, text, cx| this.import_text(text, vec!["--replace".into()], true, cx)),
        }
    }

    // -----------------------------------------------------------------------------------------

    pub(super) fn flow_buttons(&self, flow: &FlowInfo, hidden: bool, cx: &mut Context<Self>) -> Div {
        let row = div().flex().flex_wrap().items_center().gap(px(6.)).pt(px(10.));
        if hidden {
            let name = flow.name.clone();
            return row.child(ui::primary_button("flow-unhide", "Restore").on_click(cx.listener(
                move |this, _: &ClickEvent, _, cx| {
                    this.run_cmd(args(&["flow", "restore", &name]), &format!("Restored {name}"), cx)
                },
            )));
        }
        let f = flow.clone();
        let mut row =
            row.child(ui::primary_button("flow-start", "Start…").on_click(
                cx.listener(move |this, _: &ClickEvent, window, cx| this.open_start(Some(f.key()), window, cx)),
            ));
        let f = flow.clone();
        row = row.child(
            ui::button("flow-default", if flow.default { "Clear default" } else { "Set default" })
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_default(&f, cx))),
        );
        let (up, down) = (flow.clone(), flow.clone());
        row = row
            .child(
                ui::button("flow-up", "↑")
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.move_flow(&up, -1, cx))),
            )
            .child(
                ui::button("flow-down", "↓")
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.move_flow(&down, 1, cx))),
            );
        let f = flow.clone();
        row = row.child(
            ui::button("flow-copy", "Copy as TOML")
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.copy_toml(&f, cx))),
        );
        let f = flow.clone();
        row = row.child(
            ui::button("flow-duplicate", "Duplicate")
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.duplicate(&f, cx))),
        );
        let f = flow.clone();
        let edit = if flow.path.is_some() { "Edit file" } else { "Customise" };
        row = row.child(
            ui::button("flow-edit", edit).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.edit(&f, cx))),
        );
        let name = flow.name.clone();
        if flow.edited {
            let n = name.clone();
            row = row.child(ui::button("flow-restore", "Restore built-in").on_click(cx.listener(
                move |this, _: &ClickEvent, window, cx| {
                    this.ask(
                        "Restore the built-in flow?",
                        format!("Your version of {n} is deleted, and the built-in one runs again."),
                        "Restore",
                        args(&["flow", "restore", &n]),
                        window,
                        cx,
                    )
                },
            )));
        }
        let (message, command) = match (flow.origin.as_str(), flow.edited, &flow.path) {
            ("built-in", _, _) => (
                format!("{name} is hidden, and keeps its name. Restore brings it back."),
                args(&["flow", "hide", &name]),
            ),
            (_, true, _) => (
                format!(
                    "Your version of {name} is deleted and the built-in one is hidden. Restore brings the built-in back."
                ),
                args(&["flow", "hide", &name]),
            ),
            (_, false, Some(path)) => {
                (format!("{path} is deleted from disk."), args(&["flow", "delete", "--path", path]))
            }
            (_, false, None) => (format!("{name} is deleted."), args(&["flow", "delete", &name])),
        };
        row = row.child(ui::danger_button("flow-delete", "Delete").on_click(cx.listener(
            move |this, _: &ClickEvent, window, cx| {
                this.ask(&format!("Delete {name}?"), message.clone(), "Delete", command.clone(), window, cx)
            },
        )));
        div().flex().flex_col().child(row).child(
            ui::dim(format!("{edit} opens the flow's TOML in your editor; editing it in a form needs {SAVE_ISSUE}."))
                .pt(px(6.)),
        )
    }
}

// ---------------------------------------------------------------------------------------------

/// The question before a delete, a hide, a stop or a forget; it reads the flows again after.
pub(super) struct Ask {
    panel: WeakEntity<FlowsPanel>,
    title: String,
    message: String,
    button: String,
    command: Vec<String>,
    focus: FocusHandle,
}

impl Ask {
    fn run(&mut self, cx: &mut Context<Self>) {
        let (command, button) = (self.command.clone(), self.button.clone());
        let _ = self.panel.update(cx, |panel, cx| panel.run_cmd(command, &format!("{button}: done"), cx));
        close(cx);
    }
}

fn close(cx: &mut App) {
    Store::get(cx).update(cx, |s, cx| s.close_dialog(cx));
}

impl Render for Ask {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, e: &gpui::KeyDownEvent, _, cx| match e.keystroke.key.as_str() {
                "enter" => this.run(cx),
                "escape" => close(cx),
                _ => {}
            }))
            .child(dialogs::frame(
                self.title.clone(),
                div().text_color(colours::muted()).child(self.message.clone()),
                div()
                    .flex()
                    .gap(px(8.))
                    .child(ui::button("ask-cancel", "Cancel").on_click(|_: &ClickEvent, _, cx| close(cx)))
                    .child(
                        ui::danger_button("ask-confirm", self.button.clone())
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.run(cx))),
                    ),
            ))
    }
}
