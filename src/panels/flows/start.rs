//! Starting a flow, as the TUI's start dialog does: pick the project, the flow (the default one
//! selected), the task (or an issue) and optionally the branch, then `cornercase flow run`. It
//! stays open and busy until the run's tabs exist, and shows a refusal in red with the task kept.

use gpui::{
    App, AppContext as _, ClickEvent, Context, Entity, Focusable as _, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _, WeakEntity, Window, div,
    prelude::FluentBuilder as _, px,
};

use super::FlowsPanel;
use super::model::{self, FlowInfo, Started};
use crate::dialogs;
use crate::store::{self, Store, ToastKind, args};
use crate::theme::colours;
use crate::ui::{self, TextField, TextFieldEvent};

pub(super) struct StartFlow {
    panel: WeakEntity<FlowsPanel>,
    projects: Vec<(u64, String)>,
    project: Option<u64>,
    flows: Vec<FlowInfo>,
    /// The selected flow, by `FlowInfo::key`.
    selected: Option<String>,
    prompt: Entity<TextField>,
    issue: Entity<TextField>,
    branch: Entity<TextField>,
    focus: bool,
    busy: bool,
    error: Option<String>,
}

impl FlowsPanel {
    /// Opens the start dialog, on `flow` (a key) or else the default flow.
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

/// A field whose enter starts and whose escape closes.
fn field(placeholder: &str, cx: &mut Context<StartFlow>, window: &mut Window) -> Entity<TextField> {
    let field = cx.new(|cx| TextField::new(placeholder.to_string(), cx));
    cx.subscribe_in(&field, window, |this, _, event, _, cx| match event {
        TextFieldEvent::Submit => this.submit(cx),
        TextFieldEvent::Cancel => close(cx),
        TextFieldEvent::Changed => cx.notify(),
    })
    .detach();
    field
}

fn close(cx: &mut App) {
    Store::get(cx).update(cx, Store::close_dialog);
}

impl StartFlow {
    /// The default flow when it can start, else the first that can.
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

    /// The flows of the chosen project (a project's own flows differ).
    fn load(&mut self, cx: &mut Context<Self>) {
        let mut a = args(&["flow", "list", "--json"]);
        if let Some(p) = self.project {
            a.extend(["--project".into(), p.to_string()]);
        }
        store::run(cx, a, |this, result, cx| {
            match result.and_then(|out| model::parse_listing(&out)) {
                Ok(listing) => {
                    let name = this.selected().map(|f| f.name.clone());
                    this.flows = listing.flows;
                    this.selected = name
                        .and_then(|n| this.flows.iter().find(|f| f.name == n && f.can_start()).map(FlowInfo::key))
                        .or_else(|| this.first_choice());
                }
                Err(e) => this.error = Some(e.to_string()),
            }
            cx.notify();
        });
    }

    fn selected(&self) -> Option<&FlowInfo> {
        self.flows.iter().find(|f| Some(f.key()) == self.selected)
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Some(flow) = self.selected().cloned() else {
            self.error = Some("Pick a flow.".into());
            cx.notify();
            return;
        };
        if let Some(problem) = flow.problems.first() {
            self.error = Some(format!("{} cannot start: {problem}", flow.name));
            cx.notify();
            return;
        }
        let prompt = self.prompt.read(cx).text().trim().to_string();
        let issue = self.issue.read(cx).text().trim().to_string();
        let branch = self.branch.read(cx).text().trim().to_string();
        if prompt.is_empty() && issue.is_empty() {
            self.error = Some("Say what the team should do, or give an issue.".into());
            cx.notify();
            return;
        }
        let mut a = args(&["flow", "run", &flow.name, "--json"]);
        if let Some(p) = self.project {
            a.extend(["--project".into(), p.to_string()]);
        }
        if issue.is_empty() {
            a.extend(["--prompt".into(), prompt]);
        } else {
            a.extend(["--issue".into(), issue]);
        }
        if !branch.is_empty() {
            a.extend(["--worktree".into(), branch]);
        }
        if self.focus {
            a.push("--focus".into());
        }
        let a = Store::get(cx).read(cx).with_view(a);
        self.busy = true;
        self.error = None;
        cx.notify();
        let name = flow.name.clone();
        store::run(cx, a, move |this, result, cx| {
            this.busy = false;
            match result.and_then(|out| Ok(serde_json::from_str::<Started>(&out)?)) {
                Ok(started) => {
                    let text = format!("Started {name} on {}", started.branch);
                    Store::get(cx).update(cx, |s, cx| {
                        if let Some(note) = &started.note {
                            s.toast(note.clone(), ToastKind::Info, cx);
                        }
                        s.toast(text, ToastKind::Done, cx);
                        s.close_dialog(cx);
                    });
                    let _ = this.panel.update(cx, |panel, cx| panel.select_workspace_run(started.workspace, cx));
                }
                Err(e) => this.error = Some(e.to_string()),
            }
            cx.notify();
        });
    }
}

impl Render for StartFlow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let boxed = |field: &Entity<TextField>, window: &Window, cx: &App| {
            ui::input_box(field.clone(), field.read(cx).focus_handle(cx).is_focused(window))
        };
        let projects = ui::segments(
            self.projects
                .iter()
                .map(|(id, name)| {
                    let id = *id;
                    ui::segment(("start-project", id), name.clone(), self.project == Some(id))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if this.project != Some(id) {
                                this.project = Some(id);
                                this.load(cx);
                            }
                        }))
                        .into_any_element()
                })
                .collect(),
        )
        .flex_wrap();
        let mut list = div().flex().flex_col().gap(px(1.));
        for (i, flow) in self.flows.iter().enumerate() {
            let key = flow.key();
            let selected = self.selected.as_deref() == Some(key.as_str());
            let second = match flow.problems.first() {
                Some(p) => div().text_size(px(11.)).text_color(colours::failed()).truncate().child(p.clone()),
                None => ui::dim(flow.description.clone()).truncate(),
            };
            list = list.child(
                ui::row(("start-flow", i), selected)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.selected = Some(key.clone());
                        this.error = None;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .flex()
                                    .gap(px(6.))
                                    .items_center()
                                    .child(div().text_size(px(13.)).child(flow.name.clone()))
                                    .children(flow.tags().into_iter().map(ui::dim))
                                    .child(div().flex_1())
                                    .child(ui::dim(format!("{} agents", flow.agents()))),
                            )
                            .child(second),
                    ),
            );
        }
        let risk = self.selected().and_then(|f| f.check.clone());
        let body = div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(ui::field("Project", projects))
            .child(ui::field(
                "Flow",
                div()
                    .id("start-flows")
                    .max_h(px(220.))
                    .overflow_y_scroll()
                    .rounded(px(5.))
                    .border_1()
                    .border_color(colours::border())
                    .p(px(2.))
                    .child(list),
            ))
            .children(risk.map(|r| div().text_size(px(12.)).text_color(colours::failed()).child(r)))
            .child(ui::field("Task", boxed(&self.prompt, window, cx)))
            .child(ui::field("Issue", boxed(&self.issue, window, cx)))
            .child(ui::field("Branch", boxed(&self.branch, window, cx)))
            .child(
                ui::checkbox("start-focus", self.focus, "Show it in the window").on_click(cx.listener(
                    |this, _: &ClickEvent, _, cx| {
                        this.focus = !this.focus;
                        cx.notify();
                    },
                )),
            )
            .child(ui::dim(
                "One tab per role in a new worktree, the orchestrator first; every agent starts without permission prompts.",
            ))
            .when_some(self.error.clone(), |d, e| {
                d.child(div().text_size(px(12.)).text_color(colours::failed()).child(SharedString::from(e)))
            });
        let label = if self.busy { "Starting…" } else { "Start" };
        dialogs::frame(
            "Start a flow",
            body,
            div()
                .flex()
                .gap(px(8.))
                .child(ui::button("start-cancel", "Cancel").on_click(|_: &ClickEvent, _, cx| close(cx)))
                .child(
                    ui::primary_button("start-run", label)
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.submit(cx))),
                ),
        )
        .w(px(580.))
    }
}
