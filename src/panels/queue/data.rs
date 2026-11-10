use std::cmp::Reverse;
use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

use super::repeat;
use crate::cli::Version;
use crate::model::agent_settings::Agents;
use crate::model::{Project, Session};
use crate::panels::flows::model::FlowInfo;
use crate::platform::Os;
use crate::platform::paths::Style;

pub(super) const SOURCES: [&str; 7] = ["github", "gitlab", "shortcut", "linear", "jira", "plane", "asana"];
pub(super) const ROUTINES_NOTE: &str = "A routine only runs while cornercase's server runs. Each run gets a fresh \
                                        worktree from the up-to-date default branch, so only git repositories can \
                                        have routines.";
const RUNS_SHOWN: usize = 10;

const fn machine(os: Os) -> &'static str {
    match os {
        Os::Mac => "the Mac",
        Os::Linux => "the computer",
        Os::Windows => "the PC",
    }
}

pub(super) fn missed(os: Os) -> String {
    format!(
        "It was due while cornercase was stopped or {} slept. Start it now, or edit it for another time.",
        machine(os)
    )
}

pub(super) fn missed_run(os: Os) -> String {
    format!("It was due while cornercase was stopped or {} slept. The routine goes on with its next time.", machine(os))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum State {
    Pending,
    Starting,
    Started,
    Failed,
    Missed,
    Removed,
    Kept,
    Repeating,
    Paused,
    Other,
}

impl State {
    pub(super) fn of(entry: &Entry) -> Self {
        if entry.is_routine() {
            return if entry.paused { Self::Paused } else { Self::Repeating };
        }
        match entry.state.as_str() {
            "pending" => Self::Pending,
            "starting" => Self::Starting,
            "started" => Self::Started,
            "failed" => Self::Failed,
            "missed" => Self::Missed,
            "removed" => Self::Removed,
            "kept" => Self::Kept,
            _ => Self::Other,
        }
    }

    pub(super) fn group(self) -> Group {
        match self {
            Self::Failed | Self::Missed => Group::Unstarted,
            Self::Pending | Self::Starting => Group::Scheduled,
            Self::Repeating | Self::Paused => Group::Routines,
            Self::Started | Self::Removed | Self::Kept => Group::Started,
            Self::Other => Group::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Group {
    Unstarted,
    Scheduled,
    Routines,
    Started,
    Other,
}

impl Group {
    const ALL: [Group; 5] = [Group::Unstarted, Group::Scheduled, Group::Routines, Group::Started, Group::Other];

    pub(super) fn title(self) -> &'static str {
        match self {
            Self::Unstarted => "Did not start",
            Self::Scheduled => "Scheduled",
            Self::Routines => "Routines",
            Self::Started => "Started",
            Self::Other => "Other",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Body {
    Loading,
    Empty,
    NoMatch,
    List,
}

impl Body {
    pub(super) fn of(loaded: bool, entries: usize, shown: usize) -> Self {
        match (loaded, entries, shown) {
            (false, ..) => Self::Loading,
            (true, 0, _) => Self::Empty,
            (true, _, 0) => Self::NoMatch,
            _ => Self::List,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Click {
    Edit,
    Open(u64),
    Nothing,
}

impl Click {
    pub(super) fn of(entry: &Entry, pane: Option<u64>, busy: bool) -> Self {
        match pane {
            Some(pane) => Self::Open(pane),
            None if entry.runnable() && !busy => Self::Edit,
            None => Self::Nothing,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Place {
    pub project: String,
    pub workspace: Option<String>,
    pub workspace_name: Option<String>,
    pub workspace_id: Option<u64>,
    pub branch: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Entry {
    pub id: u64,
    pub at: i64,
    pub label: String,
    pub agent: String,
    pub mode: Option<String>,
    pub prompt: String,
    pub place: Place,
    pub state: String,
    pub pane: Option<u64>,
    pub server: Option<String>,
    pub reason: Option<String>,
    pub accounts: BTreeMap<String, String>,
    pub repeat: Option<String>,
    pub flow: Option<String>,
    pub cleanup: bool,
    pub paused: bool,
    pub next: Option<i64>,
    pub routine: Option<u64>,
}

impl Entry {
    pub(crate) fn runnable(&self) -> bool {
        self.routine.is_none() && matches!(self.state.as_str(), "pending" | "failed" | "missed")
    }

    pub(super) fn is_routine(&self) -> bool {
        self.repeat.is_some()
    }

    pub(super) fn clearable(&self) -> bool {
        match State::of(self) {
            State::Started => true,
            State::Removed | State::Kept | State::Failed | State::Missed => self.routine.is_some(),
            _ => false,
        }
    }

    pub(super) fn next_time(&self) -> i64 {
        self.next.unwrap_or(self.at)
    }

    pub(super) fn who(&self) -> String {
        self.flow.as_ref().map_or_else(|| self.agent.clone(), |flow| format!("flow {flow}"))
    }

    pub(super) fn first_prompt_line(&self) -> String {
        self.prompt.lines().next().unwrap_or_default().to_string()
    }

    pub(super) fn meta(&self, session: Option<&Session>) -> String {
        let mut meta = vec![self.who()];
        if let Some(mode) = self.mode.as_ref().filter(|m| *m != "default") {
            meta.push(mode.clone());
        }
        meta.push(place_label(&self.place, session));
        meta.extend(
            self.accounts
                .iter()
                .map(|(agent, account)| format!("{agent}: {}", if account.is_empty() { "System" } else { account })),
        );
        if self.is_routine() && self.cleanup {
            meta.push("removes its workspace".into());
        }
        meta.join(" · ")
    }

    pub(super) fn matches(&self, filter: &str, session: Option<&Session>) -> bool {
        if filter.is_empty() {
            return true;
        }
        let place = place_label(&self.place, session);
        let flow = self.flow.clone().unwrap_or_default();
        [&self.label, &self.prompt, &self.agent, &flow, &place].iter().any(|t| t.to_lowercase().contains(filter))
    }
}

pub(crate) fn entries(app: Option<&Value>) -> Vec<Entry> {
    app.and_then(|app| app.get("queue"))
        .and_then(Value::as_array)
        .map(|queue| queue.iter().filter_map(|e| Entry::deserialize(e).ok()).filter(|e| e.id != 0).collect())
        .unwrap_or_default()
}

pub(super) fn folder_name(path: &str) -> String {
    Style::of_path(path).name(path).to_string()
}

pub(crate) fn place_label(place: &Place, session: Option<&Session>) -> String {
    let project = session
        .and_then(|s| s.projects.iter().find(|p| p.path == place.project))
        .map_or_else(|| folder_name(&place.project), |p| p.name.clone());
    match (&place.branch, &place.workspace_name, &place.workspace) {
        (Some(branch), ..) => format!("{project} › {branch}"),
        (None, Some(name), _) => format!("{project} › {name}"),
        (None, None, Some(path)) if *path != place.project => format!("{project} › {}", folder_name(path)),
        _ => project,
    }
}

#[derive(Debug)]
pub(super) struct Row<'a> {
    pub(super) entry: &'a Entry,
    pub(super) runs: Vec<&'a Entry>,
}

impl Row<'_> {
    pub(super) fn removable(&self) -> bool {
        State::of(self.entry) != State::Starting && !self.runs.iter().any(|run| State::of(run) == State::Starting)
    }
}

#[derive(Debug)]
pub(super) struct Section<'a> {
    pub(super) group: Group,
    pub(super) rows: Vec<Row<'a>>,
}

fn runs_of(entries: &[Entry], routine: u64) -> Vec<&Entry> {
    let mut runs: Vec<&Entry> = entries.iter().filter(|e| e.routine == Some(routine)).collect();
    runs.sort_by_key(|run| Reverse((run.at, run.id)));
    runs.truncate(RUNS_SHOWN);
    runs
}

fn rows<'a>(entries: &'a [Entry], filter: &str, session: Option<&Session>) -> Vec<Row<'a>> {
    let nested = |e: &Entry| e.routine.is_some_and(|id| entries.iter().any(|r| r.id == id && r.is_routine()));
    entries
        .iter()
        .filter(|e| !nested(e))
        .map(|entry| Row { entry, runs: runs_of(entries, entry.id) })
        .filter(|row| row.entry.matches(filter, session) || row.runs.iter().any(|run| run.matches(filter, session)))
        .collect()
}

pub(super) fn sections<'a>(entries: &'a [Entry], filter: &str, session: Option<&Session>) -> Vec<Section<'a>> {
    let mut rows = rows(entries, filter, session);
    let says_nothing = filter.is_empty() && !rows.iter().any(|row| row.entry.is_routine());
    Group::ALL
        .into_iter()
        .map(|group| {
            let (of_group, rest) = rows.drain(..).partition(|row| State::of(row.entry).group() == group);
            rows = rest;
            Section { group, rows: of_group }
        })
        .filter(|section| !section.rows.is_empty() || (section.group == Group::Scheduled && says_nothing))
        .collect()
}

pub(super) fn clear_goes_in(sections: &[Section]) -> Option<Group> {
    if sections.iter().any(|section| section.group == Group::Started) {
        return Some(Group::Started);
    }
    let finished = sections.iter().flat_map(|s| &s.rows).flat_map(|row| &row.runs).any(|run| run.clearable());
    finished.then_some(Group::Routines)
}

pub(super) fn is_repository(project: &Project) -> bool {
    project.workspaces.iter().any(|w| w.worktree || (w.path == project.path && w.branch.is_some()))
}

pub(super) fn repository_for(project: Option<u64>, session: &Session) -> Option<u64> {
    let mut repositories = session.projects.iter().filter(|p| is_repository(p)).map(|p| p.id);
    project.filter(|id| repositories.clone().any(|root| root == *id)).or_else(|| repositories.next())
}

pub(super) fn flow_names(flows: &[FlowInfo]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for flow in flows.iter().filter(|flow| flow.can_start()) {
        if !names.contains(&flow.name) {
            names.push(flow.name.clone());
        }
    }
    names
}

pub(crate) fn live_pane(entry: &Entry, boot: Option<&str>, session: Option<&Session>) -> Option<u64> {
    let pane = entry.pane.filter(|_| matches!(State::of(entry), State::Started | State::Kept))?;
    let this_run = boot.is_some() && entry.server.as_deref() == boot;
    (this_run && session.and_then(|s| s.pane(pane)).is_some()).then_some(pane)
}

pub(crate) fn status_label(entry: &Entry, pane: Option<u64>) -> String {
    match State::of(entry) {
        State::Repeating | State::Paused => format!("↻ {}", repeat::words(entry.repeat.as_deref().unwrap_or_default())),
        State::Pending => "Pending".into(),
        State::Starting => "Starting…".into(),
        State::Started => pane.map_or_else(|| "Started · pane closed".into(), |pane| format!("Started · pane {pane}")),
        State::Failed => "Failed".into(),
        State::Missed => "Missed".into(),
        State::Removed => "Workspace removed".into(),
        State::Kept => pane.map_or_else(|| "Workspace kept".into(), |pane| format!("Workspace kept · pane {pane}")),
        State::Other => entry.state.clone(),
    }
}

pub(super) fn notice(entry: &Entry) -> Option<String> {
    match State::of(entry) {
        State::Failed | State::Kept => entry.reason.as_deref().map(str::trim).filter(|r| !r.is_empty()).map(sentence),
        State::Missed if entry.routine.is_some() => Some(missed_run(Os::CURRENT)),
        State::Missed => Some(missed(Os::CURRENT)),
        _ => None,
    }
}

pub(super) fn remove_tip(entry: &Entry) -> &'static str {
    match (entry.is_routine(), entry.routine.is_some(), State::of(entry)) {
        (true, ..) => "Remove the routine and its runs: agents it started keep running, in their workspaces",
        (_, true, _) => "Remove this run from the list: its agent and its workspace stay",
        (.., State::Started) => "Remove it from the list: its agent keeps running",
        _ => "Remove",
    }
}

pub(super) fn sentence(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| first.to_uppercase().chain(chars).collect())
}

pub(crate) fn agent_choices(config: &Value) -> (Vec<String>, String) {
    let setting = setting(config, "agent");
    let choices = setting.map(choice_values).unwrap_or_default();
    let default = setting.and_then(|s| s.get("value")).and_then(Value::as_str).unwrap_or("auto").to_string();
    (choices, default)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ModeChip {
    pub value: String,
    pub label: String,
    pub rests: bool,
}

pub(super) fn mode_chips(config: &Value, agent: &str, server: Option<Version>) -> (Vec<ModeChip>, Option<String>) {
    let agents = Agents::from_config(config);
    let Some(modes) = agents.modes_of(agent).cloned().map(|modes| modes.on_server(server)) else {
        return (Vec::new(), None);
    };
    let chips = modes
        .choices
        .iter()
        .map(|choice| {
            let label =
                if choice.value == modes.chosen { format!("{} ✓", choice.value) } else { choice.value.clone() };
            ModeChip { value: choice.value.clone(), label, rests: modes.rests(&choice.value) }
        })
        .collect();
    (chips, modes.waiting(None))
}

fn setting<'a>(config: &'a Value, key: &str) -> Option<&'a Value> {
    config.get("settings")?.as_array()?.iter().find(|s| s.get("key").and_then(Value::as_str) == Some(key))
}

fn choice_values(setting: &Value) -> Vec<String> {
    setting
        .get("choices")
        .and_then(Value::as_array)
        .map(|c| c.iter().filter_map(|c| c.get("value").and_then(Value::as_str)).map(str::to_string).collect())
        .unwrap_or_default()
}
