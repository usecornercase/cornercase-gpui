use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

use crate::model::Session;

pub(super) const SOURCES: [&str; 7] = ["github", "gitlab", "shortcut", "linear", "jira", "plane", "asana"];

const MISSED: &str =
    "It was due while cornercase was stopped or the Mac slept. Start it now, or edit it for another time.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum State {
    Pending,
    Starting,
    Started,
    Failed,
    Missed,
    Other,
}

impl State {
    pub(super) fn of(entry: &Entry) -> Self {
        match entry.state.as_str() {
            "pending" => Self::Pending,
            "starting" => Self::Starting,
            "started" => Self::Started,
            "failed" => Self::Failed,
            "missed" => Self::Missed,
            _ => Self::Other,
        }
    }

    pub(super) fn group(self) -> Group {
        match self {
            Self::Failed | Self::Missed => Group::Unstarted,
            Self::Pending | Self::Starting => Group::Scheduled,
            Self::Started => Group::Started,
            Self::Other => Group::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Group {
    Unstarted,
    Scheduled,
    Started,
    Other,
}

impl Group {
    const ALL: [Group; 4] = [Group::Unstarted, Group::Scheduled, Group::Started, Group::Other];

    pub(super) fn title(self) -> &'static str {
        match self {
            Self::Unstarted => "Did not start",
            Self::Scheduled => "Scheduled",
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
}

impl Entry {
    pub(crate) fn runnable(&self) -> bool {
        matches!(self.state.as_str(), "pending" | "failed" | "missed")
    }

    pub(super) fn first_prompt_line(&self) -> String {
        self.prompt.lines().next().unwrap_or_default().to_string()
    }

    pub(super) fn meta(&self, session: Option<&Session>) -> String {
        let mut meta = vec![self.agent.clone()];
        if let Some(mode) = self.mode.as_ref().filter(|m| *m != "default") {
            meta.push(mode.clone());
        }
        meta.push(place_label(&self.place, session));
        meta.extend(
            self.accounts
                .iter()
                .map(|(agent, account)| format!("{agent}: {}", if account.is_empty() { "System" } else { account })),
        );
        meta.join(" · ")
    }

    pub(super) fn matches(&self, filter: &str, session: Option<&Session>) -> bool {
        if filter.is_empty() {
            return true;
        }
        let place = place_label(&self.place, session);
        [&self.label, &self.prompt, &self.agent, &place].iter().any(|t| t.to_lowercase().contains(filter))
    }
}

pub(crate) fn entries(app: Option<&Value>) -> Vec<Entry> {
    app.and_then(|app| app.get("queue"))
        .and_then(Value::as_array)
        .map(|queue| queue.iter().filter_map(|e| Entry::deserialize(e).ok()).filter(|e| e.id != 0).collect())
        .unwrap_or_default()
}

pub(super) fn folder_name(path: &str) -> String {
    path.trim_end_matches('/').rsplit('/').next().unwrap_or(path).to_string()
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

pub(super) fn sections<'a>(
    entries: &'a [Entry],
    filter: &str,
    session: Option<&Session>,
) -> Vec<(Group, Vec<&'a Entry>)> {
    let shown: Vec<&Entry> = entries.iter().filter(|e| e.matches(filter, session)).collect();
    Group::ALL
        .into_iter()
        .map(|group| (group, shown.iter().copied().filter(|e| State::of(e).group() == group).collect::<Vec<_>>()))
        .filter(|(group, list)| !list.is_empty() || (*group == Group::Scheduled && filter.is_empty()))
        .collect()
}

pub(crate) fn live_pane(entry: &Entry, boot: Option<&str>, session: Option<&Session>) -> Option<u64> {
    let pane = entry.pane.filter(|_| State::of(entry) == State::Started)?;
    let this_run = boot.is_some() && entry.server.as_deref() == boot;
    (this_run && session.and_then(|s| s.pane(pane)).is_some()).then_some(pane)
}

pub(crate) fn status_label(entry: &Entry, pane: Option<u64>) -> String {
    match State::of(entry) {
        State::Pending => "Pending".into(),
        State::Starting => "Starting…".into(),
        State::Started => pane.map_or_else(|| "Started · pane closed".into(), |pane| format!("Started · pane {pane}")),
        State::Failed => "Failed".into(),
        State::Missed => "Missed".into(),
        State::Other => entry.state.clone(),
    }
}

pub(super) fn notice(entry: &Entry) -> Option<String> {
    match State::of(entry) {
        State::Failed => entry.reason.as_deref().map(str::trim).filter(|r| !r.is_empty()).map(sentence),
        State::Missed => Some(MISSED.into()),
        _ => None,
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

pub(crate) fn mode_choices(config: &Value, agent: &str) -> (Vec<String>, Option<String>) {
    let setting = setting(config, &format!("agents.{agent}.mode"));
    let choices = setting.map(choice_values).unwrap_or_default();
    let chosen = setting.and_then(|s| s.get("value")).and_then(Value::as_str).map(str::to_string);
    (choices, chosen)
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
