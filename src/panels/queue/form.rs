use gpui::{App, AppContext as _, Context, Entity, Focusable as _, Subscription, Window};

use super::data::{Entry, flow_names, repository_for};
use super::draft::{Draft, Target, When, cleanup_flag};
use super::repeat::{DAYS, Repeat};
use super::time::local;
use super::{QueuePanel, toast_error};
use crate::cli;
use crate::model::Session;
use crate::panels::flows::model::{parse_listing, project_args};
use crate::store::{self, Store, ToastKind, args};
use crate::ui::account_picker::AccountPickers;
use crate::ui::{TextField, TextFieldEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Spot {
    Keep,
    Workspace(u64),
    NewWorktree,
}

impl From<Option<u64>> for Spot {
    fn from(workspace: Option<u64>) -> Self {
        workspace.map_or(Spot::NewWorktree, Spot::Workspace)
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct Old {
    name: String,
    prompt: String,
    pub(super) repeat: Option<String>,
    flow: Option<String>,
    cleanup: bool,
    agent: String,
}

impl Old {
    fn of(entry: Option<&Entry>) -> Self {
        entry.map_or_else(Self::default, |e| Self {
            name: e.label.clone(),
            prompt: e.prompt.clone(),
            repeat: e.repeat.clone(),
            flow: e.flow.clone(),
            cleanup: e.cleanup,
            agent: e.agent.clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Times {
    pub(super) clock: String,
    pub(super) date: String,
    pub(super) amount: String,
    pub(super) unit: char,
    pub(super) weekday: &'static str,
}

impl Times {
    pub(super) fn of(entry: Option<&Entry>) -> Self {
        let at = entry.and_then(|e| local(e.at));
        let repeat = entry.and_then(|e| e.repeat.as_deref()).and_then(Repeat::parse);
        let (amount, unit) = match (&repeat, entry) {
            (Some(Repeat::Every { amount, unit, .. }), _) => (amount.to_string(), *unit),
            (None, Some(e)) if !e.is_routine() => (String::new(), 'h'),
            _ => ("1".to_string(), 'h'),
        };
        let weekday = if let Some(Repeat::Week(day, _)) = &repeat { *day } else { DAYS[0] };
        let clock = repeat.as_ref().and_then(Repeat::clock).map(str::to_string);
        Self {
            clock: clock.or_else(|| at.map(|c| c.clock())).unwrap_or_default(),
            date: at.map(|c| c.date()).unwrap_or_default(),
            amount,
            unit,
            weekday,
        }
    }
}

pub(super) struct Form {
    pub(super) edit: Option<u64>,
    pub(super) issue: bool,
    pub(super) prompt: Entity<TextField>,
    pub(super) issue_key: Entity<TextField>,
    pub(super) source: Option<String>,
    pub(super) when: When,
    pub(super) clock: Entity<TextField>,
    pub(super) date: Entity<TextField>,
    pub(super) amount: Entity<TextField>,
    pub(super) unit: char,
    pub(super) weekday: &'static str,
    pub(super) project: Option<u64>,
    pub(super) spot: Spot,
    pub(super) branch: Entity<TextField>,
    pub(super) agent: Option<String>,
    pub(super) accounts: AccountPickers,
    pub(super) mode: Option<String>,
    pub(super) flow: Option<String>,
    pub(super) cleanup: bool,
    pub(super) name: Entity<TextField>,
    pub(super) old: Old,
    pub(super) error: Option<String>,
    pub(super) busy: bool,
    _subscriptions: Vec<Subscription>,
}

#[derive(Default)]
struct Fields {
    subscriptions: Vec<Subscription>,
}

impl Fields {
    fn add(
        &mut self,
        placeholder: &str,
        text: &str,
        window: &mut Window,
        cx: &mut Context<QueuePanel>,
    ) -> Entity<TextField> {
        let field = cx.new(|cx| {
            let mut field = TextField::new(placeholder.to_string(), cx).wraps();
            if !text.is_empty() {
                field.set_text(text.to_string(), cx);
            }
            field
        });
        self.subscriptions.push(cx.subscribe_in(&field, window, |this, _, event, _, cx| match event {
            TextFieldEvent::Submit => this.save(cx),
            TextFieldEvent::Cancel => this.close_form(cx),
            TextFieldEvent::Changed => cx.notify(),
        }));
        field
    }
}

pub(super) fn starting_agent(
    runs_flow: bool,
    chosen: Option<&str>,
    kept: Option<&str>,
    default: Option<&str>,
) -> Vec<String> {
    let agent = chosen.or(kept).or(default).filter(|_| !runs_flow);
    agent.map(str::to_string).into_iter().collect()
}

fn start_place(entry: Option<&Entry>, session: Option<&Session>) -> (Option<u64>, Spot) {
    let Some(entry) = entry else {
        let shown = session.map(Session::shown).unwrap_or_default();
        return (shown.project, shown.workspace.into());
    };
    let project = session.and_then(|s| s.projects.iter().find(|p| p.path == entry.place.project));
    (project.map(|p| p.id), Spot::Keep)
}

impl Form {
    fn new(
        entry: Option<&Entry>,
        session: Option<&Session>,
        window: &mut Window,
        cx: &mut Context<QueuePanel>,
    ) -> Self {
        let times = Times::of(entry);
        let mut f = Fields::default();
        let prompt = f.add("What the agent should do", entry.map_or("", |e| &e.prompt), window, cx);
        let issue_key = f.add("https://github.com/org/repo/issues/12, #12 or ENG-42", "", window, cx);
        let clock = f.add("14:30", &times.clock, window, cx);
        let date = f.add("2026-10-24", &times.date, window, cx);
        let amount = f.add("1", &times.amount, window, cx);
        let branch = f.add("feat/my-branch", "", window, cx);
        let name = f.add("the tab's name (optional)", entry.map_or("", |e| &e.label), window, cx);
        let (project, spot) = start_place(entry, session);
        Self {
            edit: entry.map(|e| e.id),
            issue: false,
            prompt,
            issue_key,
            source: None,
            when: if entry.is_some() { When::Keep } else { When::In },
            clock,
            date,
            amount,
            unit: times.unit,
            weekday: times.weekday,
            project,
            spot,
            branch,
            agent: None,
            accounts: AccountPickers::keeping(entry.map(|e| e.accounts.clone()).unwrap_or_default()),
            mode: None,
            flow: None,
            cleanup: entry.is_some_and(|e| e.cleanup),
            name,
            old: Old::of(entry),
            error: None,
            busy: false,
            _subscriptions: f.subscriptions,
        }
    }

    pub(super) fn editing(&self) -> bool {
        self.edit.is_some()
    }

    pub(super) fn repeats(&self) -> bool {
        self.when.repeats() || self.old.repeat.is_some()
    }

    pub(super) fn runs_flow(&self) -> bool {
        self.flow.is_some() || (self.agent.is_none() && self.old.flow.is_some())
    }

    pub(super) fn account_agents(&self, default: Option<&str>) -> Vec<String> {
        let kept = self.editing().then_some(self.old.agent.as_str());
        starting_agent(self.runs_flow(), self.agent.as_deref(), kept, default)
    }

    pub(super) fn choose_when(&mut self, kind: When) {
        self.when = kind;
        if !self.repeats() {
            self.flow = None;
        }
    }

    pub(super) fn fit_repository(&mut self, session: &Session) {
        if !self.repeats() || self.spot == Spot::Keep {
            return;
        }
        let fitting = repository_for(self.project, session);
        if fitting != self.project {
            self.project = fitting;
            self.spot = Spot::NewWorktree;
        }
    }

    fn target(&self, cx: &App) -> Target {
        match (self.spot, self.project) {
            (Spot::Keep, _) => Target::Keep,
            (_, Some(project)) if self.repeats() => Target::Repository(project),
            (Spot::Workspace(id), _) if !self.repeats() => Target::Workspace(id),
            (Spot::NewWorktree, Some(project)) => Target::Worktree { project, branch: self.branch.read(cx).text() },
            _ => Target::Keep,
        }
    }

    fn draft(&self, cx: &App) -> Draft {
        let text = |f: &Entity<TextField>| f.read(cx).text();
        let default = Store::get(cx).read(cx).account_settings.default_agent.clone();
        let editing = self.editing();
        let issue = self.issue && !self.repeats();
        let mut body = if issue { text(&self.issue_key) } else { text(&self.prompt) };
        if editing && !issue && body.trim() == self.old.prompt.trim() {
            body.clear();
        }
        let name = text(&self.name);
        let name = if editing && name.trim() == self.old.name.trim() { None } else { Some(name) };
        Draft {
            edit: self.edit,
            issue,
            text: body,
            source: self.source.clone(),
            when: self.when,
            clock: text(&self.clock),
            date: text(&self.date),
            amount: text(&self.amount),
            unit: self.unit,
            weekday: self.weekday.to_string(),
            target: self.target(cx),
            agent: self.agent.clone(),
            mode: self.mode.clone(),
            flow: self.flow.clone().filter(|_| self.repeats()),
            cleanup: cleanup_flag(self.repeats(), self.cleanup, editing.then_some(self.old.cleanup)),
            name,
            accounts: self.accounts.picks(&self.account_agents(default.as_deref()), cx),
        }
    }
}

#[derive(Default)]
pub(super) struct FlowChoices {
    asked: bool,
    project: Option<u64>,
    pub(super) names: Option<Vec<String>>,
}

impl FlowChoices {
    fn asked_for(&self, project: Option<u64>) -> bool {
        self.asked && self.project == project
    }
}

fn saved_text(editing: bool, routine: bool) -> &'static str {
    match (editing, routine) {
        (true, true) => "Routine saved",
        (true, false) => "Queue entry saved",
        (false, true) => "Routine scheduled",
        (false, false) => "Scheduled",
    }
}

impl QueuePanel {
    pub(super) fn open_form(&mut self, entry: Option<&Entry>, window: &mut Window, cx: &mut Context<Self>) {
        if self.config.is_none() {
            Self::load_config(cx);
        }
        let store = Store::get(cx);
        let session = store.read(cx).session.clone();
        let form = Form::new(entry, session.as_ref(), window, cx);
        let focus = form.prompt.read(cx).focus_handle(cx);
        self.form = Some(form);
        self.flows = FlowChoices::default();
        window.focus(&focus);
        cx.notify();
    }

    fn load_config(cx: &mut Context<Self>) {
        store::run_json(cx, args(&["config", "--json"]), |this: &mut Self, config, cx| {
            if let Ok(config) = config {
                this.config = Some(config);
                cx.notify();
            }
        });
    }

    pub(super) fn sync_accounts(&mut self, cx: &mut Context<Self>) {
        let choices = Store::get(cx).read(cx).account_choices();
        if let Some(form) = self.form.as_mut() {
            form.accounts.follow(choices, |this, cx| this.update_form(|_| {}, cx), cx);
        }
    }

    pub(super) fn sync_flows(&mut self, cx: &mut Context<Self>) {
        let Some(project) = self.form.as_ref().filter(|f| f.repeats()).map(|f| f.project) else { return };
        if self.flows.asked_for(project) {
            return;
        }
        self.flows = FlowChoices { asked: true, project, names: None };
        let mut command = args(&["flow", "list", "--json"]);
        command.extend(project_args(project));
        store::run(cx, command, move |this: &mut Self, result, cx| this.flows_listed(project, result, cx));
    }

    fn flows_listed(&mut self, project: Option<u64>, result: anyhow::Result<String>, cx: &mut Context<Self>) {
        if !self.flows.asked_for(project) {
            return;
        }
        let names = result.and_then(|out| parse_listing(&out)).map(|listing| flow_names(&listing.flows));
        self.flows.names = Some(names.unwrap_or_else(|e| {
            toast_error(&e, cx);
            Vec::new()
        }));
        cx.notify();
    }

    pub(super) fn close_form(&mut self, cx: &mut Context<Self>) {
        self.form = None;
        cx.notify();
    }

    pub(super) fn save(&mut self, cx: &mut Context<Self>) {
        let Some(draft) = self.form.as_ref().map(|f| f.draft(cx)) else { return };
        let Some(form) = self.form.as_mut().filter(|f| !f.busy) else { return };
        let command = match draft.args() {
            Ok(command) => command,
            Err(e) => {
                form.error = Some(e);
                cx.notify();
                return;
            }
        };
        form.busy = true;
        form.error = None;
        let text = saved_text(draft.edit.is_some(), form.repeats());
        store::run_json(cx, command, move |this: &mut Self, answer, cx| match answer {
            Ok(_) => this.saved(text, cx),
            Err(e) => this.save_failed(cli::reason(&e), cx),
        });
        cx.notify();
    }

    fn saved(&mut self, text: &'static str, cx: &mut Context<Self>) {
        self.form = None;
        Store::get(cx).update(cx, |s, cx| s.toast(text, ToastKind::Done, cx));
        cx.notify();
    }

    fn save_failed(&mut self, error: String, cx: &mut Context<Self>) {
        if let Some(form) = self.form.as_mut() {
            form.busy = false;
            form.error = Some(error);
        }
        cx.notify();
    }

    pub(super) fn update_form(&mut self, change: impl Fn(&mut Form), cx: &mut Context<Self>) {
        if let Some(form) = self.form.as_mut() {
            change(form);
            form.error = None;
            cx.notify();
        }
    }
}
