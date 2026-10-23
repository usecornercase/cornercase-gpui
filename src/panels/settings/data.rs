use serde::Deserialize;
use serde_json::Value;

use crate::store::args;

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(super) struct Described {
    pub revision: u64,
    pub path: String,
    pub problem: Option<String>,
    pub settings: Vec<Setting>,
    pub connections: Vec<Connection>,
    pub projects: Vec<ProjectEntry>,
    pub notices: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum Kind {
    Switch,
    Number,
    #[default]
    Text,
    Choice,
    List,
    Key,
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(super) struct Choice {
    pub value: String,
    pub note: String,
    pub dangerous: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(super) struct Setting {
    pub key: String,
    pub page: String,
    pub section: String,
    pub label: String,
    #[serde(rename = "type")]
    pub kind: Kind,
    pub value: Value,
    pub default: Value,
    pub shown: String,
    pub note: String,
    pub choices: Vec<Choice>,
    pub field: Option<String>,
    pub question: Option<String>,
    pub dangerous: bool,
    pub restart: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum ConnectionKind {
    #[default]
    Tracker,
    Browsers,
    Bot,
    Chat,
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum State {
    #[default]
    Missing,
    Connected,
    Refused,
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(super) struct Connection {
    pub key: String,
    pub kind: ConnectionKind,
    pub page: String,
    pub section: String,
    pub label: String,
    pub note: String,
    pub state: State,
    pub account: Option<String>,
    pub from: Option<String>,
    pub env: Option<String>,
    pub count: Option<usize>,
    pub field: Option<String>,
    pub removable: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(super) struct ProjectEntry {
    pub folder: String,
    pub key: String,
    pub project: Option<u64>,
    pub repository: bool,
    pub setup: String,
    pub teardown: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(super) struct Exported {
    pub text: String,
    pub settings: usize,
    pub repositories: usize,
    pub left_out: Vec<String>,
    pub warnings: Vec<String>,
}

pub(super) fn parse(text: &str) -> anyhow::Result<Described> {
    Ok(serde_json::from_str(text)?)
}

pub(super) fn parse_export(text: &str) -> anyhow::Result<Exported> {
    Ok(serde_json::from_str(text)?)
}

pub(super) fn value_text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Array(items) => items.iter().map(value_text).collect::<Vec<_>>().join(","),
        other => other.to_string(),
    }
}

impl Setting {
    pub(super) fn text(&self) -> String {
        value_text(&self.value)
    }

    pub(super) fn is_default(&self) -> bool {
        self.value == self.default
    }

    pub(super) fn on(&self) -> bool {
        self.value.as_bool().unwrap_or(false)
    }

    pub(super) fn items(&self) -> Vec<String> {
        match &self.value {
            Value::Array(items) => items.iter().map(value_text).collect(),
            _ => Vec::new(),
        }
    }

    pub(super) fn switch_text(&self) -> &str {
        self.shown.trim_start_matches("[x]").trim_start_matches("[ ]").trim()
    }

    pub(super) fn edited_as_text(&self) -> bool {
        matches!(self.kind, Kind::Text | Kind::Number | Kind::Key | Kind::Other)
    }

    pub(super) fn as_segments(&self) -> bool {
        self.choices.len() <= 4 && self.choices.iter().all(|c| c.value.chars().count() <= 18 && !c.dangerous)
    }

    pub(super) fn choice(&self, value: &str) -> Option<&Choice> {
        self.choices.iter().find(|c| c.value == value)
    }
}

pub(super) fn set_args(key: &str, value: &str) -> Vec<String> {
    args(&["config", "set", "--json", "--", key, value])
}

pub(super) fn project_args(entry: &ProjectEntry, which: Script, text: &str) -> Vec<String> {
    let mut a = args(&["config", "project", "--json"]);
    if let Some(id) = entry.project {
        a.push("--project".into());
        a.push(id.to_string());
    }
    a.push(format!("--{}={text}", which.name()));
    if entry.project.is_none() {
        a.push("--".into());
        a.push(entry.folder.clone());
    }
    a
}

pub(super) fn clean_error(error: &anyhow::Error) -> String {
    let text = error.to_string();
    let text = match text.split_once(": ") {
        Some((head, rest)) if head.starts_with("cornercase ") => rest.to_string(),
        _ => text,
    };
    let text = text.strip_prefix("Error: ").or_else(|| text.strip_prefix("error: ")).unwrap_or(&text);
    text.trim().to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Script {
    Setup,
    Teardown,
}

impl Script {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Setup => "setup",
            Self::Teardown => "teardown",
        }
    }

    pub(super) fn of(self, entry: &ProjectEntry) -> &str {
        match self {
            Self::Setup => &entry.setup,
            Self::Teardown => &entry.teardown,
        }
    }
}

pub(super) fn is_number(text: &str) -> bool {
    !text.trim().is_empty() && text.trim().chars().all(|c| c.is_ascii_digit())
}

pub(super) fn list_toggle(items: &[String], item: &str) -> Vec<String> {
    if items.iter().any(|i| i == item) {
        items.iter().filter(|i| *i != item).cloned().collect()
    } else {
        let mut out = items.to_vec();
        out.push(item.to_string());
        out
    }
}

pub(super) fn list_move(items: &[String], index: usize, by: isize) -> Vec<String> {
    let mut out = items.to_vec();
    let Some(to) = index.checked_add_signed(by) else { return out };
    if index < out.len() && to < out.len() {
        out.swap(index, to);
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Tone {
    Good,
    Bad,
    Quiet,
}

impl Connection {
    pub(super) fn token_from_env(&self) -> bool {
        self.from.as_deref() == Some("env")
    }

    pub(super) fn takes_secret(&self) -> bool {
        matches!(self.kind, ConnectionKind::Tracker | ConnectionKind::Bot) && !self.token_from_env()
    }

    pub(super) fn status(&self) -> (String, Tone) {
        if self.kind == ConnectionKind::Browsers {
            return match self.count.unwrap_or(0) {
                0 => ("none paired".into(), Tone::Quiet),
                1 => ("1 paired".into(), Tone::Good),
                n => (format!("{n} paired"), Tone::Good),
            };
        }
        let source = match (self.token_from_env(), &self.env) {
            (true, Some(env)) => format!(" · from ${env}"),
            (true, None) => " · from the environment".into(),
            _ => String::new(),
        };
        match self.state {
            State::Connected => match &self.account {
                Some(account) if !account.is_empty() => (format!("connected as {account}{source}"), Tone::Good),
                _ => (format!("connected{source}"), Tone::Good),
            },
            State::Refused => (format!("refused{source}"), Tone::Bad),
            State::Missing => ("not set".into(), Tone::Quiet),
            State::Other => ("unknown".into(), Tone::Quiet),
        }
    }
}

pub(super) const PROJECTS: &str = "Projects";
pub(super) const SHARE: &str = "Share";

const ORDER: [&str; 5] = ["Worktrees", "Agents", "Issues", "UI", "Remote"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Item {
    Setting(usize),
    Connection(usize),
    Agent { kind: String, mode: Option<usize>, extra: Option<usize> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Section {
    pub title: String,
    pub items: Vec<Item>,
}

pub(super) fn pages(described: &Described) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let all = described.settings.iter().map(|s| &s.page).chain(described.connections.iter().map(|c| &c.page));
    for page in all {
        if !page.is_empty() && !found.contains(page) {
            found.push(page.clone());
        }
    }
    let rank = |page: &String| ORDER.iter().position(|p| p == page).unwrap_or(ORDER.len());
    found.sort_by_key(rank);
    found
}

pub(super) fn sections(described: &Described, page: &str) -> Vec<Section> {
    section_titles(described, page)
        .into_iter()
        .map(|title| Section { title: title.to_string(), items: section_items(described, page, title) })
        .collect()
}

fn section_titles<'d>(described: &'d Described, page: &str) -> Vec<&'d str> {
    let mut titles: Vec<&str> = Vec::new();
    let connections = described.connections.iter().filter(|c| c.page == page).map(|c| c.section.as_str());
    let settings = described.settings.iter().filter(|s| s.page == page).map(|s| s.section.as_str());
    for title in connections.chain(settings) {
        if !titles.contains(&title) {
            titles.push(title);
        }
    }
    titles.sort_by_key(|t| !t.is_empty());
    titles
}

fn section_items(described: &Described, page: &str, title: &str) -> Vec<Item> {
    let mut items = Vec::new();
    let settings = described.settings.iter().enumerate().filter(|(_, s)| s.page == page && s.section == title);
    for (i, setting) in settings {
        match agent_part(&setting.key) {
            Some((kind, part)) => place_agent_part(&mut items, kind, part, i),
            None => items.push(Item::Setting(i)),
        }
    }
    let connections = described.connections.iter().enumerate();
    items.extend(connections.filter(|(_, c)| c.page == page && c.section == title).map(|(i, _)| Item::Connection(i)));
    items
}

fn place_agent_part(items: &mut Vec<Item>, kind: &str, part: &str, index: usize) {
    let found = items.iter().position(|item| matches!(item, Item::Agent { kind: k, .. } if k == kind));
    let at = found.unwrap_or_else(|| {
        items.push(Item::Agent { kind: kind.to_string(), mode: None, extra: None });
        items.len() - 1
    });
    if let Item::Agent { mode, extra, .. } = &mut items[at] {
        let slot = if part == "mode" { mode } else { extra };
        *slot = Some(index);
    }
}

fn agent_part(key: &str) -> Option<(&str, &str)> {
    let rest = key.strip_prefix("agents.")?;
    let (kind, part) = rest.rsplit_once('.')?;
    matches!(part, "mode" | "extra").then_some((kind, part))
}

#[cfg(test)]
mod tests;
