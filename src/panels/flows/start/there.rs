use gpui::{App, AppContext as _, Context, Div, Entity, ParentElement as _, div};

use super::StartFlow;
use super::plan::{IssueTask, issue_hints};
use crate::cli;
use crate::dialogs::{RunOn, RunOnEvent, agent_line, follow_field};
use crate::model::Spot;
use crate::model::machines::run_on::checks::Wanted;
use crate::model::machines::run_on::{Starting, There};
use crate::panels::flows::model;
use crate::store::{StartThere, Store};
use crate::ui::account_picker::AccountPickers;

const WATCHED_THERE: &str = "A machine's run is watched through its tab: the run page reads this computer's runs.";
const NOT_LISTED: &str = "The flows there show once the machine has this project.";

pub(super) fn run_on_of(issue: Option<&IssueTask>, project: Option<u64>, cx: &mut Context<StartFlow>) -> Entity<RunOn> {
    let tracker = issue.map(|issue| issue.source.clone());
    let issue = issue.map(|issue| issue.key.clone());
    let wanted = Wanted { accounts: true, accounts_hold: true, tracker, flows: true, modes: false, issue };
    let run_on = cx.new(|cx| RunOn::new(project, wanted, cx));
    cx.observe(&run_on, |this, _, cx| {
        this.follow_there(cx);
        cx.notify();
    })
    .detach();
    cx.subscribe(&run_on, |this, _, event, cx| this.run_on_event(event, cx)).detach();
    run_on
}

impl StartFlow {
    pub(super) fn on_machine(&self, cx: &App) -> bool {
        self.run_on.read(cx).machine().is_some()
    }

    fn run_on_event(&mut self, event: &RunOnEvent, cx: &mut Context<Self>) {
        match event {
            RunOnEvent::Chosen => self.machine_chosen(cx),
            RunOnEvent::Flows(answer) => self.flows_there(answer),
        }
        cx.notify();
    }

    fn machine_chosen(&mut self, cx: &mut Context<Self>) {
        self.accounts = AccountPickers::default();
        self.error = None;
        self.keep_flow = self.selected().map(|flow| flow.name.clone()).or(self.keep_flow.take());
        self.flows.clear();
        self.selected = None;
        if !self.on_machine(cx) {
            self.load(cx);
        }
        self.follow_there(cx);
    }

    pub(super) fn shown_issue(&self, cx: &App) -> Option<IssueTask> {
        let issue = self.issue.as_ref()?;
        let run_on = self.run_on.read(cx);
        Some(match run_on.machine() {
            Some(_) => issue.there(run_on.issue_there()),
            None => issue.clone(),
        })
    }

    pub(super) fn follow_there(&mut self, cx: &mut Context<Self>) {
        let Some(shown) = self.shown_issue(cx) else { return };
        let (task_hint, branch_hint) = issue_hints(self.run_on.read(cx).machine());
        let was = &self.prefill;
        let task = follow_field(&self.fields.prompt, task_hint, (&was.0, shown.task()), cx);
        let branch = follow_field(&self.fields.branch, branch_hint, (&was.1, shown.branch.clone()), cx);
        self.prefill = (task, branch);
    }

    fn flows_there(&mut self, answer: &Result<String, String>) {
        let Ok(out) = answer else { return };
        match model::parse_listing(out) {
            Ok(listing) => self.reselect(listing.flows),
            Err(e) => self.error = Some(cli::reason(&e)),
        }
    }

    pub(super) fn submit_there(&mut self, spot: Spot, cx: &mut Context<Self>) {
        let plan = match self.flow_plan(cx) {
            Ok(plan) => plan,
            Err(error) => {
                self.error = Some(error);
                cx.notify();
                return;
            }
        };
        self.error = None;
        let starting = Starting::Flow {
            name: plan.flow.clone(),
            close_when_merged: plan.close_when_merged,
            free_builds: plan.free_builds,
        };
        let command = plan.args_there(&spot);
        let start = StartThere { spot, starting, command, target: cli::target(), select: self.ticks.focus };
        Store::get(cx).update(cx, |s, cx| s.start_there(start, cx));
    }

    pub(super) fn run_on_rows(&self, cx: &App) -> Vec<Div> {
        let run_on = self.run_on.read(cx);
        let row = run_on.offered(cx).then(|| div().child(self.run_on.clone()));
        row.into_iter().chain(run_on.where_line("In its own worktree", cx)).collect()
    }

    pub(super) fn agent_note(&self, cx: &App) -> Option<Div> {
        let team = self.team();
        agent_line(&self.run_on, &team, &self.accounts.picks(&team, cx), cx)
    }

    pub(super) fn no_flows(&self, cx: &App) -> &'static str {
        match self.run_on.read(cx).there(cx) {
            None => "No flows to start here.",
            Some(There::Ready(_)) => "No flows to start there.",
            Some(_) => NOT_LISTED,
        }
    }

    pub(super) fn footnote_there(&self, note: String, cx: &App) -> String {
        if self.on_machine(cx) { format!("{note} {WATCHED_THERE}") } else { note }
    }
}
