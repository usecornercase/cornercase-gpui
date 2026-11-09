mod plan;
mod project;
mod there;

use gpui::{
    AnyElement, App, AppContext as _, ClickEvent, Context, Div, Entity, Focusable as _, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, ScrollHandle, Stateful, StatefulInteractiveElement as _, Styled as _,
    Window, div, px,
};

pub(crate) use self::plan::{FlowStart, IssueTask};
use self::plan::{Typed, issue_hints, issue_note, issue_prompt, plan, refusal};
use self::project::{ProjectChoice, ProjectMenu};
use super::FlowsPanel;
use super::actions::announce;
use super::dialog::{Submit, boxed, buttons, field_in, filled_field, prompt_field};
use super::model::{self, FlowInfo, Listing, Started, project_args};
use super::widgets::{error_text, flow_summary, glyph};
use crate::cli;
use crate::dialogs::{self, Gate, RunOn, accounts_of, dialog_starts};
use crate::store::{self, Store, args};
use crate::theme::{FONT_FAMILY, colours, radius};
use crate::ui::account_picker::AccountPickers;
use crate::ui::{self, Icon, TextField};

const CHOICE: f32 = 30.;
const TEAM_NOTE: &str =
    "One tab per role in a new worktree, the orchestrator first; every agent starts without permission prompts.";

enum IssueRead {
    Done,
    Reading,
    Failed(String),
}

struct Known {
    project: Option<u64>,
    flows: Vec<FlowInfo>,
}

struct Fields {
    scroller: ScrollHandle,
    prompt: Entity<TextField>,
    issue: Entity<TextField>,
    branch: Entity<TextField>,
    workspace: Entity<TextField>,
}

pub(super) struct StartFlow {
    issue: Option<IssueTask>,
    projects: Vec<(u64, ProjectChoice)>,
    project: Option<u64>,
    menu: ProjectMenu,
    flows: Vec<FlowInfo>,
    selected: Option<String>,
    keep_flow: Option<String>,
    loading: bool,
    fields: Fields,
    accounts: AccountPickers,
    focus: bool,
    busy: bool,
    error: Option<String>,
    issue_read: IssueRead,
    prefill: (String, String),
    run_on: Entity<RunOn>,
}

pub(crate) fn start_flow(preset: FlowStart, window: &mut Window, cx: &mut App) {
    open(preset, None, window, cx);
}

impl FlowsPanel {
    pub(super) fn open_start(&mut self, flow: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        let known = self.listing.as_ref().map(|l| Known { project: self.project, flows: l.flows.clone() });
        open(FlowStart { flow, ..FlowStart::in_project(self.project) }, known, window, cx);
    }
}

fn open(preset: FlowStart, known: Option<Known>, window: &mut Window, cx: &mut App) {
    let store = Store::get(cx);
    let Some(session) = store.read(cx).session.as_ref() else { return };
    let open: Vec<_> =
        session.projects.iter().map(|p| (p.id, ProjectChoice { name: p.name.clone(), path: p.path.clone() })).collect();
    let projects = preset.choices(&open);
    let project = preset.chosen(&projects, session.shown().project);
    let view = cx.new(|cx| StartFlow::new(preset, (projects, project), known, window, cx));
    let focus = view.read(cx).fields.prompt.read(cx).focus_handle(cx);
    store.update(cx, |s, cx| s.show_dialog(view.into(), cx));
    window.focus(&focus);
}

impl Fields {
    fn new(issue: Option<&IssueTask>, window: &mut Window, cx: &mut Context<StartFlow>) -> Self {
        let (task, branch, workspace) =
            issue.map(|i| (i.task(), i.branch.clone(), i.workspace_name())).unwrap_or_default();
        let (task_hint, hint) = match issue {
            Some(_) => issue_hints(None),
            None => ("What the team should do".to_string(), "From the task's first words, made unique".to_string()),
        };
        let scroller = ScrollHandle::new();
        Self {
            prompt: prompt_field(&task_hint, &task, &scroller, cx, window),
            issue: field_in(
                "#482, sc-48, ENG-42 or its URL (optional; a task above replaces its own)",
                &scroller,
                cx,
                window,
            ),
            branch: filled_field(&hint, &branch, &scroller, cx, window),
            workspace: filled_field("The issue's key and title", &workspace, &scroller, cx, window),
            scroller,
        }
    }
}

impl Submit for StartFlow {
    fn submit(&mut self, cx: &mut Context<Self>) {
        if self.busy || dialog_starts(cx) {
            return;
        }
        let back = self.fields.prompt.read(cx).focus_handle(cx);
        match self.run_on.update(cx, |run_on, cx| run_on.gate(back, cx)) {
            Gate::Here => self.submit_here(cx),
            Gate::There(spot) => self.submit_there(spot, cx),
            Gate::Held => {}
        }
    }
}

impl StartFlow {
    fn new(
        preset: FlowStart,
        (projects, project): (Vec<(u64, ProjectChoice)>, Option<u64>),
        known: Option<Known>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let fresh = known.as_ref().is_some_and(|k| k.project == project);
        cx.observe(&Store::get(cx), |_, _, cx| cx.notify()).detach();
        let run_on = there::run_on_of(preset.issue.as_ref(), project, cx);
        let prefill = preset.issue.as_ref().map(|i| (i.task(), i.branch.clone())).unwrap_or_default();
        let mut this = Self {
            fields: Fields::new(preset.issue.as_ref(), window, cx),
            issue: preset.issue,
            projects,
            project,
            menu: ProjectMenu::new(cx),
            flows: known.map(|k| k.flows).unwrap_or_default(),
            selected: None,
            keep_flow: None,
            loading: false,
            accounts: AccountPickers::default(),
            focus: true,
            busy: false,
            error: None,
            issue_read: IssueRead::Done,
            prefill,
            run_on,
        };
        this.selected =
            preset.flow.filter(|k| this.flows.iter().any(|f| f.key() == *k)).or_else(|| this.first_choice());
        if !fresh {
            this.load(cx);
        }
        this.read_issue(cx);
        this
    }

    fn read_issue(&mut self, cx: &mut Context<Self>) {
        let Some(issue) = self.issue.as_ref().filter(|i| i.prompt.is_none()) else { return };
        let project = self.project;
        self.issue_read = IssueRead::Reading;
        store::run(cx, issue.read_args(project), move |this, result, cx| {
            if this.project == project {
                this.issue_answered(result.and_then(|out| issue_prompt(&out)), cx);
                cx.notify();
            }
        });
    }

    fn issue_answered(&mut self, result: anyhow::Result<Option<String>>, cx: &mut Context<Self>) {
        let prompt = match result {
            Ok(prompt) => prompt,
            Err(e) => {
                self.issue_read = IssueRead::Failed(cli::reason(&e));
                return;
            }
        };
        self.issue_read = IssueRead::Done;
        if let Some(issue) = self.issue.as_mut() {
            issue.learn(prompt);
        }
        self.follow_there(cx);
    }

    fn first_choice(&self) -> Option<String> {
        let ok = |f: &&FlowInfo| f.can_start();
        self.flows
            .iter()
            .filter(ok)
            .find(|f| f.default)
            .or_else(|| self.flows.iter().filter(ok).find(|f| f.origin == "built-in"))
            .or_else(|| self.flows.iter().find(ok))
            .map(FlowInfo::key)
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        self.loading = true;
        let project = self.project;
        let mut command = args(&["flow", "list", "--json"]);
        command.extend(project_args(project));
        store::run(cx, command, move |this, result, cx| {
            if this.project == project && !this.on_machine(cx) {
                this.listed(result.and_then(|out| model::parse_listing(&out)));
                cx.notify();
            }
        });
    }

    fn listed(&mut self, result: anyhow::Result<Listing>) {
        self.loading = false;
        match result {
            Ok(listing) => self.reselect(listing.flows),
            Err(e) => self.error = Some(cli::reason(&e)),
        }
    }

    fn reselect(&mut self, flows: Vec<FlowInfo>) {
        let name = self.selected().map(|f| f.name.clone()).or(self.keep_flow.take());
        self.flows = flows;
        self.selected = name
            .and_then(|n| self.flows.iter().find(|f| f.name == n && f.can_start()).map(FlowInfo::key))
            .or_else(|| self.first_choice());
    }

    fn selected(&self) -> Option<&FlowInfo> {
        self.flows.iter().find(|f| Some(f.key()) == self.selected)
    }

    fn clear_error(&mut self, cx: &mut Context<Self>) {
        self.error = None;
        cx.notify();
    }

    fn team(&self) -> Vec<String> {
        self.selected().map(FlowInfo::team_agents).unwrap_or_default()
    }

    fn typed(&self, cx: &App) -> Typed {
        let text = |field: &Entity<TextField>| field.read(cx).text();
        let fields = &self.fields;
        Typed {
            prompt: text(&fields.prompt),
            issue: text(&fields.issue),
            branch: text(&fields.branch),
            workspace: text(&fields.workspace),
            focus: self.focus,
            accounts: self.accounts.picks(&self.team(), cx),
        }
    }

    fn submit_here(&mut self, cx: &mut Context<Self>) {
        match self.command(cx) {
            Ok((name, command)) => self.start(name, command, cx),
            Err(error) => {
                self.error = Some(error);
                cx.notify();
            }
        }
    }

    fn flow_plan(&self, cx: &App) -> Result<plan::Plan, String> {
        let Some(flow) = self.selected() else { return Err("Pick a flow.".into()) };
        if let Some(problem) = flow.problems.first() {
            return Err(format!("{} cannot start: {problem}", flow.name));
        }
        plan(&flow.name, self.project, self.shown_issue(cx).as_ref(), &self.typed(cx))
    }

    fn command(&self, cx: &App) -> Result<(String, Vec<String>), String> {
        let plan = self.flow_plan(cx)?;
        Ok((plan.flow.clone(), Store::get(cx).read(cx).with_view(plan.args())))
    }

    fn start(&mut self, name: String, command: Vec<String>, cx: &mut Context<Self>) {
        self.busy = true;
        self.error = None;
        cx.notify();
        let focus = self.focus;
        store::run(cx, command, move |this, result, cx| this.started(&name, focus, result, cx));
    }

    fn started(&mut self, name: &str, focus: bool, result: anyhow::Result<String>, cx: &mut Context<Self>) {
        self.busy = false;
        match result.and_then(|out| Ok(serde_json::from_str::<Started>(&out)?)) {
            Ok(started) => finish(name, focus, &started, cx),
            Err(e) => self.error = Some(refusal(cli::reason(&e))),
        }
        cx.notify();
    }

    fn pick_project(&mut self, id: u64, cx: &mut Context<Self>) {
        if self.project != Some(id) {
            self.project = Some(id);
            self.error = None;
            self.run_on.update(cx, |run_on, cx| run_on.set_project(Some(id), cx));
            if !self.on_machine(cx) {
                self.load(cx);
            }
            self.read_issue(cx);
            cx.notify();
        }
    }

    fn choose(&mut self, key: String, cx: &mut Context<Self>) {
        self.selected = Some(key);
        self.error = None;
        cx.notify();
    }

    fn flow_choices(&self, cx: &Context<Self>) -> Stateful<Div> {
        div()
            .id("start-flows")
            .max_h(px(220.))
            .overflow_y_scroll()
            .rounded(px(radius::control()))
            .border_1()
            .border_color(colours::border())
            .p(px(4.))
            .child(self.flow_rows(cx))
    }

    fn flow_rows(&self, cx: &Context<Self>) -> AnyElement {
        if self.flows.is_empty() {
            let reading = self.loading || self.run_on.read(cx).flows_asking();
            let note = if reading {
                ui::busy("start-flows-loading", "Reading the flows…")
            } else {
                ui::dim(self.no_flows(cx))
            };
            return note.p(px(8.)).into_any_element();
        }
        div()
            .flex()
            .flex_col()
            .gap(px(1.))
            .children(self.flows.iter().enumerate().map(|(i, flow)| self.flow_choice(i, flow, cx)))
            .into_any_element()
    }

    fn flow_choice(&self, i: usize, flow: &FlowInfo, cx: &Context<Self>) -> Stateful<Div> {
        let key = flow.key();
        let selected = self.selected.as_deref() == Some(key.as_str());
        ui::row(("start-flow", i), selected)
            .gap(px(10.))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.choose(key.clone(), cx)))
            .child(glyph(&flow.flow, None, CHOICE))
            .child(flow_summary(flow))
    }

    fn task_fields(&self, window: &Window, cx: &App) -> Vec<Div> {
        let fields = &self.fields;
        let (task, branch) = (self.task_field(window, cx), ui::field("Branch", boxed(&fields.branch, window, cx)));
        match &self.issue {
            Some(_) => vec![task, branch, ui::field("Workspace name", boxed(&fields.workspace, window, cx))],
            None => vec![task, ui::field("Issue", boxed(&fields.issue, window, cx)), branch],
        }
    }

    fn task_field(&self, window: &Window, cx: &App) -> Div {
        let field = ui::field("Task", ui::prompt_box(&self.fields.prompt, "start", window, cx));
        match &self.issue_read {
            _ if self.on_machine(cx) => field,
            IssueRead::Done => field,
            IssueRead::Reading => field.child(ui::busy("start-issue-reading", "Reading the issue's task…")),
            IssueRead::Failed(why) => field.child(error_text(format!("The issue's task could not be read: {why}"))),
        }
    }

    fn focus_checkbox(&self, cx: &Context<Self>) -> Stateful<Div> {
        ui::checkbox("start-focus", self.focus, "Show it in the window").on_click(cx.listener(
            |this, _: &ClickEvent, _, cx| {
                this.focus = !this.focus;
                cx.notify();
            },
        ))
    }

    fn footnote(&self, cx: &App) -> String {
        let note = if self.issue.is_some() {
            format!("{} {TEAM_NOTE}", issue_note(self.run_on.read(cx).machine()))
        } else {
            TEAM_NOTE.to_string()
        };
        self.footnote_there(note, cx)
    }

    fn form(&self, window: &Window, cx: &Context<Self>) -> Div {
        let risk = self.selected().and_then(|f| f.check.clone());
        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .children(self.issue.as_ref().map(|issue| ui::field("Issue", issue_line(issue))))
            .child(ui::field("Project", self.project_picker(window, cx)))
            .children(self.run_on_rows(cx))
            .child(ui::field("Flow", self.flow_choices(cx)))
            .children(risk.map(error_text))
            .children(self.task_fields(window, cx))
            .children(self.agent_note(cx))
            .children(accounts_of(&self.run_on, self.accounts.fields(&self.team(), cx), cx))
            .child(self.focus_checkbox(cx))
            .child(ui::dim(self.footnote(cx)))
            .children(self.error.clone().map(error_text))
    }
}

fn finish(name: &str, focus: bool, started: &Started, cx: &mut App) {
    announce(started.note.as_deref(), format!("Started {name} on {}", started.branch), cx);
    let (workspace, (kind, id)) = (started.workspace, started.place());
    Store::get(cx).update(cx, |s, cx| {
        s.close_dialog(cx);
        s.run_started(workspace, cx);
        if focus {
            s.focus(kind, id);
            s.show_run(cx);
        }
    });
}

fn fact_line() -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .px(px(10.))
        .py(px(6.))
        .rounded(px(radius::control()))
        .bg(colours::panel())
        .border_1()
        .border_color(colours::border())
}

fn issue_line(issue: &IssueTask) -> Div {
    fact_line()
        .child(div().flex_none().font_family(FONT_FAMILY).text_color(colours::accent()).child(issue.key.clone()))
        .child(div().flex_1().min_w_0().text_color(colours::soft()).child(ui::one_line(issue.title.clone())))
}

impl Render for StartFlow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let choices = self.run_on.read(cx).account_choices(cx);
        self.accounts.follow(choices, Self::clear_error, cx);
        let busy = (self.busy || dialog_starts(cx)).then_some("Starting…");
        let title = if self.issue.is_some() { "Start a flow on an issue" } else { "Start a flow" };
        let height = dialogs::body_height(f32::from(window.viewport_size().height));
        dialogs::frame(
            title,
            div()
                .id("start-flow-form")
                .max_h(px(height))
                .overflow_y_scroll()
                .track_scroll(&self.fields.scroller)
                .child(self.form(window, cx)),
            buttons(("start-cancel", "start-run"), (Icon::Play, "Start"), busy, cx),
        )
        .w(px(580.))
    }
}
