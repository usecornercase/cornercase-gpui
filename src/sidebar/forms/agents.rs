use std::fmt::Write as _;

use gpui::{App, AppContext as _};
use serde::Deserialize;

use crate::cli;
use crate::dialogs::{self, Question};
use crate::store::{Busy, Store, ToastKind, args};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct Plan {
    pub(super) agents: Vec<Closing>,
    pub(super) stops: Vec<Stop>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct Closing {
    pub(super) agent: Option<String>,
    pub(super) pane: u64,
    pub(super) resumes: bool,
    pub(super) status: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct Stop {
    pub(super) pane: u64,
    pub(super) tab: u64,
}

impl Closing {
    fn cut(&self) -> bool {
        matches!(self.status.as_deref(), Some("working" | "waiting"))
    }

    fn line(&self) -> String {
        let agent = self.agent.as_deref().unwrap_or("an agent");
        let mut line = format!("{agent} in pane {}", self.pane);
        if self.cut() {
            line.push_str(", its turn is cut");
        }
        if !self.resumes {
            line.push_str(", starts a new conversation when reopened");
        }
        line
    }
}

impl Plan {
    pub(super) fn loses_something(&self) -> bool {
        !self.stops.is_empty() || self.agents.iter().any(|a| a.cut() || !a.resumes)
    }

    pub(super) fn message(&self) -> String {
        let lines: Vec<String> = self.agents.iter().map(Closing::line).collect();
        let mut text =
            format!("Their tabs close, and each can be reopened later in its own conversation: {}.", lines.join("; "));
        if !self.stops.is_empty() {
            let panes: Vec<String> = self.stops.iter().map(|s| format!("pane {} in tab {}", s.pane, s.tab)).collect();
            let _ = write!(text, " Also stops {}.", panes.join(", "));
        }
        text
    }

    pub(super) fn command(&self, workspace: u64) -> Vec<String> {
        let mut command = args(&["agents", "close", "--workspace", &workspace.to_string()]);
        if self.loses_something() {
            command.push("--force".into());
        }
        command
    }
}

fn toast(text: String, cx: &mut App) {
    Store::get(cx).update(cx, |s, cx| s.toast(text, ToastKind::Error, cx));
}

fn ask(workspace: u64, name: &str, plan: &Plan, cx: &mut App) {
    if plan.agents.is_empty() {
        toast(format!("No agent to close in {name}"), cx);
        return;
    }
    let button = if plan.loses_something() { "Close anyway" } else { "Close agents" };
    let busy = Busy::new(format!("close-agents:{workspace}"), format!("Closing the agents of {name}…"));
    let question =
        Question::new(format!("Close the agents of {name}?"), plan.message(), button, plan.command(workspace))
            .busy(busy);
    if let Some(window) = cx.active_window() {
        let _ = window.update(cx, |_, window, cx| dialogs::confirm(question, window, cx));
    }
}

pub(in crate::sidebar) fn close_agents(workspace: u64, name: String, cx: &mut App) {
    let check = args(&["agents", "close", "--workspace", &workspace.to_string(), "--check", "--json"]);
    let busy = Busy::new(format!("check-agents:{workspace}"), format!("Checking the agents of {name}…"));
    let job = Store::get(cx).update(cx, |s, cx| s.start_job(busy, cx));
    let task = cx.background_spawn(async move { cli::run(&check) });
    cx.spawn(async move |cx| {
        let answer = task.await;
        let _ = cx.update(|cx| Store::get(cx).update(cx, |s, cx| s.end_job(job, cx)));
        let _ = cx.update(|cx| match answer.and_then(|out| serde_json::from_str::<Plan>(&out).map_err(Into::into)) {
            Ok(plan) => ask(workspace, &name, &plan, cx),
            Err(e) => toast(e.to_string(), cx),
        });
    })
    .detach();
}

#[cfg(test)]
mod tests;
