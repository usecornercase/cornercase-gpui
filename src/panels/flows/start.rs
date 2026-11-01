mod plan;

use gpui::{
    AnyElement, App, AppContext as _, ClickEvent, Context, Div, Entity, Focusable as _, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, Stateful, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

pub(crate) use self::plan::{FlowStart, IssueTask};
use self::plan::{Typed, hides_the_run, issue_prompt, plan};
use super::FlowsPanel;
use super::actions::announce;
use super::dialog::{Submit, boxed, buttons, field, filled_field};
use super::model::{self, FlowInfo, Listing, Started, project_args};
use super::widgets::{error_text, flow_summary, glyph};
use crate::cli;
use crate::dialogs;
use crate::store::{self, Store, args};
use crate::theme::{FONT_FAMILY, colours, radius};
use crate::ui::{self, Icon, TextField};

const CHOICE: f32 = 30.;
const TEAM_NOTE: &str =
    "One tab per role in a new worktree, the orchestrator first; every agent starts without permission prompts.";
const ISSUE_NOTE: &str = "The task starts as the issue's, through settings' prompt template.";
const RENAMED: &str =
    "With an edited task, the workspace is named after its branch: needs usecornercase/cornercase-terminal#573.";

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
    prompt: Entity<TextField>,
    issue: Entity<TextField>,
    branch: Entity<TextField>,
    workspace: Entity<TextField>,
}

pub(super) struct StartFlow {
    issue: Option<IssueTask>,
    projects: Vec<(u64, String)>,
    project: Option<u64>,
    flows: Vec<FlowInfo>,
    selected: Option<String>,
    loading: bool,
    fields: Fields,
    focus: bool,
    busy: bool,
    error: Option<String>,
    issue_read: IssueRead,
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
    let projects = preset.choices(&session.projects.iter().map(|p| (p.id, p.name.clone())).collect::<Vec<_>>());
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
            Some(_) => ("The issue's, through settings' prompt template", "The issue's branch"),
            None => ("What the team should do", "From the task's first words, made unique"),
        };
        Self {
            prompt: filled_field(task_hint, &task, cx, window),
            issue: field("#482, sc-48, ENG-42 or its URL (optional, instead of the task)", cx, window),
            branch: filled_field(hint, &branch, cx, window),
            workspace: filled_field("The issue's key and title", &workspace, cx, window),
        }
    }
}

impl Submit for StartFlow {
    fn submit(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        match self.command(cx) {
            Ok((name, command)) => self.start(name, command, cx),
            Err(error) => {
                self.error = Some(error);
                cx.notify();
            }
        }
    }
}

impl StartFlow {
    fn new(
        preset: FlowStart,
        (projects, project): (Vec<(u64, String)>, Option<u64>),
        known: Option<Known>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let fresh = known.as_ref().is_some_and(|k| k.project == project);
        let mut this = Self {
            fields: Fields::new(preset.issue.as_ref(), window, cx),
            issue: preset.issue,
            projects,
            project,
            flows: known.map(|k| k.flows).unwrap_or_default(),
            selected: None,
            loading: false,
            focus: true,
            busy: false,
            error: None,
            issue_read: IssueRead::Done,
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
        self.issue_read = IssueRead::Reading;
        store::run(cx, issue.read_args(self.project), |this, result, cx| {
            this.issue_answered(result.and_then(|out| issue_prompt(&out)), cx);
            cx.notify();
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
        let Some(issue) = self.issue.as_mut() else { return };
        issue.prompt = prompt;
        let task = issue.task();
        self.fields.prompt.update(cx, |field, cx| {
            if field.text().is_empty() {
                field.set_text(task, cx);
            }
        });
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
            if this.project == project {
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
        let name = self.selected().map(|f| f.name.clone());
        self.flows = flows;
        self.selected = name
            .and_then(|n| self.flows.iter().find(|f| f.name == n && f.can_start()).map(FlowInfo::key))
            .or_else(|| self.first_choice());
    }

    fn selected(&self) -> Option<&FlowInfo> {
        self.flows.iter().find(|f| Some(f.key()) == self.selected)
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
        }
    }

    fn command(&self, cx: &App) -> Result<(String, Vec<String>), String> {
        let Some(flow) = self.selected() else { return Err("Pick a flow.".into()) };
        if let Some(problem) = flow.problems.first() {
            return Err(format!("{} cannot start: {problem}", flow.name));
        }
        let plan = plan(&flow.name, self.project, self.issue.as_ref(), &self.typed(cx))?;
        Ok((flow.name.clone(), Store::get(cx).read(cx).with_view(plan.args())))
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
            Err(e) => self.error = Some(cli::reason(&e)),
        }
        cx.notify();
    }

    fn pick_project(&mut self, id: u64, cx: &mut Context<Self>) {
        if self.project != Some(id) {
            self.project = Some(id);
            self.error = None;
            self.load(cx);
            cx.notify();
        }
    }

    fn choose(&mut self, key: String, cx: &mut Context<Self>) {
        self.selected = Some(key);
        self.error = None;
        cx.notify();
    }

    fn project_choices(&self, cx: &Context<Self>) -> Div {
        if self.projects.is_empty() {
            return ui::dim(
                "Open a git repository's folder as a project first: a flow works in a worktree of its own.",
            );
        }
        let segments = self.projects.iter().map(|(id, name)| {
            let id = *id;
            ui::segment(("start-project", id), name.clone(), self.project == Some(id))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.pick_project(id, cx)))
                .into_any_element()
        });
        ui::segments(segments.collect()).flex_wrap()
    }

    fn flow_choices(&self, cx: &Context<Self>) -> Stateful<Div> {
        div()
            .id("start-flows")
            .max_h(px(220.))
            .overflow_y_scroll()
            .rounded(px(radius::CONTROL))
            .border_1()
            .border_color(colours::border())
            .p(px(4.))
            .child(self.flow_rows(cx))
    }

    fn flow_rows(&self, cx: &Context<Self>) -> AnyElement {
        if self.flows.is_empty() {
            let note = if self.loading {
                ui::busy("start-flows-loading", "Reading the flows…")
            } else {
                ui::dim("No flows to start here.")
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
        let branch = ui::field("Branch", boxed(&fields.branch, window, cx));
        match &self.issue {
            Some(issue) => vec![self.task_field(window, cx), branch, self.workspace_field(issue, window, cx)],
            None => vec![self.task_field(window, cx), ui::field("Issue", boxed(&fields.issue, window, cx)), branch],
        }
    }

    fn task_field(&self, window: &Window, cx: &App) -> Div {
        let field = ui::field("Task", boxed(&self.fields.prompt, window, cx));
        match &self.issue_read {
            IssueRead::Done => field,
            IssueRead::Reading => field.child(ui::busy("start-issue-reading", "Reading the issue's task…")),
            IssueRead::Failed(why) => field.child(error_text(format!("The issue's task could not be read: {why}"))),
        }
    }

    fn workspace_field(&self, issue: &IssueTask, window: &Window, cx: &App) -> Div {
        if issue.edited(&self.fields.prompt.read(cx).text()).is_some() {
            return ui::field("Workspace name", ui::dim(RENAMED));
        }
        ui::field("Workspace name", boxed(&self.fields.workspace, window, cx))
    }

    fn focus_checkbox(&self, cx: &Context<Self>) -> Stateful<Div> {
        ui::checkbox("start-focus", self.focus, "Show it in the window").on_click(cx.listener(
            |this, _: &ClickEvent, _, cx| {
                this.focus = !this.focus;
                cx.notify();
            },
        ))
    }

    fn footnote(&self) -> String {
        if self.issue.is_some() { format!("{ISSUE_NOTE} {TEAM_NOTE}") } else { TEAM_NOTE.to_string() }
    }

    fn form(&self, window: &Window, cx: &Context<Self>) -> Div {
        let risk = self.selected().and_then(|f| f.check.clone());
        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .children(self.issue.as_ref().map(|issue| ui::field("Issue", issue_line(issue))))
            .child(ui::field("Project", self.project_choices(cx)))
            .child(ui::field("Flow", self.flow_choices(cx)))
            .children(risk.map(error_text))
            .children(self.task_fields(window, cx))
            .child(self.focus_checkbox(cx))
            .child(ui::dim(self.footnote()))
            .children(self.error.clone().map(error_text))
    }
}

fn finish(name: &str, focus: bool, started: &Started, cx: &mut App) {
    announce(started.note.as_deref(), format!("Started {name} on {}", started.branch), cx);
    let workspace = started.workspace;
    Store::get(cx).update(cx, |s, cx| {
        s.close_dialog(cx);
        s.run_started(workspace, cx);
        if hides_the_run(focus, s.sheet) {
            s.close_sheet(cx);
        }
    });
}

fn issue_line(issue: &IssueTask) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .px(px(10.))
        .py(px(6.))
        .rounded(px(radius::CONTROL))
        .bg(colours::panel())
        .border_1()
        .border_color(colours::border())
        .child(div().flex_none().font_family(FONT_FAMILY).text_color(colours::accent()).child(issue.key.clone()))
        .child(div().flex_1().min_w_0().text_color(colours::soft()).child(ui::one_line(issue.title.clone())))
}

impl Render for StartFlow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = if self.issue.is_some() { "Start a flow on an issue" } else { "Start a flow" };
        let height = dialogs::body_height(f32::from(window.viewport_size().height));
        dialogs::frame(
            title,
            div().id("start-flow-form").max_h(px(height)).overflow_y_scroll().child(self.form(window, cx)),
            buttons(("start-cancel", "start-run"), (Icon::Play, "Start"), self.busy.then_some("Starting…"), cx),
        )
        .w(px(580.))
    }
}
