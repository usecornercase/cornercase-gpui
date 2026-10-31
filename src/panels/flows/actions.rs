use gpui::{
    App, AppContext as _, ClickEvent, ClipboardItem, Context, Div, FocusHandle, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, WeakEntity, Window, div,
    prelude::FluentBuilder as _, px,
};
use serde::Deserialize;

use super::dialog::close_dialog;
use super::model::{self, FlowInfo, project_args};
use super::widgets::{icon_action, open_path};
use super::{FlowsPanel, Selected};
use crate::cli;
use crate::dialogs;
use crate::store::{self, Busy, Store, ToastKind, args};
use crate::theme::colours;
use crate::ui::{self, Icon, Tone};

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
    doing: String,
}

impl Question {
    pub(super) fn new(
        title: impl Into<String>,
        message: impl Into<String>,
        button: &str,
        command: Vec<String>,
    ) -> Self {
        let doing = format!("{button}…");
        Self { title: title.into(), message: message.into(), button: button.to_string(), command, doing }
    }

    pub(super) fn doing(mut self, doing: impl Into<String>) -> Self {
        self.doing = doing.into();
        self
    }
}

pub(super) fn job_key(command: &[String]) -> String {
    format!("flow:{}", command.join(" "))
}

pub(super) fn running(command: &[String], cx: &App) -> bool {
    Store::get(cx).read(cx).busy(&job_key(command))
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
    pub(super) fn run_cmd(args: Vec<String>, doing: &str, done: &str, cx: &mut Context<Self>) {
        let done = done.to_string();
        let busy = Busy::new(job_key(&args), doing);
        store::run_busy(cx, busy, args, move |this, result, cx| {
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

    pub(super) fn taken_names(&self) -> Vec<String> {
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
        Self::run_cmd(command, "Saving the default flow…", &done, cx);
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

    fn import_text(text: String, extra: Vec<String>, cx: &mut Context<Self>) {
        let mut command = args(&["flow", "import", "-", "--yes", "--json"]);
        command.extend(extra);
        run_stdin(cx, command, text, move |this, result, cx| {
            match parse_imported(result) {
                Ok(imported) => this.saved(&imported, cx),
                Err(e) => toast(e.to_string(), ToastKind::Error, cx),
            }
            this.refresh(cx);
        });
    }

    fn saved(&mut self, imported: &Imported, cx: &mut App) {
        announce(imported.note.as_deref(), format!("Saved {}", imported.name), cx);
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
        self.export(flow, cx, move |_, text, cx| Self::import_text(text, extra, cx));
    }
}

pub(super) fn flow_buttons(flow: &FlowInfo, hidden: bool, cx: &Context<FlowsPanel>) -> Div {
    let row = div().relative().flex().flex_wrap().items_center().gap(px(8.));
    if hidden {
        return row.child(restore_hidden_button(flow, cx));
    }
    row.child(start_button(flow, cx))
        .child(edit_button(flow, cx))
        .child(ui::icon_text_button("flow-duplicate", Icon::CopyPlus, "Duplicate", Tone::Plain).on_click(with_flow(
            flow,
            cx,
            FlowsPanel::duplicate,
        )))
        .child(div().w(px(1.)).h(px(18.)).mx(px(2.)).bg(colours::border()))
        .child(tidy_buttons(flow, cx))
}

fn edit_button(flow: &FlowInfo, cx: &Context<FlowsPanel>) -> impl IntoElement + use<> {
    let flow = flow.clone();
    ui::icon_text_button("flow-edit", Icon::Pencil, "Edit", Tone::Plain)
        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.open_editor(Some(&flow), window, cx)))
}

fn tidy_buttons(flow: &FlowInfo, cx: &Context<FlowsPanel>) -> Div {
    let (star, hint) = if flow.default {
        (Icon::StarFilled, "Clear the default: the start dialog opens on the first built-in that can start")
    } else {
        (Icon::Star, "Make it the default: the start dialog opens on it")
    };
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .child(icon_action("flow-default", star, hint, false).on_click(with_flow(flow, cx, FlowsPanel::set_default)))
        .child(icon_action("flow-up", Icon::ArrowUp, "Move it up the list", false).on_click(with_flow(
            flow,
            cx,
            |this, f, cx| this.move_flow(f, -1, cx),
        )))
        .child(icon_action("flow-down", Icon::ArrowDown, "Move it down the list", false).on_click(with_flow(
            flow,
            cx,
            |this, f, cx| this.move_flow(f, 1, cx),
        )))
        .child(icon_action("flow-copy", Icon::Copy, "Copy as TOML, to share or commit", false).on_click(with_flow(
            flow,
            cx,
            FlowsPanel::copy_toml,
        )))
        .when_some(flow.path.clone(), |d, path| {
            d.child(
                icon_action("flow-open", Icon::FolderOpen, "Open its file in your editor", false)
                    .on_click(open_path(path)),
            )
        })
        .when(flow.edited, |d| d.child(restore_builtin_button(flow, cx)))
        .child(
            icon_action("flow-delete", Icon::Trash, delete_hint(flow), true)
                .on_click(ask_on_click(delete_question(flow), cx)),
        )
}

fn start_button(flow: &FlowInfo, cx: &Context<FlowsPanel>) -> impl IntoElement + use<> {
    let key = flow.key();
    ui::icon_text_button("flow-start", Icon::Play, "Start…", Tone::Primary)
        .px(px(16.))
        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.open_start(Some(key.clone()), window, cx)))
}

fn restore_hidden_button(flow: &FlowInfo, cx: &Context<FlowsPanel>) -> impl IntoElement + use<> {
    let name = flow.name.clone();
    ui::icon_text_button("flow-unhide", Icon::Undo, "Restore", Tone::Primary).on_click(cx.listener(
        move |_, _: &ClickEvent, _, cx| {
            let command = args(&["flow", "restore", &name]);
            FlowsPanel::run_cmd(command, &format!("Restoring {name}…"), &format!("Restored {name}"), cx);
        },
    ))
}

fn restore_builtin_button(flow: &FlowInfo, cx: &Context<FlowsPanel>) -> impl IntoElement + use<> {
    let question = Question::new(
        "Restore the built-in flow?",
        format!("Your version of {} is deleted, and the built-in one runs again.", flow.name),
        "Restore",
        args(&["flow", "restore", &flow.name]),
    )
    .doing(format!("Restoring {}…", flow.name));
    icon_action("flow-restore", Icon::Undo, "Restore the built-in", false).on_click(ask_on_click(question, cx))
}

fn delete_hint(flow: &FlowInfo) -> &'static str {
    if flow.origin == "built-in" { "Hide this built-in" } else { "Delete" }
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
    Question::new(format!("Delete {name}?"), message, "Delete", command).doing(format!("Deleting {name}…"))
}

pub(super) struct Ask {
    panel: WeakEntity<FlowsPanel>,
    question: Question,
    focus: FocusHandle,
}

impl Ask {
    fn run(&mut self, cx: &mut Context<Self>) {
        let (command, button, doing) =
            (self.question.command.clone(), self.question.button.clone(), self.question.doing.clone());
        let _ = self.panel.update(cx, |_, cx| FlowsPanel::run_cmd(command, &doing, &format!("{button}: done"), cx));
        close_dialog(cx);
    }
}

impl Render for Ask {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, e: &gpui::KeyDownEvent, _, cx| {
                if e.keystroke.key == "enter" {
                    this.run(cx);
                }
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
