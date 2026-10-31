use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum SourceState {
    Connected,
    Missing,
    Failing,
    #[serde(other)]
    #[default]
    Other,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum Who {
    #[default]
    Anyone,
    Me,
    Person {
        handle: String,
        #[serde(default)]
        id: Option<String>,
    },
}

impl Who {
    pub(super) fn arg(&self) -> String {
        match self {
            Self::Anyone => "anyone".into(),
            Self::Me => "me".into(),
            Self::Person { handle, id } => id.clone().unwrap_or_else(|| handle.clone()),
        }
    }

    pub(super) fn label(&self) -> String {
        match self {
            Self::Anyone => "anyone".into(),
            Self::Me => "me".into(),
            Self::Person { handle, .. } => handle.clone(),
        }
    }

    pub(super) fn is_person(&self) -> bool {
        matches!(self, Self::Person { .. })
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct SourceInfo {
    pub source: String,
    pub name: String,
    pub tab: bool,
    pub repository: bool,
    pub state: SourceState,
    pub missing: Vec<String>,
    pub reason: Option<String>,
    pub account: Option<String>,
    pub from: Option<String>,
    pub env: Option<String>,
    pub fields: [String; 2],
    pub people: [Who; 2],
}

impl SourceInfo {
    pub(super) fn connected(&self) -> bool {
        matches!(self.state, SourceState::Connected | SourceState::Failing)
    }

    pub(super) fn account_line(&self) -> String {
        let mut parts = Vec::new();
        if let Some(account) = self.account.as_deref().filter(|a| !a.is_empty()) {
            parts.push(format!("as {account}"));
        }
        match (self.from.as_deref(), self.env.as_deref()) {
            (Some("gh"), _) => parts.push("through gh".into()),
            (Some("env"), Some(var)) => parts.push(format!("token from ${var}")),
            (Some("env"), None) => parts.push("token from the environment".into()),
            (Some("saved"), _) => parts.push("saved token".into()),
            _ => {}
        }
        parts.join(", ")
    }

    pub(super) fn status_line(&self) -> String {
        match self.state {
            SourceState::Connected => capitalise(format!("connected {}", self.account_line()).trim()),
            SourceState::Failing => self.reason.as_deref().map_or_else(|| "Connected, but failing".into(), capitalise),
            SourceState::Missing if !self.missing.is_empty() => capitalise(&self.missing_line()),
            _ => "Not connected".into(),
        }
    }

    pub(super) fn missing_line(&self) -> String {
        if self.missing.iter().any(|m| m == "repository") {
            return format!("this project's folder is not a {} repository", self.name);
        }
        let words: Vec<String> = self
            .missing
            .iter()
            .map(|m| match m.as_str() {
                "email" => "an email".to_string(),
                other => format!("a {other}"),
            })
            .collect();
        match words.len() {
            0 => String::new(),
            1 => format!("needs {}", words[0]),
            n => format!("needs {} and {}", words[..n - 1].join(", "), words[n - 1]),
        }
    }

    pub(super) fn connect_hint(&self) -> String {
        match self.source.as_str() {
            "github" => "gh auth login".into(),
            "jira" => "cornercase config set jira_site … jira_email …, then config secret jira".into(),
            "plane" => "cornercase config set plane_workspace …, then config secret plane".into(),
            other => format!("printf '%s' \"$TOKEN\" | cornercase config secret {other}"),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(super) struct Place {
    pub project: u64,
    pub workspace: Option<u64>,
    pub label: String,
    pub worktree: bool,
    pub here: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct Sources {
    pub project: u64,
    pub closed: bool,
    #[serde(rename = "sources")]
    pub list: Vec<SourceInfo>,
    pub places: Vec<Place>,
}

impl Sources {
    pub(super) fn get(&self, source: &str) -> Option<&SourceInfo> {
        self.list.iter().find(|s| s.source == source)
    }

    pub(super) fn tabs(&self) -> Vec<&SourceInfo> {
        self.list.iter().filter(|s| s.tab).collect()
    }

    pub(super) fn any_connected(&self) -> bool {
        self.list.iter().any(SourceInfo::connected)
    }

    pub(super) fn project_place(&self) -> Option<&Place> {
        self.places.iter().find(|p| p.project == self.project && p.workspace.is_none())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(super) struct Issue {
    pub source: String,
    pub key: String,
    pub number: u64,
    pub title: String,
    pub url: String,
    pub state: String,
    pub closed: bool,
    pub labels: Vec<String>,
    pub assignees: Vec<String>,
    pub author: String,
    pub updated_at: String,
    pub branch: String,
}

impl Issue {
    pub(super) fn matches(&self, query: &str) -> bool {
        let hay = format!(
            "{} {} {} {} {} {}",
            self.key,
            self.title,
            self.labels.join(" "),
            self.author,
            self.assignees.join(" "),
            self.state
        )
        .to_lowercase();
        query.split_whitespace().all(|word| hay.contains(&word.to_lowercase()))
    }

    pub(super) fn info_extras(&self, info: &[String]) -> Vec<String> {
        let labels = self.labels.join(", ");
        info.iter()
            .filter(|i| !i.eq_ignore_ascii_case(&self.state) && **i != labels && !self.labels.contains(i))
            .cloned()
            .collect()
    }

    pub(super) fn id(&self) -> (String, String) {
        (self.source.clone(), self.key.clone())
    }

    pub(super) fn info(&self, read: Option<&ReadIssue>) -> Vec<String> {
        if let Some(read) = read.filter(|r| !r.info.is_empty()) {
            return read.info.clone();
        }
        let mut info = vec![self.state.clone()];
        if !self.assignees.is_empty() {
            info.push(format!("assigned to {}", self.assignees.join(", ")));
        }
        if !self.author.is_empty() {
            info.push(format!("opened by {}", self.author));
        }
        info
    }

    pub(super) fn state_label(&self) -> String {
        match self.state.trim() {
            "" if self.closed => "Closed".into(),
            "" => "Open".into(),
            state => capitalise(state),
        }
    }

    pub(super) fn state_note(&self) -> Option<&str> {
        let state = self.state.trim();
        let plain = state.is_empty() || state.eq_ignore_ascii_case("open") || state.eq_ignore_ascii_case("closed");
        (!plain).then_some(state)
    }

    pub(super) fn meta(&self, source: Option<String>, now: i64) -> String {
        let mut meta: Vec<String> = source.into_iter().collect();
        if !self.assignees.is_empty() {
            meta.push(format!("→ {}", self.assignees.join(", ")));
        } else if !self.author.is_empty() {
            meta.push(format!("by {}", self.author));
        }
        if !self.updated_at.is_empty() {
            meta.push(ago(&self.updated_at, now));
        }
        meta.join(" · ")
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct Listed {
    pub source: String,
    pub count: usize,
    pub cached: bool,
    pub account: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct Lists {
    pub project: u64,
    pub issues: Vec<Issue>,
    pub sources: Vec<Listed>,
}

pub(super) const LIST_CAP: usize = 100;

pub(super) fn count_line(shown: usize, total: usize, closed: bool) -> String {
    let what = if closed { "issue" } else { "open issue" };
    let plural = if total == 1 { "" } else { "s" };
    if shown == total { format!("{total} {what}{plural}") } else { format!("{shown} of {total} {what}{plural}") }
}

pub(super) fn capitalise(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(super) struct Comment {
    pub author: String,
    pub created_at: String,
    pub body: String,
}

impl Comment {
    pub(super) fn when(&self, now: i64) -> String {
        if self.created_at.is_empty() { String::new() } else { ago(&self.created_at, now) }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(super) struct ReadIssue {
    pub issue: Issue,
    pub info: Vec<String>,
    pub body: String,
    pub comments: Vec<Comment>,
    pub prompt: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(super) struct Person {
    pub handle: String,
    pub name: String,
    pub id: Option<String>,
}

impl Person {
    pub(super) fn who(&self) -> Who {
        Who::Person { handle: self.handle.clone(), id: self.id.clone() }
    }

    pub(super) fn label(&self) -> String {
        match self.display() {
            (name, Some(handle)) => format!("{name} ({handle})"),
            (name, None) => name,
        }
    }

    pub(super) fn display(&self) -> (String, Option<String>) {
        match (self.name.trim(), self.handle.trim()) {
            ("", handle) => (handle.to_string(), None),
            (name, handle) if name == handle || handle.is_empty() => (name.to_string(), None),
            (name, handle) => (name.to_string(), Some(handle.to_string())),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct People {
    pub source: String,
    pub fields: [String; 2],
    pub cached: bool,
    #[serde(rename = "people")]
    pub list: Vec<Person>,
}

impl People {
    pub(super) fn matching(&self, query: &str) -> Vec<&Person> {
        let query = query.to_lowercase();
        self.list.iter().filter(|p| query.is_empty() || p.label().to_lowercase().contains(&query)).collect()
    }
}

pub(super) fn parse<T: for<'de> Deserialize<'de>>(out: &str) -> Option<T> {
    serde_json::from_str(out.trim()).ok()
}

pub(super) fn parse_time(text: &str) -> Option<i64> {
    let text = text.trim();
    let days = parse_date(text.get(..10)?)?;
    let rest = text.get(10..).unwrap_or("");
    if rest.is_empty() {
        return Some(days * 86_400);
    }
    let rest = rest.strip_prefix(['T', ' '])?;
    let clock = parse_clock(rest)?;
    let offset = parse_offset(zone(rest))?;
    Some(days * 86_400 + clock - offset)
}

fn number(text: &str) -> Option<i64> {
    text.parse().ok()
}

fn parse_date(date: &str) -> Option<i64> {
    let mut parts = date.split('-');
    let year = number(parts.next()?)?;
    let month = number(parts.next()?)?;
    let day = number(parts.next()?)?;
    Some(days_from_civil(year, month, day))
}

fn parse_clock(time: &str) -> Option<i64> {
    let hours = number(time.get(0..2)?)?;
    let minutes = number(time.get(3..5)?)?;
    let seconds = number(time.get(6..8).unwrap_or("00"))?;
    Some(hours * 3600 + minutes * 60 + seconds)
}

fn zone(time: &str) -> &str {
    let tail = time.get(8..).unwrap_or("");
    match tail.strip_prefix('.') {
        Some(fraction) => fraction.trim_start_matches(|c: char| c.is_ascii_digit()),
        None => tail,
    }
}

fn parse_offset(zone: &str) -> Option<i64> {
    if matches!(zone, "" | "Z" | "z") {
        return Some(0);
    }
    let sign = match zone.chars().next()? {
        '+' => 1,
        '-' => -1,
        _ => return None,
    };
    let digits: String = zone[1..].chars().filter(char::is_ascii_digit).collect();
    let hours = number(digits.get(0..2)?)?;
    let minutes = digits.get(2..4).and_then(number).unwrap_or(0);
    Some(sign * (hours * 3600 + minutes * 60))
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let march_month = (month + 9) % 12;
    let day_of_year = (153 * march_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

pub(super) fn now() -> i64 {
    let since = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    i64::try_from(since.as_secs()).unwrap_or(i64::MAX)
}

pub(super) fn ago(text: &str, now: i64) -> String {
    let Some(at) = parse_time(text) else { return text.to_string() };
    let secs = (now - at).max(0);
    match secs {
        0..60 => "just now".into(),
        60..3600 => format!("{}m ago", secs / 60),
        3600..86_400 => format!("{}h ago", secs / 3600),
        86_400..2_592_000 => format!("{}d ago", secs / 86_400),
        2_592_000..31_536_000 => format!("{}mo ago", secs / 2_592_000),
        _ => format!("{}y ago", secs / 31_536_000),
    }
}

pub(super) fn sort_newest_first(issues: &mut [Issue]) {
    issues.sort_by_key(|i| std::cmp::Reverse(parse_time(&i.updated_at).unwrap_or(i64::MIN)));
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Choice {
    pub value: String,
    pub note: String,
    pub dangerous: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Agents {
    pub default: String,
    pub list: Vec<Choice>,
    pub modes: HashMap<String, (String, Vec<Choice>)>,
}

pub(super) const EFFORT_AGENTS: [&str; 4] = ["claude", "codex", "grok", "copilot"];

impl Agents {
    pub(super) fn from_config(config: &Value) -> Self {
        let mut agents = Agents::default();
        let Some(settings) = config.get("settings").and_then(Value::as_array) else { return agents };
        let choices = |s: &Value| -> Vec<Choice> {
            s.get("choices")
                .and_then(Value::as_array)
                .map(|list| {
                    list.iter()
                        .map(|c| Choice {
                            value: c.get("value").and_then(Value::as_str).unwrap_or_default().to_string(),
                            note: c.get("note").and_then(Value::as_str).unwrap_or_default().to_string(),
                            dangerous: c.get("dangerous").and_then(Value::as_bool).unwrap_or(false),
                        })
                        .filter(|c| !c.value.is_empty())
                        .collect()
                })
                .unwrap_or_default()
        };
        for setting in settings {
            let key = setting.get("key").and_then(Value::as_str).unwrap_or_default();
            let value = setting.get("value").and_then(Value::as_str).unwrap_or_default().to_string();
            if key == "agent" {
                agents.default = value;
                agents.list = choices(setting).into_iter().filter(|c| c.value != "auto").collect();
            } else if let Some(agent) = key.strip_prefix("agents.").and_then(|k| k.strip_suffix(".mode")) {
                agents.modes.insert(agent.to_string(), (value, choices(setting)));
            }
        }
        agents
    }

    pub(super) fn initial(&self) -> String {
        if self.list.iter().any(|a| a.value == self.default) {
            return self.default.clone();
        }
        self.list.first().map_or_else(|| "claude".into(), |a| a.value.clone())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum When {
    At,
    Tomorrow,
    On,
    In,
    ClaudeReset,
    CodexReset,
}

impl When {
    pub(super) const ALL: [When; 6] =
        [Self::At, Self::Tomorrow, Self::On, Self::In, Self::ClaudeReset, Self::CodexReset];

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::At => "at",
            Self::Tomorrow => "tomorrow at",
            Self::On => "on",
            Self::In => "in",
            Self::ClaudeReset => "Claude reset",
            Self::CodexReset => "Codex reset",
        }
    }

    pub(super) fn placeholder(self) -> Option<&'static str> {
        match self {
            Self::At => Some("14:30"),
            Self::Tomorrow => Some("9:00"),
            Self::On => Some("2026-10-20 09:00"),
            Self::In => Some("2h, 30m, 1d"),
            Self::ClaudeReset | Self::CodexReset => None,
        }
    }

    pub(super) fn at(self, value: &str) -> Option<String> {
        let value = value.trim();
        let needs = |text: String| if value.is_empty() { None } else { Some(text) };
        match self {
            Self::At | Self::On => needs(value.to_string()),
            Self::Tomorrow => needs(format!("tomorrow {value}")),
            Self::In => needs(format!("in {}", value.strip_prefix("in ").unwrap_or(value).replace(' ', ""))),
            Self::ClaudeReset => Some("claude reset".into()),
            Self::CodexReset => Some("codex reset".into()),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct StartPlan {
    pub issue: Issue,
    pub agent: String,
    pub mode: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub place: Option<Place>,
    pub branch: Option<String>,
    pub workspace_name: Option<String>,
}

impl StartPlan {
    pub(super) fn start_args(&self) -> Vec<String> {
        let mut a = vec!["start".to_string(), self.agent.clone(), "--issue".into(), self.issue.key.clone()];
        a.extend(["--source".into(), self.issue.source.clone()]);
        self.place_args(&mut a);
        for (flag, value) in [
            ("--mode", &self.mode),
            ("--model", &self.model),
            ("--effort", &self.effort),
            ("--worktree", &self.branch),
            ("--workspace-name", &self.workspace_name),
        ] {
            if let Some(value) = value {
                a.extend([flag.to_string(), value.clone()]);
            }
        }
        a.extend(["--focus".into(), "--json".into()]);
        a
    }

    pub(super) fn queue_args(&self, at: &str) -> Vec<String> {
        let mut a = vec!["queue".to_string(), "add".into(), "--at".into(), at.to_string()];
        a.extend(["--issue".into(), self.issue.key.clone(), "--source".into(), self.issue.source.clone()]);
        a.extend(["--agent".into(), self.agent.clone()]);
        if let Some(mode) = &self.mode {
            a.extend(["--mode".into(), mode.clone()]);
        }
        self.place_args(&mut a);
        if self.place.as_ref().is_some_and(|p| p.worktree) {
            let branch = self.branch.clone().unwrap_or_else(|| self.issue.branch.clone());
            if !branch.is_empty() {
                a.extend(["--worktree".into(), branch]);
            }
        }
        if let Some(name) = &self.workspace_name {
            a.extend(["--name".into(), name.clone()]);
        }
        a
    }

    fn place_args(&self, a: &mut Vec<String>) {
        match &self.place {
            Some(Place { workspace: Some(w), .. }) => a.extend(["--workspace".into(), w.to_string()]),
            Some(place) => a.extend(["--project".into(), place.project.to_string()]),
            None => {}
        }
    }
}

#[cfg(test)]
mod tests;
