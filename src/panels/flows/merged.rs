use serde::Deserialize;

use super::model::{FlowInfo, Run};
use crate::cli::Version;
use crate::store;

pub(crate) const CLOSE_WHEN_MERGED_SINCE: Version = Version::new(0, 13, 33);
pub(super) const LABEL: &str = "Close the agents once the pull request merges";
const HOW: &str = "When none of them has worked for 3 minutes after the merge. Nothing is removed: Reopen agents, on the run's page or in the workspace's menu, brings them back.";

pub(super) fn how(machine: Option<&str>) -> String {
    match machine {
        Some(machine) => format!(
            "When none of them has worked for 3 minutes after the merge. Nothing is removed: cornercase on {machine} can reopen them."
        ),
        None => HOW.to_string(),
    }
}

pub(super) struct Releases<'a> {
    pub here: Option<Version>,
    pub machine: Option<(&'a str, Option<Version>)>,
}

pub(super) fn rests(flow: Option<&FlowInfo>, releases: &Releases<'_>) -> Option<String> {
    leaves_commits(flow).or_else(|| too_old(releases))
}

fn leaves_commits(flow: Option<&FlowInfo>) -> Option<String> {
    flow.filter(|f| f.flow.ends_in_commits())
        .map(|f| format!("{} leaves the commits, so there is no pull request to wait for.", f.name))
}

fn too_old(releases: &Releases<'_>) -> Option<String> {
    if !CLOSE_WHEN_MERGED_SINCE.reached_by(releases.here) {
        return Some(needs(None, releases.here));
    }
    releases
        .machine
        .filter(|(_, release)| !CLOSE_WHEN_MERGED_SINCE.reached_by(*release))
        .map(|(machine, release)| needs(Some(machine), release))
}

fn needs(machine: Option<&str>, release: Option<Version>) -> String {
    let place = machine.map(|machine| format!(" on {machine}")).unwrap_or_default();
    let who = if machine.is_some() { "it" } else { "the server" };
    let runs = release.map(|release| format!(": {who} runs {release}")).unwrap_or_default();
    format!("Needs cornercase {CLOSE_WHEN_MERGED_SINCE} or newer{place}{runs}.")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Switch {
    Hidden,
    Rests(String),
    Offered(bool),
}

pub(super) fn switch(run: &Run, here: Option<Version>) -> Switch {
    if run.closed() || run.flow.ends_in_commits() {
        return Switch::Hidden;
    }
    match too_old(&Releases { here, machine: None }) {
        Some(line) => Switch::Rests(line),
        None => Switch::Offered(run.close_when_merged),
    }
}

pub(super) fn key(workspace: u64) -> String {
    format!("flow:close-when-merged:{workspace}")
}

pub(super) fn switch_args(workspace: u64, on: bool) -> Vec<String> {
    let mut command = store::args(&["flow", "close-when-merged", "--workspace", &workspace.to_string(), "--json"]);
    if !on {
        command.push("--off".into());
    }
    command
}

pub(super) fn doing(flow: &str, on: bool) -> String {
    if on {
        format!("Setting {flow} to close its agents once merged…")
    } else {
        format!("Setting {flow} to keep its agents open…")
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(super) struct Set {
    pub run: u64,
    pub close_when_merged: bool,
}

pub(super) fn parse_set(out: &str) -> anyhow::Result<Set> {
    Ok(serde_json::from_str(out.trim())?)
}

pub(super) fn take(runs: &mut [Run], set: &Set) {
    for run in runs.iter_mut().filter(|run| run.id == set.run) {
        run.close_when_merged = set.close_when_merged;
    }
}

pub(crate) fn closes_clause(answered: bool) -> &'static str {
    if answered { ", and its agents close once the pull request merges" } else { "" }
}

#[cfg(test)]
mod tests;
