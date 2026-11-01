mod plan;

use gpui::{
    AnyElement, App, AppContext as _, ClickEvent, Context, Div, Entity, Focusable as _, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, Stateful, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

pub(crate) use self::plan::{FlowStart, IssueTask};
use self::plan::{Typed, hides_the_run, plan};
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
const ISSUE_NOTE: &str = "The issue is the task, through settings' prompt template.";

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
    let focus = view.read(cx).first_field().read(cx).focus_handle(cx);
    store.update(cx, |s, cx| s.show_dialog(view.into(), cx));
    window.focus(&focus);
}

impl Fields {
    fn new(issue: Option<&IssueTask>, window: &mut Window, cx: &mut Context<StartFlow>) -> Self {
        let (branch, workspace) = issue.map(|i| (i.branch.clone(), i.workspace_name())).unwrap_or_default();
        let hint = if issue.is_some() { "The issue's branch" } else { "From the task's first words, made unique" };
        Self {
            prompt: field("What the team should do", cx, window),
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
        };
        this.selected =
            preset.flow.filter(|k| this.flows.iter().any(|f| f.key() == *k)).or_else(|| this.first_choice());
        if !fresh {
            this.load(cx);
        }
        this
    }

    fn first_field(&self) -> &Entity<TextField> {
        if self.issue.is_some() { &self.fields.branch } else { &self.fields.prompt }
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
        store::run(cx, command, move |this, result, cx| this.started(&name, result, cx));
    }

    fn started(&mut self, name: &str, result: anyhow::Result<String>, cx: &mut Context<Self>) {
        self.busy = false;
        match result.and_then(|out| Ok(serde_json::from_str::<Started>(&out)?)) {
            Ok(started) => self.finish(name, &started, cx),
            Err(e) => self.error = Some(cli::reason(&e)),
        }
        cx.notify();
    }

    fn finish(&self, name: &str, started: &Started, cx: &mut Context<Self>) {
        announce(started.note.as_deref(), format!("Started {name} on {}", started.branch), cx);
        let (workspace, focus) = (started.workspace, self.focus);
        Store::get(cx).update(cx, |s, cx| {
            s.close_dialog(cx);
            s.run_started(workspace, cx);
            if hides_the_run(focus, s.sheet) {
                s.close_sheet(cx);
            }
        });
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
        if self.issue.is_some() {
            return vec![branch, ui::field("Workspace name", boxed(&fields.workspace, window, cx))];
        }
        let task = ui::field("Task", boxed(&fields.prompt, window, cx));
        vec![task, ui::field("Issue", boxed(&fields.issue, window, cx)), branch]
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
        dialogs::frame(
            title,
            self.form(window, cx),
            buttons(("start-cancel", "start-run"), (Icon::Play, "Start"), self.busy.then_some("Starting…"), cx),
        )
        .w(px(580.))
    }
}
