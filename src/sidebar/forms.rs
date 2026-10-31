mod agents;
mod checked;
mod style;

use gpui::{
    AnyView, App, AppContext as _, ClickEvent, Context, Div, Entity, FocusHandle, Focusable, InteractiveElement as _,
    IntoElement, KeyDownEvent, ParentElement as _, Render, SharedString, Stateful, StatefulInteractiveElement as _,
    Styled as _, Window, div, px,
};
use serde_json::Value;

use crate::dialogs::frame;
use crate::store::{self, Busy, Store, args};
use crate::theme::colours;
use crate::ui::{self, TextField, TextFieldEvent};

pub(super) use agents::close_agents;
pub(crate) use checked::close_agent;
pub(super) use checked::remove_worktree;
pub(super) use style::group_style;

fn close(cx: &mut App) {
    Store::get(cx).update(cx, Store::close_dialog);
}

fn show<T: Render + Focusable>(view: Entity<T>, window: &mut Window, cx: &mut App) {
    let focus = view.read(cx).focus_handle(cx);
    Store::get(cx).update(cx, |s, cx| s.show_dialog(AnyView::from(view), cx));
    window.focus(&focus);
}

pub(super) fn palette_colour(index: u8) -> gpui::Rgba {
    let [r, g, b] = crate::theme::palette(index);
    gpui::rgb((u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b))
}

enum Check {
    Loading,
    Ready(Value),
    Failed(String),
}

impl Check {
    fn from(result: anyhow::Result<Value>) -> Self {
        match result {
            Ok(v) => Check::Ready(v),
            Err(e) => Check::Failed(e.to_string().lines().next().unwrap_or_default().to_string()),
        }
    }
}

trait Checked: 'static {
    fn check(&mut self) -> &mut Check;
}

fn load_check<T: Checked>(command: Vec<String>, cx: &mut Context<T>) {
    store::run_json(cx, command, |this: &mut T, result, cx| {
        *this.check() = Check::from(result);
        cx.notify();
    });
}

fn line(text: impl Into<SharedString>) -> Div {
    div().text_color(colours::muted()).child(text.into())
}

fn warning(text: impl Into<SharedString>) -> Div {
    div().text_color(colours::failed()).child(text.into())
}

fn bullet(text: &str, colour: gpui::Rgba) -> Div {
    div().pl(px(12.)).text_color(colour).child(format!("• {text}"))
}

fn buttons(go: impl IntoIterator<Item = Stateful<Div>>) -> Div {
    div()
        .flex()
        .gap(px(8.))
        .child(ui::button("cancel", "Cancel").on_click(|_: &ClickEvent, _, cx| close(cx)))
        .children(go)
}

fn keyed<T: 'static>(focus: &FocusHandle, enter: fn(&mut T, &mut Context<T>), cx: &mut Context<T>) -> Div {
    div().track_focus(focus).on_key_down(cx.listener(move |this, e: &KeyDownEvent, _, cx| {
        if e.keystroke.key == "enter" {
            enter(this, cx);
        }
    }))
}

pub(super) struct NewGroup {
    name: Entity<TextField>,
    project: Option<u64>,
}

pub(super) fn new_group(project: Option<u64>, window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx| {
        let name = cx.new(|cx| TextField::new("work", cx));
        cx.subscribe_in(&name, window, |this: &mut NewGroup, _, event, _, cx| match event {
            TextFieldEvent::Submit => this.submit(cx),
            TextFieldEvent::Cancel => close(cx),
            TextFieldEvent::Changed => cx.notify(),
        })
        .detach();
        NewGroup { name, project }
    });
    show(view, window, cx);
}

fn group_created(project: Option<u64>, result: anyhow::Result<Value>, cx: &mut App) {
    let store = Store::get(cx);
    let (text, kind) = match result.map(|v| v.get("group").and_then(Value::as_u64)) {
        Ok(Some(group)) => {
            if let Some(project) = project {
                store.read(cx).cmd(args(&["move", "--project", &project.to_string(), "--into", &group.to_string()]));
            }
            store.update(cx, Store::close_dialog);
            return;
        }
        Ok(None) => ("cornercase group new printed no id".to_string(), store::ToastKind::Error),
        Err(e) => (e.to_string(), store::ToastKind::Error),
    };
    store.update(cx, |s, cx| s.toast(text, kind, cx));
}

const CREATING_GROUP: &str = "group-new";

impl NewGroup {
    fn submit(&mut self, cx: &mut Context<Self>) {
        let name = self.name.read(cx).text().trim().to_string();
        if name.is_empty() {
            return;
        }
        if Store::get(cx).read(cx).busy(CREATING_GROUP) {
            return;
        }
        let project = self.project;
        let busy = Busy::new(CREATING_GROUP, format!("Creating the group {name}…"));
        store::run_busy(cx, busy, args(&["group", "new", &name, "--json"]), move |_, result, cx| {
            group_created(project, result.and_then(|out| serde_json::from_str(&out).map_err(Into::into)), cx);
        });
    }
}

impl Focusable for NewGroup {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.name.read(cx).focus_handle(cx)
    }
}

impl Render for NewGroup {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focused = self.name.read(cx).focus_handle(cx).is_focused(window);
        frame(
            "New group",
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(ui::field("Name", ui::input_box(self.name.clone(), focused)))
                .child(ui::dim("It takes the next icon and colour; change them from its menu.")),
            buttons([if Store::get(cx).read(cx).busy(CREATING_GROUP) {
                ui::busy_button("create", "Creating…", ui::Tone::Primary)
            } else {
                ui::primary_button("create", "Create")
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.submit(cx)))
            }]),
        )
    }
}

pub(super) struct DeleteGroup {
    group: u64,
    name: String,
    projects: usize,
    focus: FocusHandle,
}

pub(super) fn delete_group(group: u64, name: String, projects: usize, window: &mut Window, cx: &mut App) {
    if projects == 0 {
        crate::dialogs::confirm(
            crate::dialogs::Question::new(
                format!("Delete the group {name}?"),
                "It holds no projects.",
                "Delete group",
                args(&["group", "delete", &group.to_string()]),
            ),
            window,
            cx,
        );
        return;
    }
    let view = cx.new(|cx| DeleteGroup { group, name, projects, focus: cx.focus_handle() });
    show(view, window, cx);
}

impl DeleteGroup {
    fn run(&self, close_projects: bool, cx: &mut App) {
        let mut a = args(&["group", "delete", &self.group.to_string()]);
        if !close_projects {
            Store::get(cx).update(cx, |s, cx| {
                s.cmd(a);
                s.close_dialog(cx);
            });
            return;
        }
        a.push("--close-projects".into());
        let busy = Busy::new(format!("delete-group:{}", self.group), format!("Closing the projects of {}…", self.name));
        Store::get(cx).update(cx, |s, cx| {
            s.run_aside(busy, a, cx);
            s.close_dialog(cx);
        });
    }
}

impl Focusable for DeleteGroup {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for DeleteGroup {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let n = self.projects;
        let plural = if n == 1 { "project" } else { "projects" };
        keyed(&self.focus, |this: &mut Self, cx| this.run(false, cx), cx).child(frame(
            format!("Delete the group {}?", self.name),
            div().flex().flex_col().gap(px(6.)).child(line(format!(
                "It holds {n} {plural}. Keep them outside any group, or close them too: closing stops every \
                 shell in them, agents included. Folders and worktrees stay on disk."
            ))),
            buttons([
                ui::danger_button("close-projects", format!("Close {plural}"))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.run(true, cx))),
                ui::primary_button("keep", format!("Keep {plural}"))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.run(false, cx))),
            ]),
        ))
    }
}
