use gpui::{
    AnyElement, AnyView, App, AppContext as _, ClickEvent, Context, Div, Entity, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString, Stateful,
    StatefulInteractiveElement as _, Styled as _, Window, div, prelude::FluentBuilder as _, px,
};

use crate::cli;
use crate::model::Spot;
use crate::model::accounts::pick::{AccountPick, account_args};
use crate::model::machines::run_on::Starting;
use crate::model::machines::run_on::checks::Wanted;
use crate::store::{self, Busy, StartThere, Store, args};
use crate::theme::{colours, radius};
use crate::ui::account_picker::AccountPickers;
use crate::ui::{self, Icon, TextField, TextFieldEvent, Tone};

mod context;
mod merge_check;
mod pairing;
mod pull_request;
mod remote;
mod run_on;
mod shortcuts;

pub(crate) use context::{VIEW_CONTEXT, context_choices, view_context};
pub(crate) use merge_check::merge_check;
pub(crate) use pairing::{PairKey, pairing};
pub(crate) use pull_request::{pull_request, pull_tag, shown_pull_request, stack_tag};
pub(crate) use remote::{PathQuestion, ask_path, connect, ssh_hosts};
pub(crate) use run_on::{Gate, RunOn, RunOnEvent, accounts_of, agent_line, starting_there};
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

pub(crate) fn body_height(window: f32) -> f32 {
    (window - 240.).clamp(200., 560.)
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

fn muted(text: impl Into<SharedString>) -> Div {
    div().text_size(px(12.)).text_color(colours::muted()).child(text.into())
}

fn branch_line(text: String) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .text_size(px(12.))
        .text_color(colours::muted())
        .child(ui::icon(Icon::Branch, 12.).text_color(colours::faint()))
        .child(div().flex_1().min_w_0().child(ui::one_line(text)))
}

fn ask_button(workspace: u64, text: String, label: String) -> Stateful<Div> {
    ui::icon_text_button("ask-agent", Icon::Send, "Ask agent", Tone::Plain).on_click(move |_: &ClickEvent, _, cx| {
        close(cx);
        Store::get(cx).update(cx, |s, cx| s.ask_agent(workspace, &text, &label, cx));
    })
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
    run_on: Entity<RunOn>,
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
        let run_on = cx.new(|cx| RunOn::new(Some(id), Wanted::default(), cx));
        cx.observe(&run_on, |_, _, cx| cx.notify()).detach();
        NewWorkspace { project: id, project_name: name, name: name_field, worktree: true, run_on }
    });
    let focus = view.read(cx).name.read(cx).focus_handle(cx);
    show(view.into(), &focus, window, cx);
}

fn workspace_args_there(name: &str, worktree: bool, spot: &Spot) -> Vec<String> {
    let mut a = args(&["new-workspace", name]);
    a.extend(spot.starting_args());
    if worktree {
        a.push("--worktree".into());
    }
    a.push("--json".into());
    a
}

impl NewWorkspace {
    fn submit(&mut self, cx: &mut Context<Self>) {
        let name = self.name.read(cx).text().trim().to_string();
        if name.is_empty() || starting_there(cx) {
            return;
        }
        let back = self.name.read(cx).focus_handle(cx);
        match self.run_on.update(cx, |run_on, cx| run_on.gate(back, cx)) {
            Gate::Here => self.create_here(&name, cx),
            Gate::There(spot) => self.create_there(name, spot, cx),
            Gate::Held => {}
        }
    }

    fn create_here(&self, name: &str, cx: &mut Context<Self>) {
        let mut a = args(&["new-workspace", name, "--project", &self.project.to_string(), "--focus"]);
        if self.worktree {
            a.push("--worktree".into());
        }
        let busy = Busy::new(format!("new-workspace:{name}"), format!("Creating {name}…"));
        Store::get(cx).update(cx, |s, cx| {
            s.act_busy(busy, a, cx);
            s.close_dialog(cx);
        });
    }

    fn create_there(&self, name: String, spot: Spot, cx: &mut Context<Self>) {
        let command = workspace_args_there(&name, self.worktree, &spot);
        let start =
            StartThere { spot, starting: Starting::Workspace { name }, command, target: cli::target(), select: true };
        Store::get(cx).update(cx, |s, cx| s.start_there(start, cx));
    }

    fn create_button(cx: &Context<Self>) -> Stateful<Div> {
        if starting_there(cx) {
            return ui::busy_button("create", "Creating…", Tone::Primary);
        }
        ui::primary_button("create", "Create").on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.submit(cx)))
    }

    fn where_line(&self, cx: &App) -> Option<Div> {
        let how = if self.worktree { "In its own worktree" } else { "In a workspace" };
        self.run_on.read(cx).where_line(how, cx)
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
                )
                .when(self.run_on.read(cx).offered(cx), |d| d.child(self.run_on.clone()))
                .children(self.where_line(cx)),
            cancel_and(Self::create_button(cx)),
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
    accounts: AccountPickers,
}

fn start_args(agent: &str, prompt: &str, worktree: &str, picks: &[AccountPick]) -> Vec<String> {
    let mut a = args(&["start"]);
    if !agent.is_empty() {
        a.push(agent.to_string());
    }
    a.push("--focus".into());
    if !prompt.is_empty() {
        a.extend(args(&["--prompt", prompt]));
    }
    if !worktree.is_empty() {
        a.extend(args(&["--worktree", worktree]));
    }
    a.extend(account_args(picks));
    a
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
        cx.observe(&store, |_, _, cx| cx.notify()).detach();
        StartAgent { agent, prompt, worktree, accounts: AccountPickers::default() }
    });
    let focus = view.read(cx).prompt.read(cx).focus_handle(cx);
    show(view.into(), &focus, window, cx);
}

impl StartAgent {
    fn agents(&self, cx: &App) -> Vec<String> {
        let typed = self.agent.read(cx).text().trim().to_string();
        let default = Store::get(cx).read(cx).account_settings.default_agent.clone();
        Some(typed).filter(|agent| !agent.is_empty()).or(default).into_iter().collect()
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let agent = self.agent.read(cx).text().trim().to_string();
        let prompt = self.prompt.read(cx).text().trim().to_string();
        let worktree = self.worktree.read(cx).text().trim().to_string();
        let busy = starting(&agent);
        let a = start_args(&agent, &prompt, &worktree, &self.accounts.picks(&self.agents(cx), cx));
        Store::get(cx).update(cx, |s, cx| {
            s.act_busy(busy, a, cx);
            s.close_dialog(cx);
        });
    }
}

impl Render for StartAgent {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let choices = Store::get(cx).read(cx).account_choices();
        self.accounts.follow(choices, |_, _| {}, cx);
        frame(
            "Start an agent in a new tab",
            div()
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(ui::field("Agent", ui::boxed(&self.agent, window, cx)))
                .children(self.accounts.fields(&self.agents(cx), cx))
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
type Body = Box<dyn Fn(bool) -> AnyElement>;

pub(crate) struct Tick {
    label: String,
    button: String,
    on_yes: OnYes,
    on: bool,
}

impl Tick {
    pub(crate) fn new(
        label: impl Into<String>,
        button: impl Into<String>,
        on_yes: impl Fn(&mut App) + 'static,
    ) -> Self {
        Self { label: label.into(), button: button.into(), on_yes: Box::new(on_yes), on: false }
    }
}

pub(crate) struct Question {
    title: String,
    message: String,
    button: String,
    command: Vec<String>,
    reply: Option<Reply>,
    on_yes: Option<OnYes>,
    busy: Option<Busy>,
    body: Option<Body>,
    tick: Option<Tick>,
}

impl Question {
    pub(crate) fn new(
        title: impl Into<String>,
        message: impl Into<String>,
        button: impl Into<String>,
        command: Vec<String>,
    ) -> Self {
        let (title, message, button) = (title.into(), message.into(), button.into());
        Self { title, message, button, command, reply: None, on_yes: None, busy: None, body: None, tick: None }
    }

    pub(crate) fn body(mut self, body: impl Fn(bool) -> AnyElement + 'static) -> Self {
        self.body = Some(Box::new(body));
        self
    }

    pub(crate) fn tick(mut self, tick: Tick) -> Self {
        self.tick = Some(tick);
        self
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
    fn ticked(&self) -> Option<&Tick> {
        self.question.tick.as_ref().filter(|tick| tick.on)
    }

    fn toggle_tick(&mut self, cx: &mut Context<Self>) {
        if let Some(tick) = &mut self.question.tick {
            tick.on = !tick.on;
            cx.notify();
        }
    }

    fn said(&self, window: &Window) -> AnyElement {
        let Some(body) = &self.question.body else {
            return div().text_color(colours::muted()).child(self.question.message.clone()).into_any_element();
        };
        div()
            .id("question-body")
            .max_h(px(body_height(f32::from(window.viewport_size().height))))
            .overflow_y_scroll()
            .child(body(self.ticked().is_some()))
            .into_any_element()
    }

    fn tick_box(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let tick = self.question.tick.as_ref()?;
        Some(
            ui::checkbox("question-tick", tick.on, tick.label.clone())
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_tick(cx))),
        )
    }

    fn run(&mut self, cx: &mut Context<Self>) {
        if let Some(tick) = self.question.tick.take_if(|tick| tick.on) {
            Store::get(cx).update(cx, Store::close_dialog);
            (tick.on_yes)(cx);
            return;
        }
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let button = self.ticked().map_or(&self.question.button, |tick| &tick.button).clone();
        let said = self.said(window);
        let tick = self.tick_box(cx);
        div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, e: &gpui::KeyDownEvent, _, cx| {
                if e.keystroke.key == "enter" {
                    this.run(cx);
                }
            }))
            .child(frame(
                self.question.title.clone(),
                div().flex().flex_col().gap(px(12.)).child(said).children(tick),
                cancel_and(
                    ui::danger_button("confirm", button)
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.run(cx))),
                ),
            ))
    }
}

#[cfg(test)]
mod tests;
