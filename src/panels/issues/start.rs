use gpui::{
    AnyElement, App, AppContext as _, ClickEvent, Context, Div, Entity, Focusable as _, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, WeakEntity, Window, div,
    prelude::FluentBuilder as _, px,
};

use super::IssuesPanel;
use super::data::{Agents, Choice, EFFORT_AGENTS, Issue, Place, Sources, StartPlan, When};
use crate::cli;
use crate::dialogs::{self, Gate, RunOn, RunOnEvent, accounts_of, agent_line, dialog_starts};
use crate::model::Spot;
use crate::model::machines::run_on::Starting;
use crate::model::machines::run_on::checks::Wanted;
use crate::store::{StartThere, Store};
use crate::theme::colours;
use crate::ui::account_picker::AccountPickers;
use crate::ui::{self, Icon, TextField, TextFieldEvent, Tone, boxed};

pub(super) struct Setup {
    pub issue: Issue,
    pub sources: Sources,
    pub repository: bool,
    pub agents: Agents,
    pub schedule: bool,
    pub project_name: String,
    pub panel: WeakEntity<IssuesPanel>,
}

struct Places {
    list: Vec<Place>,
    choose: bool,
    chosen: Option<usize>,
}

impl Places {
    fn of(sources: &Sources, repository: bool, project_name: String) -> Self {
        if repository {
            let place = sources.project_place().cloned().unwrap_or(Place {
                project: sources.project,
                workspace: None,
                label: project_name,
                worktree: false,
                here: true,
            });
            return Self { list: vec![place], choose: false, chosen: Some(0) };
        }
        let list = sources.places.clone();
        let chosen = match list.len() {
            1 => Some(0),
            _ => list.iter().position(|p| p.here),
        };
        Self { list, choose: true, chosen }
    }
}

pub(super) struct StartForm {
    issue: Issue,
    agents: Agents,
    places: Places,
    agent: String,
    accounts: AccountPickers,
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
    run_on: Entity<RunOn>,
}

pub(super) fn open(setup: Setup, window: &mut Window, cx: &mut Context<IssuesPanel>) {
    let store = Store::get(cx);
    let form = cx.new(|cx| StartForm::new(setup, cx));
    let focus = {
        let form = form.read(cx);
        if form.schedule { &form.at } else { &form.model }.read(cx).focus_handle(cx)
    };
    store.update(cx, |s, cx| s.show_dialog(form.into(), cx));
    window.focus(&focus);
}

fn close(cx: &mut App) {
    Store::get(cx).update(cx, Store::close_dialog);
}

fn text_field(placeholder: &str, text: &str, clear_error: bool, cx: &mut Context<StartForm>) -> Entity<TextField> {
    let field = cx.new(|cx| {
        let mut field = TextField::new(placeholder.to_string(), cx).in_tab_order().wraps();
        field.set_text(text.to_string(), cx);
        field
    });
    cx.subscribe(&field, move |this: &mut StartForm, _, event, cx| this.on_field(event, clear_error, cx)).detach();
    field
}

fn run_on_of(issue: &Issue, places: &Places, cx: &mut Context<StartForm>) -> Entity<RunOn> {
    let project = places.chosen.and_then(|at| places.list.get(at)).map(|place| place.project);
    let wanted = Wanted { accounts: true, tracker: Some(issue.source.clone()), ..Wanted::default() };
    let run_on = cx.new(|cx| RunOn::new(project, wanted, cx));
    cx.observe(&run_on, |_, _, cx| cx.notify()).detach();
    cx.subscribe(&run_on, |this, _, event, cx| this.run_on_event(event, cx)).detach();
    run_on
}

fn chips(
    id: &'static str,
    choices: &[Choice],
    selected: &str,
    pick: fn(&mut StartForm, String, &mut Context<StartForm>),
    cx: &mut Context<StartForm>,
) -> AnyElement {
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

impl StartForm {
    fn new(setup: Setup, cx: &mut Context<Self>) -> Self {
        let Setup { issue, sources, repository, agents, schedule, project_name, panel } = setup;
        let agent = agents.initial();
        let mode = agents.modes.get(&agent).map(|(chosen, _)| chosen.clone());
        let name_hint = format!("{} {}", issue.key, issue.title);
        cx.observe(&Store::get(cx), |_, _, cx| cx.notify()).detach();
        let places = Places::of(&sources, repository, project_name);
        let run_on = run_on_of(&issue, &places, cx);
        Self {
            model: text_field("the agent's own, such as opus", "", true, cx),
            effort: text_field("the agent's own, such as high", "", true, cx),
            branch: text_field("the issue's branch", &issue.branch, true, cx),
            name: text_field(&name_hint, "", true, cx),
            at: text_field(When::At.placeholder().unwrap_or_default(), "", true, cx),
            places,
            issue,
            agents,
            agent,
            accounts: AccountPickers::default(),
            mode,
            schedule,
            when: When::At,
            error: None,
            panel,
            run_on,
        }
    }

    fn run_on_event(&mut self, event: &RunOnEvent, cx: &mut Context<Self>) {
        if *event == RunOnEvent::Chosen {
            self.accounts = AccountPickers::default();
            self.error = None;
            self.schedule = self.schedule && !self.on_machine(cx);
        }
        cx.notify();
    }

    fn on_machine(&self, cx: &App) -> bool {
        self.run_on.read(cx).machine().is_some()
    }

    fn on_field(&mut self, event: &TextFieldEvent, clear_error: bool, cx: &mut Context<Self>) {
        match event {
            TextFieldEvent::Submit => self.submit(cx),
            TextFieldEvent::Cancel => close(cx),
            TextFieldEvent::Changed => {
                if clear_error {
                    self.error = None;
                }
                cx.notify();
            }
        }
    }

    fn place(&self) -> Option<&Place> {
        self.places.chosen.and_then(|i| self.places.list.get(i))
    }

    fn worktree(&self) -> bool {
        self.place().is_some_and(|p| p.worktree)
    }

    fn set_agent(&mut self, agent: String, cx: &mut Context<Self>) {
        self.mode = self.agents.modes.get(&agent).map(|(chosen, _)| chosen.clone());
        self.agent = agent;
        cx.notify();
    }

    fn clear_error(&mut self, cx: &mut Context<Self>) {
        self.error = None;
        cx.notify();
    }

    fn set_mode(&mut self, mode: String, cx: &mut Context<Self>) {
        self.mode = Some(mode);
        cx.notify();
    }

    fn set_schedule(&mut self, schedule: bool, cx: &mut Context<Self>) {
        self.schedule = schedule;
        cx.notify();
    }

    fn set_when(&mut self, when: When, cx: &mut Context<Self>) {
        self.when = when;
        self.at = text_field(when.placeholder().unwrap_or_default(), "", false, cx);
        cx.notify();
    }

    fn set_place(&mut self, index: usize, cx: &mut Context<Self>) {
        self.places.chosen = Some(index);
        self.error = None;
        let project = self.place().map(|place| place.project);
        self.run_on.update(cx, |run_on, cx| run_on.set_project(project, cx));
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
            mode: self.mode.clone().filter(|m| Some(m) != chosen_mode.as_ref() && !self.on_machine(cx)),
            model: text(&self.model),
            effort,
            place: Some(place),
            branch,
            workspace_name: text(&self.name).filter(|_| worktree || self.schedule),
            accounts: self.accounts.picks(std::slice::from_ref(&self.agent), cx),
        })
    }

    fn fail(&mut self, error: String, cx: &mut Context<Self>) {
        self.error = Some(error);
        cx.notify();
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        if dialog_starts(cx) {
            return;
        }
        let plan = match self.plan(cx) {
            Ok(plan) => plan,
            Err(e) => return self.fail(e, cx),
        };
        let back = if self.schedule { &self.at } else { &self.model }.read(cx).focus_handle(cx);
        match self.run_on.update(cx, |run_on, cx| run_on.gate(back, cx)) {
            Gate::Here => self.submit_here(&plan, cx),
            Gate::There(spot) => Self::submit_there(&plan, spot, cx),
            Gate::Held => {}
        }
    }

    fn submit_here(&mut self, plan: &StartPlan, cx: &mut Context<Self>) {
        if !self.schedule {
            Store::get(cx).update(cx, |s, cx| s.start_here(plan.start_here(), cx));
            return;
        }
        let Some(at) = self.when.at(&self.at.read(cx).text()) else {
            return self.fail(format!("Say when: {}.", self.when.placeholder().unwrap_or_default()), cx);
        };
        close(cx);
        let _ = self.panel.update(cx, |_, cx| IssuesPanel::schedule(plan, at, cx));
    }

    fn submit_there(plan: &StartPlan, spot: Spot, cx: &mut Context<Self>) {
        let starting = Starting::Agent { agent: plan.agent.clone(), issue: plan.issue.key.clone() };
        let command = plan.start_args_there(&spot);
        let start = StartThere { spot, starting, command, target: cli::target(), select: true };
        Store::get(cx).update(cx, |s, cx| s.start_there(start, cx));
    }

    fn issue_heading(&self) -> Div {
        div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(div().text_size(px(12.)).text_color(colours::accent()).child(self.issue.key.clone()))
            .child(div().text_color(colours::muted()).line_clamp(2).text_ellipsis().child(self.issue.title.clone()))
    }

    fn now_or_later(&self, cx: &mut Context<Self>) -> Option<Div> {
        (!self.on_machine(cx)).then(|| self.now_or_later_segments(cx))
    }

    fn now_or_later_segments(&self, cx: &mut Context<Self>) -> Div {
        ui::segments(vec![
            ui::segment("start-now", "Start now", !self.schedule)
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.set_schedule(false, cx)))
                .into_any_element(),
            ui::segment("start-later", "Schedule", self.schedule)
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.set_schedule(true, cx)))
                .into_any_element(),
        ])
    }

    fn when_field(&self, window: &Window, cx: &mut Context<Self>) -> Div {
        let kinds = When::ALL.iter().enumerate().map(|(i, when)| {
            let when = *when;
            ui::segment(("when", i), when.label(), self.when == when)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_when(when, cx)))
                .into_any_element()
        });
        ui::field(
            "When",
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(ui::segments(kinds.collect()).flex_wrap())
                .when(self.when.placeholder().is_some(), |d| d.child(boxed(&self.at, window, cx))),
        )
    }

    fn agent_fields(&self, cx: &mut Context<Self>) -> Vec<Div> {
        let agents = if self.agents.list.is_empty() {
            vec![Choice { value: self.agent.clone(), ..Default::default() }]
        } else {
            self.agents.list.clone()
        };
        let chosen = std::slice::from_ref(&self.agent);
        let mut fields = vec![ui::field("Agent", chips("agent", &agents, &self.agent, Self::set_agent, cx))];
        fields.extend(agent_line(&self.run_on, chosen, &self.accounts.picks(chosen, cx), cx));
        fields.extend(accounts_of(&self.run_on, self.accounts.fields(chosen, cx), cx));
        let modes = self.agents.modes.get(&self.agent).filter(|_| !self.on_machine(cx));
        if let Some((_, modes)) = modes {
            let mode = self.mode.clone().unwrap_or_default();
            fields.push(ui::field("Mode", chips("mode", modes, &mode, Self::set_mode, cx)));
        }
        fields
    }

    fn model_fields(&self, window: &Window, cx: &App) -> Div {
        let effort = EFFORT_AGENTS.contains(&self.agent.as_str());
        div()
            .flex()
            .gap(px(10.))
            .child(div().flex_1().child(ui::field("Model", boxed(&self.model, window, cx))))
            .when(effort, |d| d.child(div().flex_1().child(ui::field("Effort", boxed(&self.effort, window, cx)))))
    }

    fn line_there(&self, cx: &App) -> Option<Div> {
        let how = if self.worktree() { "In its own worktree" } else { "In a tab" };
        self.run_on.read(cx).where_line(how, cx)
    }

    fn place_field(&self, cx: &mut Context<Self>) -> Option<Div> {
        let there = self.line_there(cx);
        if !self.places.choose {
            if let Some(there) = there {
                return Some(ui::field("Where", there));
            }
            let place = self.place()?;
            let what = if place.worktree { "A new worktree of" } else { "A new tab in" };
            let text = format!("{what} {}, the issue's repository", place.label);
            return Some(ui::field("Where", div().text_size(px(12.5)).text_color(colours::soft()).child(text)));
        }
        let places: Vec<AnyElement> =
            self.places.list.iter().enumerate().map(|(i, place)| self.place_chip(i, place, cx)).collect();
        Some(ui::field(
            "Where",
            if places.is_empty() {
                ui::dim("Open a project first: this tracker's issues need a place to start in.").into_any_element()
            } else {
                let chips = div().flex().flex_wrap().gap(px(4.)).children(places);
                div().flex().flex_col().gap(px(6.)).child(chips).children(there).into_any_element()
            },
        ))
    }

    fn place_chip(&self, index: usize, place: &Place, cx: &mut Context<Self>) -> AnyElement {
        let label = format!("{}{}", place.label, if place.worktree { " · new worktree" } else { " · new tab" });
        let is = self.places.chosen == Some(index);
        ui::segment(("place", index), label, is)
            .border_1()
            .border_color(if is { colours::accent() } else { colours::border() })
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_place(index, cx)))
            .into_any_element()
    }

    fn naming_fields(&self, window: &Window, cx: &App) -> Vec<Div> {
        let worktree = self.worktree();
        let name = if self.schedule { "Name" } else { "Workspace name" };
        let branch = worktree.then(|| ui::field("Branch", boxed(&self.branch, window, cx)));
        let named = (worktree || self.schedule).then(|| ui::field(name, boxed(&self.name, window, cx)));
        branch.into_iter().chain(named).collect()
    }

    fn footnote(&self, cx: &App) -> Div {
        if let Some(machine) = self.run_on.read(cx).machine() {
            return ui::dim(format!(
                "The agent starts with the issue as its first prompt ({machine}'s prompt template, the issue's URL by \
                 default), sent at once. Schedule is hidden: the queue starts work on this computer only."
            ));
        }
        ui::dim(if self.schedule {
            "The queue starts the agent at that time with the issue as its prompt, accepting folder trust prompts."
        } else {
            "The agent starts with the issue as its first prompt (settings' prompt template), sent at once."
        })
    }

    fn go_button(&self, cx: &mut Context<Self>) -> gpui::Stateful<Div> {
        if dialog_starts(cx) {
            return ui::busy_button("start-go", "Starting…", Tone::Primary);
        }
        let (glyph, label) = if self.schedule { (Icon::Queue, "Schedule") } else { (Icon::Play, "Start") };
        ui::icon_text_button("start-go", glyph, label, Tone::Primary)
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.submit(cx)))
    }

    fn buttons(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .gap(px(8.))
            .child(ui::button("start-cancel", "Cancel").on_click(|_: &ClickEvent, _, cx| close(cx)))
            .child(self.go_button(cx))
    }
}

impl Render for StartForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let choices = self.run_on.read(cx).account_choices(cx);
        self.accounts.follow(choices, Self::clear_error, cx);
        let schedule = self.schedule;
        let body = div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(self.issue_heading())
            .children(self.now_or_later(cx))
            .when(self.run_on.read(cx).offered(cx), |d| d.child(self.run_on.clone()))
            .when(schedule, |d| d.child(self.when_field(window, cx)))
            .children(self.agent_fields(cx))
            .when(!schedule, |d| d.child(self.model_fields(window, cx)))
            .children(self.place_field(cx))
            .children(self.naming_fields(window, cx))
            .child(self.footnote(cx))
            .children(
                self.error.clone().map(|error| div().text_size(px(12.)).text_color(colours::failed()).child(error)),
            );
        dialogs::frame(
            if schedule { "Schedule work on an issue" } else { "Start work on an issue" },
            div().id("start-form").max_h(px(560.)).overflow_y_scroll().child(body),
            self.buttons(cx),
        )
        .w(px(580.))
    }
}
