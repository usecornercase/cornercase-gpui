use gpui::{
    App, AppContext as _, ClickEvent, ClipboardItem, Context, Div, FocusHandle, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, WeakEntity, Window, div,
    prelude::FluentBuilder as _, px,
};
use serde::Deserialize;

use super::dialog::close_dialog;
use super::model::{self, FlowInfo, project_args};
use super::{FlowsPanel, Selected};
use crate::cli;
use crate::dialogs;
use crate::store::{self, Store, ToastKind, args};
use crate::theme::colours;
use crate::ui;

pub(super) const SAVE_ISSUE: &str = "usecornercase/cornercase-terminal#465";

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Exported {
    text: String,
    note: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(super) struct Imported {
    pub name: String,
    pub origin: String,
    pub path: Option<String>,
    pub note: Option<String>,
}

pub(super) fn parse_imported(result: anyhow::Result<String>) -> anyhow::Result<Imported> {
    Ok(serde_json::from_str::<Imported>(&result?)?)
}

pub(super) fn run_stdin<T: 'static>(
    cx: &mut Context<T>,
    args: Vec<String>,
    stdin: String,
    then: impl FnOnce(&mut T, anyhow::Result<String>, &mut Context<T>) + 'static,
) {
    let task = cx.background_spawn(async move { cli::run_with_stdin(&args, Some(&stdin)) });
    super::deliver(cx, task, then);
}

fn toast(text: impl Into<String>, kind: ToastKind, cx: &mut App) {
    let text = text.into();
    Store::get(cx).update(cx, |s, cx| s.toast(text, kind, cx));
}

fn note(note: Option<&str>, cx: &mut App) {
    if let Some(note) = note {
        toast(note, ToastKind::Info, cx);
    }
}

pub(super) fn announce(text: Option<&str>, done: String, cx: &mut App) {
    note(text, cx);
    toast(done, ToastKind::Done, cx);
}

#[derive(Debug, Clone)]
pub(super) struct Question {
    title: String,
    message: String,
    button: String,
    command: Vec<String>,
}

impl Question {
    pub(super) fn new(
        title: impl Into<String>,
        message: impl Into<String>,
        button: &str,
        command: Vec<String>,
    ) -> Self {
        Self { title: title.into(), message: message.into(), button: button.to_string(), command }
    }
}

pub(super) fn ask_on_click(
    question: Question,
    cx: &Context<FlowsPanel>,
) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    cx.listener(move |_, _: &ClickEvent, window, cx| FlowsPanel::ask(question.clone(), window, cx))
}

fn with_flow(
    flow: &FlowInfo,
    cx: &Context<FlowsPanel>,
    action: fn(&mut FlowsPanel, &FlowInfo, &mut Context<FlowsPanel>),
) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    let flow = flow.clone();
    cx.listener(move |this, _: &ClickEvent, _, cx| action(this, &flow, cx))
}

impl FlowsPanel {
    pub(super) fn run_cmd(args: Vec<String>, done: &str, cx: &mut Context<Self>) {
        let done = done.to_string();
        store::run(cx, args, move |this, result, cx| {
            match result {
                Ok(_) => toast(done, ToastKind::Done, cx),
                Err(e) => toast(e.to_string(), ToastKind::Error, cx),
            }
            this.refresh(cx);
        });
    }

    pub(super) fn ask(question: Question, window: &mut Window, cx: &mut Context<Self>) {
        let panel = cx.entity().downgrade();
        let view = cx.new(|cx| Ask { panel, question, focus: cx.focus_handle() });
        let focus = view.read(cx).focus.clone();
        Store::get(cx).update(cx, |s, cx| s.show_dialog(view.into(), cx));
        window.focus(&focus);
    }

    fn taken_names(&self) -> Vec<String> {
        let Some(listing) = &self.listing else { return Vec::new() };
        listing.flows.iter().chain(&listing.hidden).map(|f| f.name.clone()).chain(listing.builtins.clone()).collect()
    }

    fn set_default(&mut self, flow: &FlowInfo, cx: &mut Context<Self>) {
        let mut command = args(&["flow", "default"]);
        command.push(if flow.default { "--clear".into() } else { flow.name.clone() });
        command.extend(project_args(self.project));
        let done = if flow.default {
            "No default flow: the start dialog opens on the first built-in that can start".to_string()
        } else {
            format!("The start dialog opens on {} now", flow.name)
        };
        Self::run_cmd(command, &done, cx);
    }

    fn move_flow(&mut self, flow: &FlowInfo, delta: isize, cx: &mut Context<Self>) {
        let Some(listing) = &self.listing else { return };
        let names: Vec<String> = listing.flows.iter().map(|f| f.name.clone()).collect();
        let Some(index) = listing.flows.iter().position(|f| f.key() == flow.key()) else { return };
        let Some(order) = model::moved(&names, index, delta) else { return };
        let mut command = args(&["flow", "order"]);
        command.extend(order);
        command.extend(project_args(self.project));
        store::run(cx, command, |this, result, cx| {
            if let Err(e) = result {
                toast(e.to_string(), ToastKind::Error, cx);
            }
            this.refresh(cx);
        });
    }

    fn export(
        &mut self,
        flow: &FlowInfo,
        cx: &mut Context<Self>,
        then: impl FnOnce(&mut Self, String, &mut Context<Self>) + 'static,
    ) {
        let mut command = args(&["flow", "export", &flow.name, "--json"]);
        command.extend(project_args(self.project));
        store::run(cx, command, move |this, result, cx| {
            match result.and_then(|out| Ok(serde_json::from_str::<Exported>(&out)?)) {
                Ok(exported) => {
                    note(exported.note.as_deref(), cx);
                    then(this, exported.text, cx);
                }
                Err(e) => toast(e.to_string(), ToastKind::Error, cx),
            }
        });
    }

    fn copy_toml(&mut self, flow: &FlowInfo, cx: &mut Context<Self>) {
        let name = flow.name.clone();
        self.export(flow, cx, move |_, text, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            toast(format!("Copied {name} as TOML: `cornercase flow import -` or Import… adds it"), ToastKind::Done, cx);
        });
    }

    fn import_text(text: String, extra: Vec<String>, open: bool, cx: &mut Context<Self>) {
        let mut command = args(&["flow", "import", "-", "--yes", "--json"]);
        command.extend(extra);
        run_stdin(cx, command, text, move |this, result, cx| {
            match parse_imported(result) {
                Ok(imported) => this.saved(&imported, open, cx),
                Err(e) => toast(e.to_string(), ToastKind::Error, cx),
            }
            this.refresh(cx);
        });
    }

    fn saved(&mut self, imported: &Imported, open: bool, cx: &mut Context<Self>) {
        announce(imported.note.as_deref(), format!("Saved {}", imported.name), cx);
        if open && let Some(path) = &imported.path {
            cx.open_with_system(std::path::Path::new(path));
        }
        self.select_imported(imported);
    }

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
            extra.extend(project_args(self.project));
        }
        self.export(flow, cx, move |_, text, cx| Self::import_text(text, extra, false, cx));
    }

    fn edit(&mut self, flow: &FlowInfo, cx: &mut Context<Self>) {
        match &flow.path {
            Some(path) => cx.open_with_system(std::path::Path::new(path)),
            None => self.export(flow, cx, |_, text, cx| Self::import_text(text, vec!["--replace".into()], true, cx)),
        }
    }
}

pub(super) fn flow_buttons(flow: &FlowInfo, hidden: bool, cx: &Context<FlowsPanel>) -> Div {
    let row = div().flex().flex_wrap().items_center().gap(px(6.)).pt(px(10.));
    if hidden {
        return row.child(restore_hidden_button(flow, cx));
    }
    let edit = if flow.path.is_some() { "Edit file" } else { "Customise" };
    let row =
        row.child(start_button(flow, cx))
            .child(
                ui::button("flow-default", if flow.default { "Clear default" } else { "Set default" })
                    .on_click(with_flow(flow, cx, FlowsPanel::set_default)),
            )
            .child(ui::button("flow-up", "↑").on_click(with_flow(flow, cx, |this, f, cx| this.move_flow(f, -1, cx))))
            .child(ui::button("flow-down", "↓").on_click(with_flow(flow, cx, |this, f, cx| this.move_flow(f, 1, cx))))
            .child(ui::button("flow-copy", "Copy as TOML").on_click(with_flow(flow, cx, FlowsPanel::copy_toml)))
            .child(ui::button("flow-duplicate", "Duplicate").on_click(with_flow(flow, cx, FlowsPanel::duplicate)))
            .child(ui::button("flow-edit", edit).on_click(with_flow(flow, cx, FlowsPanel::edit)))
            .when(flow.edited, |d| d.child(restore_builtin_button(flow, cx)))
            .child(ui::danger_button("flow-delete", "Delete").on_click(ask_on_click(delete_question(flow), cx)));
    div().flex().flex_col().child(row).child(
        ui::dim(format!("{edit} opens the flow's TOML in your editor; editing it in a form needs {SAVE_ISSUE}."))
            .pt(px(6.)),
    )
}

fn start_button(flow: &FlowInfo, cx: &Context<FlowsPanel>) -> impl IntoElement + use<> {
    let key = flow.key();
    ui::primary_button("flow-start", "Start…")
        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.open_start(Some(key.clone()), window, cx)))
}

fn restore_hidden_button(flow: &FlowInfo, cx: &Context<FlowsPanel>) -> impl IntoElement + use<> {
    let name = flow.name.clone();
    ui::primary_button("flow-unhide", "Restore").on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
        FlowsPanel::run_cmd(args(&["flow", "restore", &name]), &format!("Restored {name}"), cx);
    }))
}

fn restore_builtin_button(flow: &FlowInfo, cx: &Context<FlowsPanel>) -> impl IntoElement + use<> {
    let question = Question::new(
        "Restore the built-in flow?",
        format!("Your version of {} is deleted, and the built-in one runs again.", flow.name),
        "Restore",
        args(&["flow", "restore", &flow.name]),
    );
    ui::button("flow-restore", "Restore built-in").on_click(ask_on_click(question, cx))
}

fn delete_question(flow: &FlowInfo) -> Question {
    let name = &flow.name;
    let (message, command) = match (flow.origin.as_str(), flow.edited, &flow.path) {
        ("built-in", _, _) => {
            (format!("{name} is hidden, and keeps its name. Restore brings it back."), args(&["flow", "hide", name]))
        }
        (_, true, _) => (
            format!(
                "Your version of {name} is deleted and the built-in one is hidden. Restore brings the built-in back."
            ),
            args(&["flow", "hide", name]),
        ),
        (_, false, Some(path)) => (format!("{path} is deleted from disk."), args(&["flow", "delete", "--path", path])),
        (_, false, None) => (format!("{name} is deleted."), args(&["flow", "delete", name])),
    };
    Question::new(format!("Delete {name}?"), message, "Delete", command)
}

pub(super) struct Ask {
    panel: WeakEntity<FlowsPanel>,
    question: Question,
    focus: FocusHandle,
}

impl Ask {
    fn run(&mut self, cx: &mut Context<Self>) {
        let (command, button) = (self.question.command.clone(), self.question.button.clone());
        let _ = self.panel.update(cx, |_, cx| FlowsPanel::run_cmd(command, &format!("{button}: done"), cx));
        close_dialog(cx);
    }
}

impl Render for Ask {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, e: &gpui::KeyDownEvent, _, cx| match e.keystroke.key.as_str() {
                "enter" => this.run(cx),
                "escape" => close_dialog(cx),
                _ => {}
            }))
            .child(dialogs::frame(
                self.question.title.clone(),
                div().text_color(colours::muted()).child(self.question.message.clone()),
                div()
                    .flex()
                    .gap(px(8.))
                    .child(ui::button("ask-cancel", "Cancel").on_click(|_: &ClickEvent, _, cx| close_dialog(cx)))
                    .child(
                        ui::danger_button("ask-confirm", self.question.button.clone())
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.run(cx))),
                    ),
            ))
    }
}
