use gpui::{
    App, AppContext as _, ClickEvent, Context, Div, Entity, Focusable as _, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, Stateful, StatefulInteractiveElement as _, Styled as _, WeakEntity, Window, div, px,
};

use super::FlowsPanel;
use super::actions::announce;
use super::dialog::{Submit, boxed, buttons, close_dialog, field};
use super::model::{self, FlowInfo, Started, project_args};
use super::widgets::{error_text, flow_summary, glyph};
use crate::dialogs;
use crate::store::{self, Store, args};
use crate::theme::{colours, radius};
use crate::ui::{self, Icon, TextField};

const CHOICE: f32 = 30.;

pub(super) struct StartFlow {
    panel: WeakEntity<FlowsPanel>,
    projects: Vec<(u64, String)>,
    project: Option<u64>,
    flows: Vec<FlowInfo>,
    selected: Option<String>,
    prompt: Entity<TextField>,
    issue: Entity<TextField>,
    branch: Entity<TextField>,
    focus: bool,
    busy: bool,
    error: Option<String>,
}

impl FlowsPanel {
    pub(super) fn open_start(&mut self, flow: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        let store = Store::get(cx);
        let Some(session) = store.read(cx).session.clone() else { return };
        let projects: Vec<(u64, String)> = session.projects.iter().map(|p| (p.id, p.name.clone())).collect();
        let project = self.project.or(session.shown().project).or(projects.first().map(|p| p.0));
        let flows = self.listing.as_ref().map(|l| l.flows.clone()).unwrap_or_default();
        let panel = cx.entity().downgrade();
        let view = cx.new(|cx| {
            let prompt = field("What the team should do", cx, window);
            let issue = field("#482, sc-48, ENG-42 or its URL (optional, instead of the task)", cx, window);
            let branch = field("From the task's first words, made unique", cx, window);
            let mut this = StartFlow {
                panel,
                projects,
                project,
                flows,
                selected: None,
                prompt,
                issue,
                branch,
                focus: true,
                busy: false,
                error: None,
            };
            this.selected = flow.filter(|k| this.flows.iter().any(|f| f.key() == *k)).or_else(|| this.first_choice());
            if this.project != self.project {
                this.load(cx);
            }
            this
        });
        let focus = view.read(cx).prompt.read(cx).focus_handle(cx);
        store.update(cx, |s, cx| s.show_dialog(view.into(), cx));
        window.focus(&focus);
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
        let mut command = args(&["flow", "list", "--json"]);
        command.extend(project_args(self.project));
        store::run(cx, command, |this, result, cx| {
            match result.and_then(|out| model::parse_listing(&out)) {
                Ok(listing) => this.reselect(listing.flows),
                Err(e) => this.error = Some(e.to_string()),
            }
            cx.notify();
        });
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

    fn command(&self, cx: &App) -> Result<(String, Vec<String>), String> {
        let Some(flow) = self.selected() else { return Err("Pick a flow.".into()) };
        if let Some(problem) = flow.problems.first() {
            return Err(format!("{} cannot start: {problem}", flow.name));
        }
        let text = |field: &Entity<TextField>| field.read(cx).text().trim().to_string();
        let (prompt, issue, branch) = (text(&self.prompt), text(&self.issue), text(&self.branch));
        if prompt.is_empty() && issue.is_empty() {
            return Err("Say what the team should do, or give an issue.".into());
        }
        let mut command = args(&["flow", "run", &flow.name, "--json"]);
        command.extend(project_args(self.project));
        command.extend(if issue.is_empty() { ["--prompt".into(), prompt] } else { ["--issue".into(), issue] });
        if !branch.is_empty() {
            command.extend(["--worktree".into(), branch]);
        }
        if self.focus {
            command.push("--focus".into());
        }
        Ok((flow.name.clone(), Store::get(cx).read(cx).with_view(command)))
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
            Err(e) => self.error = Some(e.to_string()),
        }
        cx.notify();
    }

    fn finish(&self, name: &str, started: &Started, cx: &mut Context<Self>) {
        announce(started.note.as_deref(), format!("Started {name} on {}", started.branch), cx);
        close_dialog(cx);
        let workspace = started.workspace;
        let _ = self.panel.update(cx, |panel, cx| panel.select_workspace_run(workspace, cx));
    }

    fn pick_project(&mut self, id: u64, cx: &mut Context<Self>) {
        if self.project != Some(id) {
            self.project = Some(id);
            self.load(cx);
        }
    }

    fn choose(&mut self, key: String, cx: &mut Context<Self>) {
        self.selected = Some(key);
        self.error = None;
        cx.notify();
    }

    fn project_choices(&self, cx: &Context<Self>) -> Div {
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
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(1.))
                    .children(self.flows.iter().enumerate().map(|(i, flow)| self.flow_choice(i, flow, cx))),
            )
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

    fn form(&self, window: &Window, cx: &Context<Self>) -> Div {
        let risk = self.selected().and_then(|f| f.check.clone());
        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(ui::field("Project", self.project_choices(cx)))
            .child(ui::field("Flow", self.flow_choices(cx)))
            .children(risk.map(error_text))
            .child(ui::field("Task", boxed(&self.prompt, window, cx)))
            .child(ui::field("Issue", boxed(&self.issue, window, cx)))
            .child(ui::field("Branch", boxed(&self.branch, window, cx)))
            .child(ui::checkbox("start-focus", self.focus, "Show it in the window").on_click(cx.listener(
                |this, _: &ClickEvent, _, cx| {
                    this.focus = !this.focus;
                    cx.notify();
                },
            )))
            .child(ui::dim(
                "One tab per role in a new worktree, the orchestrator first; every agent starts without permission prompts.",
            ))
            .children(self.error.clone().map(error_text))
    }
}

impl Render for StartFlow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        dialogs::frame(
            "Start a flow",
            self.form(window, cx),
            buttons(("start-cancel", "start-run"), (Icon::Play, "Start"), self.busy.then_some("Starting…"), cx),
        )
        .w(px(580.))
    }
}
