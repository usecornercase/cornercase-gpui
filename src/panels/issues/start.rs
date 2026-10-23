//! The start form: which agent, mode, model and effort take an issue, where (for trackers not
//! tied to a repository), the worktree's branch and workspace name, and whether it starts now
//! (`start --issue`) or later (`queue add --issue`, at a time the TUI's schedule form offers).

use gpui::{
    AnyElement, App, AppContext as _, ClickEvent, Context, Entity, Focusable as _, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _, WeakEntity, Window, div,
    prelude::FluentBuilder as _, px,
};

use super::IssuesPanel;
use super::data::{Agents, Choice, EFFORT_AGENTS, Issue, Place, Sources, StartPlan, When};
use crate::dialogs;
use crate::store::Store;
use crate::theme::colours;
use crate::ui::{self, TextField, TextFieldEvent};

pub struct Setup {
    pub issue: Issue,
    pub sources: Sources,
    /// The issue belongs to the project's repository (GitHub, GitLab): no place to choose.
    pub repository: bool,
    pub agents: Agents,
    pub schedule: bool,
    pub project_name: String,
    pub panel: WeakEntity<IssuesPanel>,
}

pub struct StartForm {
    issue: Issue,
    agents: Agents,
    places: Vec<Place>,
    /// The places to choose from; empty when the issue's repository decides.
    choose_place: bool,
    place: Option<usize>,
    agent: String,
    mode: Option<String>,
    schedule: bool,
    when: When,
    model: Entity<TextField>,
    effort: Entity<TextField>,
    branch: Entity<TextField>,
    name: Entity<TextField>,
    at: Entity<TextField>,
    error: Option<String>,
    panel: WeakEntity<IssuesPanel>,
}

pub fn open(setup: Setup, window: &mut Window, cx: &mut Context<IssuesPanel>) {
    let Setup { issue, sources, repository, agents, schedule, project_name, panel } = setup;
    let (places, choose_place, place) = if repository {
        // The project's own repository: a worktree when it is a repository root, else a tab.
        let place = sources.project_place().cloned().unwrap_or(Place {
            project: sources.project,
            workspace: None,
            label: project_name,
            worktree: false,
            here: true,
        });
        (vec![place], false, Some(0))
    } else {
        let places = sources.places.clone();
        let place = match places.len() {
            1 => Some(0),
            _ => places.iter().position(|p| p.here),
        };
        (places, true, place)
    };
    let agent = agents.initial();
    let mode = agents.modes.get(&agent).map(|(chosen, _)| chosen.clone());
    let store = Store::get(cx);
    let form = cx.new(|cx| {
        let field = |placeholder: &str, text: &str, cx: &mut Context<StartForm>| {
            let field = cx.new(|cx| {
                let mut field = TextField::new(placeholder.to_string(), cx);
                field.set_text(text.to_string(), cx);
                field
            });
            cx.subscribe(&field, |this: &mut StartForm, _, event, cx| match event {
                TextFieldEvent::Submit => this.submit(cx),
                TextFieldEvent::Cancel => close(cx),
                TextFieldEvent::Changed => {
                    this.error = None;
                    cx.notify();
                }
            })
            .detach();
            field
        };
        let name_hint = format!("{} {}", issue.key, issue.title);
        StartForm {
            model: field("the agent's own, such as opus", "", cx),
            effort: field("the agent's own, such as high", "", cx),
            branch: field("the issue's branch", &issue.branch, cx),
            name: field(&name_hint, "", cx),
            at: field(When::At.placeholder().unwrap_or_default(), "", cx),
            issue,
            agents,
            places,
            choose_place,
            place,
            agent,
            mode,
            schedule,
            when: When::At,
            error: None,
            panel,
        }
    });
    let focus = {
        let form = form.read(cx);
        if form.schedule { &form.at } else { &form.model }.read(cx).focus_handle(cx)
    };
    store.update(cx, |s, cx| s.show_dialog(form.into(), cx));
    window.focus(&focus);
}

fn close(cx: &mut App) {
    Store::get(cx).update(cx, |s, cx| s.close_dialog(cx));
}

impl StartForm {
    fn place(&self) -> Option<&Place> {
        self.place.and_then(|i| self.places.get(i))
    }

    fn worktree(&self) -> bool {
        self.place().is_some_and(|p| p.worktree)
    }

    fn set_agent(&mut self, agent: String, cx: &mut Context<Self>) {
        self.mode = self.agents.modes.get(&agent).map(|(chosen, _)| chosen.clone());
        self.agent = agent;
        cx.notify();
    }

    fn plan(&self, cx: &App) -> Result<StartPlan, String> {
        let place = self.place().cloned().ok_or("Choose where to work on it.")?;
        let text = |field: &Entity<TextField>| {
            let text = field.read(cx).text().trim().to_string();
            (!text.is_empty()).then_some(text)
        };
        let chosen_mode = self.agents.modes.get(&self.agent).map(|(chosen, _)| chosen.clone());
        let worktree = place.worktree;
        let branch = text(&self.branch).filter(|b| worktree && *b != self.issue.branch);
        let effort = text(&self.effort).filter(|_| EFFORT_AGENTS.contains(&self.agent.as_str()));
        Ok(StartPlan {
            issue: self.issue.clone(),
            agent: self.agent.clone(),
            mode: self.mode.clone().filter(|m| Some(m) != chosen_mode.as_ref()),
            model: text(&self.model),
            effort,
            place: Some(place),
            branch,
            workspace_name: text(&self.name).filter(|_| worktree || self.schedule),
        })
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let plan = match self.plan(cx) {
            Ok(plan) => plan,
            Err(e) => {
                self.error = Some(e);
                cx.notify();
                return;
            }
        };
        let panel = self.panel.clone();
        if self.schedule {
            let Some(at) = self.when.at(&self.at.read(cx).text()) else {
                self.error = Some(format!("Say when: {}.", self.when.placeholder().unwrap_or_default()));
                cx.notify();
                return;
            };
            close(cx);
            let _ = panel.update(cx, |p, cx| p.schedule(plan, at, cx));
        } else {
            close(cx);
            let _ = panel.update(cx, |p, cx| p.start(plan, cx));
        }
    }

    fn chips(&self, id: &'static str, choices: &[Choice], selected: &str, cx: &mut Context<Self>, pick: fn(&mut Self, String, &mut Context<Self>)) -> AnyElement {
        div()
            .flex()
            .flex_wrap()
            .gap(px(4.))
            .children(choices.iter().enumerate().map(|(i, choice)| {
                let value = choice.value.clone();
                let is = choice.value == selected;
                ui::segment((id, i), choice.value.clone(), is)
                    .border_1()
                    .border_color(if is { colours::accent() } else { colours::border() })
                    .when(choice.dangerous, |d| d.text_color(colours::failed()))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| pick(this, value.clone(), cx)))
            }))
            .into_any_element()
    }
}

impl Render for StartForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let boxed = |field: &Entity<TextField>, window: &Window, cx: &App| {
            ui::input_box(field.clone(), field.read(cx).focus_handle(cx).is_focused(window))
        };
        let schedule = self.schedule;
        let mut body = div().flex().flex_col().gap(px(12.));
        body = body.child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(div().text_size(px(12.)).text_color(colours::accent()).child(self.issue.key.clone()))
                .child(div().text_color(colours::muted()).line_clamp(2).child(self.issue.title.clone())),
        );
        body = body.child(ui::segments(vec![
            ui::segment("start-now", "Start now", !schedule)
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.schedule = false;
                    cx.notify();
                }))
                .into_any_element(),
            ui::segment("start-later", "Schedule", schedule)
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.schedule = true;
                    cx.notify();
                }))
                .into_any_element(),
        ]));
        if schedule {
            let kinds = When::ALL.iter().enumerate().map(|(i, when)| {
                let when = *when;
                ui::segment(("when", i), when.label(), self.when == when)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.when = when;
                        let placeholder = when.placeholder().unwrap_or_default().to_string();
                        this.at = cx.new(|cx| TextField::new(placeholder, cx));
                        cx.subscribe(&this.at, |this: &mut StartForm, _, event, cx| match event {
                            TextFieldEvent::Submit => this.submit(cx),
                            TextFieldEvent::Cancel => close(cx),
                            TextFieldEvent::Changed => cx.notify(),
                        })
                        .detach();
                        cx.notify();
                    }))
                    .into_any_element()
            });
            body = body.child(ui::field(
                "When",
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(ui::segments(kinds.collect()).flex_wrap())
                    .when(self.when.placeholder().is_some(), |d| d.child(boxed(&self.at, window, cx))),
            ));
        }
        let agents = self.agents.agents.clone();
        let agent_list = if agents.is_empty() {
            vec![Choice { value: self.agent.clone(), ..Default::default() }]
        } else {
            agents
        };
        let selected_agent = self.agent.clone();
        body = body.child(ui::field("Agent", self.chips("agent", &agent_list, &selected_agent, cx, |this, value, cx| this.set_agent(value, cx))));
        if let Some((_, modes)) = self.agents.modes.get(&self.agent).cloned() {
            let mode = self.mode.clone().unwrap_or_default();
            body = body.child(ui::field(
                "Mode",
                self.chips("mode", &modes, &mode, cx, |this, value, cx| {
                    this.mode = Some(value);
                    cx.notify();
                }),
            ));
        }
        if !schedule {
            let effort = EFFORT_AGENTS.contains(&self.agent.as_str());
            body = body.child(
                div()
                    .flex()
                    .gap(px(10.))
                    .child(div().flex_1().child(ui::field("Model", boxed(&self.model, window, cx))))
                    .when(effort, |d| d.child(div().flex_1().child(ui::field("Effort", boxed(&self.effort, window, cx))))),
            );
        }
        if self.choose_place {
            let places: Vec<AnyElement> = self
                .places
                .iter()
                .enumerate()
                .map(|(i, place)| {
                    let label = format!("{}{}", place.label, if place.worktree { " · new worktree" } else { " · new tab" });
                    let is = self.place == Some(i);
                    ui::segment(("place", i), label, is)
                        .border_1()
                        .border_color(if is { colours::accent() } else { colours::border() })
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.place = Some(i);
                            this.error = None;
                            cx.notify();
                        }))
                        .into_any_element()
                })
                .collect();
            body = body.child(ui::field(
                "Where",
                if places.is_empty() {
                    ui::dim("Open a project first: this tracker's issues need a place to start in.").into_any_element()
                } else {
                    div().flex().flex_wrap().gap(px(4.)).children(places).into_any_element()
                },
            ));
        } else if let Some(place) = self.place() {
            let what = if place.worktree { "a new worktree of" } else { "a new tab in" };
            body = body.child(ui::dim(format!("Works in {what} {}, the issue's repository.", place.label)));
        }
        if self.worktree() {
            body = body.child(
                div()
                    .flex()
                    .gap(px(10.))
                    .child(div().flex_1().child(ui::field("Branch", boxed(&self.branch, window, cx))))
                    .child(div().flex_1().child(ui::field(if schedule { "Name" } else { "Workspace name" }, boxed(&self.name, window, cx)))),
            );
        } else if schedule {
            body = body.child(ui::field("Name", boxed(&self.name, window, cx)));
        }
        body = body.child(ui::dim(if schedule {
            "The queue starts the agent at that time with the issue as its prompt, accepting folder trust prompts."
        } else {
            "The agent starts with the issue as its first prompt (settings' prompt template), sent at once."
        }));
        if let Some(error) = &self.error {
            body = body.child(div().text_size(px(12.)).text_color(colours::failed()).child(error.clone()));
        }
        let label: SharedString = if schedule { "Schedule".into() } else { "Start".into() };
        dialogs::frame(
            if schedule { "Schedule work on an issue" } else { "Start work on an issue" },
            div().id("start-form").max_h(px(560.)).overflow_y_scroll().child(body),
            div()
                .flex()
                .gap(px(8.))
                .child(ui::button("start-cancel", "Cancel").on_click(|_: &ClickEvent, _, cx| close(cx)))
                .child(ui::primary_button("start-go", label).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.submit(cx)))),
        )
        .w(px(580.))
    }
}
