use serde::Deserialize;

use super::model::{FlowInfo, Run};
use crate::cli::Version;
use crate::store;

pub(crate) const CLOSE_WHEN_MERGED_SINCE: Version = Version::new(0, 13, 33);
pub(crate) const FREE_BUILDS_SINCE: Version = Version::new(0, 13, 64);
pub(super) const LABEL: &str = "Close the agents once the pull request merges";
pub(super) const FREE_LABEL: &str = "Free the build folders too";
pub(super) const FREE_HOW: &str = "Deletes the git-ignored build folders, such as target and node_modules, of the run's worktree and of its roles' own once the agents have closed. The worktrees, their branches and the run's folder stay, and reopening the agents rebuilds what they need. A worktree where a program still runs keeps its builds. Turning the close off turns this off too.";
const WHEN: &str = "When none of them has worked for 3 minutes after the merge.";

pub(super) fn how(machine: Option<&str>, free: bool) -> String {
    let kept = if free { "" } else { " Nothing is removed:" };
    match machine {
        Some(machine) => format!("{WHEN}{kept} cornercase on {machine} can reopen them."),
        None => format!("{WHEN}{kept} Reopen agents, on the run's page or in the workspace's menu, brings them back."),
    }
}

pub(super) struct Releases<'a> {
    pub here: Option<Version>,
    pub machine: Option<(&'a str, Option<Version>)>,
}

pub(super) fn rests(flow: Option<&FlowInfo>, releases: &Releases<'_>) -> Option<String> {
    leaves_commits(flow).or_else(|| too_old(CLOSE_WHEN_MERGED_SINCE, releases))
}

fn leaves_commits(flow: Option<&FlowInfo>) -> Option<String> {
    flow.filter(|f| f.flow.ends_in_commits())
        .map(|f| format!("{} leaves the commits, so there is no pull request to wait for.", f.name))
}

fn too_old(since: Version, releases: &Releases<'_>) -> Option<String> {
    if !since.reached_by(releases.here) {
        return Some(needs(since, None, releases.here));
    }
    releases
        .machine
        .filter(|(_, release)| !since.reached_by(*release))
        .map(|(machine, release)| needs(since, Some(machine), release))
}

fn needs(since: Version, machine: Option<&str>, release: Option<Version>) -> String {
    let place = machine.map(|machine| format!(" on {machine}")).unwrap_or_default();
    let who = if machine.is_some() { "it" } else { "the server" };
    let runs = release.map(|release| format!(": {who} runs {release}")).unwrap_or_default();
    format!("Needs cornercase {since} or newer{place}{runs}.")
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
    match too_old(CLOSE_WHEN_MERGED_SINCE, &Releases { here, machine: None }) {
        Some(line) => Switch::Rests(line),
        None => Switch::Offered(run.close_when_merged),
    }
}

pub(super) fn free_switch(run: &Run, here: Option<Version>) -> Switch {
    match switch(run, here) {
        Switch::Offered(close) => free_tail(&Releases { here, machine: None }, close, run.free_builds),
        _ => Switch::Hidden,
    }
}

pub(super) fn free_box(flow: Option<&FlowInfo>, releases: &Releases<'_>, close: bool, free: bool) -> Switch {
    match rests(flow, releases) {
        Some(_) => Switch::Hidden,
        None => free_tail(releases, close, free),
    }
}

fn free_tail(releases: &Releases<'_>, close: bool, free: bool) -> Switch {
    if let Some(line) = too_old(FREE_BUILDS_SINCE, releases) {
        return Switch::Rests(line);
    }
    if close { Switch::Offered(free) } else { Switch::Rests(FREE_HOW.to_string()) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Choice {
    pub close: bool,
    pub free: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Press {
    Close,
    Free,
}

impl Choice {
    pub(super) fn of(run: &Run, here: Option<Version>) -> Self {
        Self { close: run.close_when_merged, free: run.free_builds && FREE_BUILDS_SINCE.reached_by(here) }
    }

    pub(super) fn pressed(self, press: Press) -> Self {
        match press {
            Press::Close if self.close => Self { close: false, free: false },
            Press::Close => Self { close: true, ..self },
            Press::Free => Self { close: true, free: !self.free },
        }
    }
}

pub(super) fn key(workspace: u64) -> String {
    format!("flow:close-when-merged:{workspace}")
}

pub(super) fn free_key(workspace: u64) -> String {
    format!("flow:free-builds:{workspace}")
}

pub(super) fn switch_args(workspace: u64, choice: Choice) -> Vec<String> {
    let mut command = store::args(&["flow", "close-when-merged", "--workspace", &workspace.to_string(), "--json"]);
    if !choice.close {
        command.push("--off".into());
    } else if choice.free {
        command.push("--free-builds".into());
    }
    command
}

pub(super) fn doing(flow: &str, press: Press, to: Choice) -> String {
    match (press, to.close, to.free) {
        (Press::Close, true, _) => format!("Setting {flow} to close its agents once merged…"),
        (Press::Close, false, _) => format!("Setting {flow} to keep its agents open…"),
        (Press::Free, _, true) => format!("Setting {flow} to free its build folders once merged…"),
        (Press::Free, _, false) => format!("Setting {flow} to keep its build folders…"),
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(super) struct Set {
    pub run: u64,
    pub close_when_merged: bool,
    pub free_builds: bool,
}

pub(super) fn parse_set(out: &str) -> anyhow::Result<Set> {
    Ok(serde_json::from_str(out.trim())?)
}

pub(super) fn take(runs: &mut [Run], set: &Set) {
    for run in runs.iter_mut().filter(|run| run.id == set.run) {
        run.close_when_merged = set.close_when_merged;
        run.free_builds = set.free_builds;
    }
}

pub(crate) fn closes_clause(close: bool, free: bool) -> &'static str {
    match (close, free) {
        (true, true) => ", and its agents close and their build folders are freed once the pull request merges",
        (true, false) => ", and its agents close once the pull request merges",
        (false, _) => "",
    }
}

#[cfg(test)]
mod tests;
