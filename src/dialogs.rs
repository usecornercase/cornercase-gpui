use gpui::{
    AnyView, App, AppContext as _, ClickEvent, Context, Div, Entity, FocusHandle, Focusable, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, SharedString, Stateful, StatefulInteractiveElement as _, Styled as _,
    Window, div, px,
};

use crate::store::{self, Busy, Store, args};
use crate::theme::{colours, radius};
use crate::ui::{self, TextField, TextFieldEvent};

mod pull_request;
mod remote;
mod shortcuts;

pub(crate) use pull_request::{pull_request, pull_tag, shown_pull_request, stack_tag};
pub(crate) use remote::{PathQuestion, ask_path, connect};
pub(crate) use shortcuts::shortcuts;

pub(crate) fn frame(title: impl Into<SharedString>, body: impl IntoElement, buttons: impl IntoElement) -> Div {
    ui::glass::card(radius::SHEET)
        .w(px(480.))
        .p(px(22.))
        .flex()
        .flex_col()
        .gap(px(16.))
        .child(ui::title(title).text_size(px(15.)))
        .child(body)
        .child(div().flex().justify_end().gap(px(8.)).child(buttons))
}

pub(crate) fn cancel_and(action: Stateful<Div>) -> Div {
    div()
        .flex()
        .gap(px(8.))
        .child(ui::button("cancel", "Cancel").on_click(|_: &ClickEvent, _, cx| close(cx)))
        .child(action)
}

fn close(cx: &mut App) {
    Store::get(cx).update(cx, Store::close_dialog);
}

fn show(view: AnyView, focus: &FocusHandle, window: &mut Window, cx: &mut App) {
    Store::get(cx).update(cx, |s, cx| s.show_dialog(view, cx));
    window.focus(focus);
}

fn text_field<T: 'static>(
    placeholder: &str,
    cx: &mut Context<T>,
    on_submit: impl Fn(&mut T, &mut Window, &mut Context<T>) + 'static,
    window: &mut Window,
) -> Entity<TextField> {
    let field = cx.new(|cx| TextField::new(placeholder.to_string(), cx).in_tab_order());
    cx.subscribe_in(&field, window, move |this, _, event, window, cx| match event {
        TextFieldEvent::Submit => on_submit(this, window, cx),
        TextFieldEvent::Cancel => close(cx),
        TextFieldEvent::Changed => cx.notify(),
    })
    .detach();
    field
}

pub(crate) struct NewWorkspace {
    project: u64,
    project_name: String,
    name: Entity<TextField>,
    worktree: bool,
}

pub(crate) fn new_workspace(project: Option<u64>, window: &mut Window, cx: &mut App) {
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
    show(view.into(), &focus, window, cx);
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
        let busy = Busy::new(format!("new-workspace:{name}"), format!("Creating {name}…"));
        Store::get(cx).update(cx, |s, cx| {
            s.act_busy(busy, a, cx);
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
                    ui::checkbox("worktree", self.worktree, "Check the branch out in a git worktree of its own")
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.worktree = !this.worktree;
                            cx.notify();
                        })),
                ),
            cancel_and(
                ui::primary_button("create", "Create")
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.submit(cx))),
            ),
        )
    }
}

pub(crate) fn starting(agent: &str) -> Busy {
    let label = if agent.is_empty() { "Starting an agent…".to_string() } else { format!("Starting {agent}…") };
    Busy::new(format!("start:{agent}"), label)
}

pub(crate) struct StartAgent {
    agent: Entity<TextField>,
    prompt: Entity<TextField>,
    worktree: Entity<TextField>,
}

pub(crate) fn start_agent(window: &mut Window, cx: &mut App) {
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
    show(view.into(), &focus, window, cx);
}

impl StartAgent {
    fn submit(&mut self, cx: &mut Context<Self>) {
        let agent = self.agent.read(cx).text().trim().to_string();
        let prompt = self.prompt.read(cx).text().trim().to_string();
        let worktree = self.worktree.read(cx).text().trim().to_string();
        let busy = starting(&agent);
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
            s.act_busy(busy, a, cx);
            s.close_dialog(cx);
        });
    }
}

impl Render for StartAgent {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        frame(
            "Start an agent in a new tab",
            div()
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(ui::field("Agent", ui::boxed(&self.agent, window, cx)))
                .child(ui::field("Prompt", ui::boxed(&self.prompt, window, cx)))
                .child(ui::field("Worktree", ui::boxed(&self.worktree, window, cx))),
            cancel_and(
                ui::primary_button("start", "Start")
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.submit(cx))),
            ),
        )
    }
}

type Reply = Box<dyn FnOnce(anyhow::Result<String>, &mut App)>;
type OnYes = Box<dyn Fn(&mut App)>;

pub(crate) struct Question {
    title: String,
    message: String,
    button: String,
    command: Vec<String>,
    reply: Option<Reply>,
    on_yes: Option<OnYes>,
    busy: Option<Busy>,
}

impl Question {
    pub(crate) fn new(
        title: impl Into<String>,
        message: impl Into<String>,
        button: impl Into<String>,
        command: Vec<String>,
    ) -> Self {
        let (title, message, button) = (title.into(), message.into(), button.into());
        Self { title, message, button, command, reply: None, on_yes: None, busy: None }
    }

    pub(crate) fn busy(mut self, busy: Busy) -> Self {
        self.busy = Some(busy);
        self
    }

    pub(crate) fn on_yes(mut self, then: impl Fn(&mut App) + 'static) -> Self {
        self.on_yes = Some(Box::new(then));
        self
    }

    pub(crate) fn on_reply(mut self, reply: impl FnOnce(anyhow::Result<String>, &mut App) + 'static) -> Self {
        self.reply = Some(Box::new(reply));
        self
    }
}

pub(crate) struct Confirm {
    question: Question,
    focus: FocusHandle,
}

pub(crate) fn confirm(question: Question, window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx| Confirm { question, focus: cx.focus_handle() });
    let focus = view.read(cx).focus.clone();
    show(view.into(), &focus, window, cx);
}

impl Confirm {
    fn run(&mut self, cx: &mut Context<Self>) {
        let command = self.question.command.clone();
        if let Some(then) = self.question.on_yes.take() {
            Store::get(cx).update(cx, Store::close_dialog);
            then(cx);
            return;
        }
        let (reply, busy) = (self.question.reply.take(), self.question.busy.take());
        if let Some(reply) = reply {
            match busy {
                Some(busy) => store::queued_busy(cx, busy, command, reply),
                None => store::queued(cx, command, reply),
            }
            Store::get(cx).update(cx, Store::close_dialog);
            return;
        }
        Store::get(cx).update(cx, |s, cx| {
            match busy {
                Some(busy) => s.run_aside(busy, command, cx),
                None => s.cmd(command),
            }
            s.close_dialog(cx);
        });
    }
}

impl Render for Confirm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, e: &gpui::KeyDownEvent, _, cx| {
                if e.keystroke.key == "enter" {
                    this.run(cx);
                }
            }))
            .child(frame(
                self.question.title.clone(),
                div().text_color(colours::muted()).child(self.question.message.clone()),
                cancel_and(
                    ui::danger_button("confirm", self.question.button.clone())
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.run(cx))),
                ),
            ))
    }
}
