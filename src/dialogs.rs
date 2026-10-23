//! Small dialogs over the window: making a workspace, starting an agent, confirming something
//! that cannot be undone. Open one with `Store::show_dialog`; any panel can.

use gpui::{
    App, AppContext as _, ClickEvent, Context, Entity, FocusHandle, Focusable, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, StatefulInteractiveElement as _, SharedString, Styled as _, Window, div, px,
};

use crate::store::{Store, args};
use crate::theme::colours;
use crate::ui::{self, TextField, TextFieldEvent};

/// The frame every dialog is drawn in.
pub fn frame(title: impl Into<SharedString>, body: impl IntoElement, buttons: impl IntoElement) -> gpui::Div {
    div()
        .w(px(460.))
        .p(px(18.))
        .flex()
        .flex_col()
        .gap(px(14.))
        .rounded(px(10.))
        .bg(colours::panel())
        .border_1()
        .border_color(colours::border())
        .shadow_lg()
        .child(ui::title(title))
        .child(body)
        .child(div().flex().justify_end().gap(px(8.)).child(buttons))
}

fn close(cx: &mut App) {
    Store::get(cx).update(cx, |s, cx| s.close_dialog(cx));
}

/// Makes a field, subscribes the dialog to its enter and escape, and returns it.
fn text_field<T: 'static>(
    placeholder: &str,
    cx: &mut Context<T>,
    on_submit: impl Fn(&mut T, &mut Window, &mut Context<T>) + 'static,
    window: &mut Window,
) -> Entity<TextField> {
    let field = cx.new(|cx| TextField::new(placeholder.to_string(), cx));
    cx.subscribe_in(&field, window, move |this, _, event, window, cx| match event {
        TextFieldEvent::Submit => on_submit(this, window, cx),
        TextFieldEvent::Cancel => close(cx),
        TextFieldEvent::Changed => cx.notify(),
    })
    .detach();
    field
}

// ---------------------------------------------------------------------------------------------

pub struct NewWorkspace {
    project: u64,
    project_name: String,
    name: Entity<TextField>,
    worktree: bool,
}

/// `cornercase new-workspace NAME --project ID [--worktree] --focus`, in a project (the shown one
/// without one).
pub fn new_workspace(project: Option<u64>, window: &mut Window, cx: &mut App) {
    let store = Store::get(cx);
    let s = store.read(cx);
    let Some(session) = &s.session else { return };
    let project = project.or(session.shown().project);
    let Some(p) = session.projects.iter().find(|p| Some(p.id) == project) else { return };
    let (id, name) = (p.id, p.name.clone());
    let view = cx.new(|cx| {
        let name_field = text_field("feat/my-branch", cx, |this: &mut NewWorkspace, _, cx| this.submit(cx), window);
        NewWorkspace { project: id, project_name: name, name: name_field, worktree: true }
    });
    let focus = view.read(cx).name.read(cx).focus_handle(cx);
    store.update(cx, |s, cx| s.show_dialog(view.into(), cx));
    window.focus(&focus);
}

impl NewWorkspace {
    fn submit(&mut self, cx: &mut Context<Self>) {
        let name = self.name.read(cx).text().trim().to_string();
        if name.is_empty() {
            return;
        }
        let mut a = args(&["new-workspace", &name, "--project", &self.project.to_string(), "--focus"]);
        if self.worktree {
            a.push("--worktree".into());
        }
        Store::get(cx).update(cx, |s, cx| {
            s.act(a);
            s.close_dialog(cx);
        });
    }
}

impl Render for NewWorkspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focused = self.name.read(cx).focus_handle(cx).is_focused(window);
        frame(
            format!("New workspace in {}", self.project_name),
            div()
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(ui::field("Branch or name", ui::input_box(self.name.clone(), focused)))
                .child(
                    ui::checkbox("worktree", self.worktree, "Check the branch out in a git worktree of its own").on_click(
                        cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.worktree = !this.worktree;
                            cx.notify();
                        }),
                    ),
                ),
            div()
                .flex()
                .gap(px(8.))
                .child(ui::button("cancel", "Cancel").on_click(|_: &ClickEvent, _, cx| close(cx)))
                .child(
                    ui::primary_button("create", "Create")
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.submit(cx))),
                ),
        )
    }
}

// ---------------------------------------------------------------------------------------------

pub struct StartAgent {
    agent: Entity<TextField>,
    prompt: Entity<TextField>,
    worktree: Entity<TextField>,
}

/// `cornercase start [AGENT] --focus [--prompt TEXT] [--worktree BRANCH]` in the shown workspace.
pub fn start_agent(window: &mut Window, cx: &mut App) {
    let store = Store::get(cx);
    if store.read(cx).view_id.is_none() {
        return;
    }
    let view = cx.new(|cx| {
        let submit = |this: &mut StartAgent, _: &mut Window, cx: &mut Context<StartAgent>| this.submit(cx);
        let agent = text_field("the default agent (settings → agents)", cx, submit, window);
        let prompt = text_field("what it should do (optional)", cx, submit, window);
        let worktree = text_field("a branch, to work in its own worktree (optional)", cx, submit, window);
        StartAgent { agent, prompt, worktree }
    });
    let focus = view.read(cx).prompt.read(cx).focus_handle(cx);
    store.update(cx, |s, cx| s.show_dialog(view.into(), cx));
    window.focus(&focus);
}

impl StartAgent {
    fn submit(&mut self, cx: &mut Context<Self>) {
        let agent = self.agent.read(cx).text().trim().to_string();
        let prompt = self.prompt.read(cx).text().trim().to_string();
        let worktree = self.worktree.read(cx).text().trim().to_string();
        let mut a = args(&["start"]);
        if !agent.is_empty() {
            a.push(agent);
        }
        a.push("--focus".into());
        if !prompt.is_empty() {
            a.push("--prompt".into());
            a.push(prompt);
        }
        if !worktree.is_empty() {
            a.push("--worktree".into());
            a.push(worktree);
        }
        Store::get(cx).update(cx, |s, cx| {
            s.act(a);
            s.close_dialog(cx);
        });
    }
}

impl Render for StartAgent {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let boxed = |field: &Entity<TextField>, window: &Window, cx: &App| {
            ui::input_box(field.clone(), field.read(cx).focus_handle(cx).is_focused(window))
        };
        frame(
            "Start an agent in a new tab",
            div()
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(ui::field("Agent", boxed(&self.agent, window, cx)))
                .child(ui::field("Prompt", boxed(&self.prompt, window, cx)))
                .child(ui::field("Worktree", boxed(&self.worktree, window, cx))),
            div()
                .flex()
                .gap(px(8.))
                .child(ui::button("cancel", "Cancel").on_click(|_: &ClickEvent, _, cx| close(cx)))
                .child(
                    ui::primary_button("start", "Start")
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.submit(cx))),
                ),
        )
    }
}

// ---------------------------------------------------------------------------------------------

/// Asks before running a command that cannot be undone. `command` goes through `Store::act`
/// (with the view) when `with_view`, else `Store::cmd`.
pub struct Confirm {
    title: String,
    message: String,
    confirm: String,
    command: Vec<String>,
    with_view: bool,
    focus: FocusHandle,
}

pub fn confirm(
    title: impl Into<String>,
    message: impl Into<String>,
    confirm: impl Into<String>,
    command: Vec<String>,
    with_view: bool,
    window: &mut Window,
    cx: &mut App,
) {
    let view = cx.new(|cx| Confirm {
        title: title.into(),
        message: message.into(),
        confirm: confirm.into(),
        command,
        with_view,
        focus: cx.focus_handle(),
    });
    let focus = view.read(cx).focus.clone();
    Store::get(cx).update(cx, |s, cx| s.show_dialog(view.into(), cx));
    window.focus(&focus);
}

impl Confirm {
    fn run(&mut self, cx: &mut Context<Self>) {
        let (command, with_view) = (self.command.clone(), self.with_view);
        Store::get(cx).update(cx, |s, cx| {
            if with_view { s.act(command) } else { s.cmd(command) }
            s.close_dialog(cx);
        });
    }
}

impl Render for Confirm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, e: &gpui::KeyDownEvent, _, cx| match e.keystroke.key.as_str() {
                "enter" => this.run(cx),
                "escape" => close(cx),
                _ => {}
            }))
            .child(frame(
                self.title.clone(),
                div().text_color(colours::muted()).child(self.message.clone()),
                div()
                    .flex()
                    .gap(px(8.))
                    .child(ui::button("cancel", "Cancel").on_click(|_: &ClickEvent, _, cx| close(cx)))
                    .child(
                        ui::danger_button("confirm", self.confirm.clone())
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.run(cx))),
                    ),
            ))
    }
}
