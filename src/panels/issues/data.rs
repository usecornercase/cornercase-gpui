//! What `cornercase issue sources|list|read|people --json` and `config --json` print, read into
//! local types, and the small pure helpers the sheet draws with: search, times, the agents'
//! choices, the schedule's time text and the arguments of `start --issue` and `queue add`.

use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;

// ---------------------------------------------------------------------------------------------
// `issue sources`

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceState {
    Connected,
    Missing,
    Failing,
    #[serde(other)]
    #[default]
    Other,
}

/// A filter on people: `me`, `anyone` or someone from `issue people`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Who {
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
    /// What `--assignee` / `--author` take: the id for trackers that have one, else the handle.
    pub fn arg(&self) -> String {
        match self {
            Self::Anyone => "anyone".into(),
            Self::Me => "me".into(),
            Self::Person { handle, id } => id.clone().unwrap_or_else(|| handle.clone()),
        }
    }

    pub fn label(&self) -> String {
        match self {
            Self::Anyone => "anyone".into(),
            Self::Me => "me".into(),
            Self::Person { handle, .. } => handle.clone(),
        }
    }

    /// Whether the filter names one tracker's person (and so needs `--source`).
    pub fn is_person(&self) -> bool {
        matches!(self, Self::Person { .. })
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SourceInfo {
    pub source: String,
    pub name: String,
    /// Shown as a tab (settings → Issues → sources shown).
    pub tab: bool,
    /// Its issues belong to the project's repository (GitHub, GitLab).
    pub repository: bool,
    pub state: SourceState,
    pub missing: Vec<String>,
    pub reason: Option<String>,
    pub account: Option<String>,
    pub from: Option<String>,
    pub env: Option<String>,
    /// The two people filters' names: assignee/author, owner/requester…
    pub fields: [String; 2],
    /// The window's remembered filter.
    pub people: [Who; 2],
}

impl SourceInfo {
    pub fn connected(&self) -> bool {
        matches!(self.state, SourceState::Connected | SourceState::Failing)
    }

    /// Where the connection comes from and as whom: "as @me, through gh".
    pub fn account_line(&self) -> String {
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

    /// What it lacks, in words: "needs a token", "needs a site, an email and a token".
    pub fn missing_line(&self) -> String {
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

    /// How to connect it from a shell, for the empty state.
    pub fn connect_hint(&self) -> String {
        match self.source.as_str() {
            "github" => "gh auth login".into(),
            "jira" => "cornercase config set jira_site … jira_email …, then config secret jira".into(),
            "plane" => "cornercase config set plane_workspace …, then config secret plane".into(),
            other => format!("printf '%s' \"$TOKEN\" | cornercase config secret {other}"),
        }
    }
}

/// Somewhere work on an issue that is not tied to a repository can start: a project's new
/// worktree (`worktree`) or a new tab in a workspace.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Place {
    pub project: u64,
    pub workspace: Option<u64>,
    pub label: String,
    pub worktree: bool,
    pub here: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Sources {
    pub project: u64,
    pub closed: bool,
    pub sources: Vec<SourceInfo>,
    pub places: Vec<Place>,
}

impl Sources {
    pub fn get(&self, source: &str) -> Option<&SourceInfo> {
        self.sources.iter().find(|s| s.source == source)
    }

    /// The tabs, in settings' order: the shown sources.
    pub fn tabs(&self) -> Vec<&SourceInfo> {
        self.sources.iter().filter(|s| s.tab).collect()
    }

    pub fn any_connected(&self) -> bool {
        self.sources.iter().any(SourceInfo::connected)
    }

    /// The place of the project asked: a repository root makes a worktree there.
    pub fn project_place(&self) -> Option<&Place> {
        self.places.iter().find(|p| p.project == self.project && p.workspace.is_none())
    }
}

// ---------------------------------------------------------------------------------------------
// `issue list` and `issue read`

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Issue {
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
    /// The branch `start --issue` makes (settings' issue branch template).
    pub branch: String,
}

impl Issue {
    /// Whether every word of `query` is in its key, title, labels or people (any case).
    pub fn matches(&self, query: &str) -> bool {
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

    /// The detail's info line without what the chips already show: the state and the labels.
    pub fn info_extras(&self, info: &[String]) -> Vec<String> {
        let labels = self.labels.join(", ");
        info.iter()
            .filter(|i| !i.eq_ignore_ascii_case(&self.state) && **i != labels && !self.labels.contains(i))
            .cloned()
            .collect()
    }

    /// What identifies it across tabs.
    pub fn id(&self) -> (String, String) {
        (self.source.clone(), self.key.clone())
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Listed {
    pub source: String,
    pub count: usize,
    pub cached: bool,
    pub account: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Lists {
    pub project: u64,
    pub issues: Vec<Issue>,
    pub sources: Vec<Listed>,
}

/// Trackers answer at most this many issues per list, and offer no further pages.
pub const LIST_CAP: usize = 100;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Comment {
    pub author: String,
    pub created_at: String,
    pub body: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ReadIssue {
    pub issue: Issue,
    /// The detail's info line: state, labels, who it is assigned to, the section…
    pub info: Vec<String>,
    pub body: String,
    pub comments: Vec<Comment>,
    /// What `start` would paste.
    pub prompt: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Person {
    pub handle: String,
    pub name: String,
    pub id: Option<String>,
}

impl Person {
    pub fn who(&self) -> Who {
        Who::Person { handle: self.handle.clone(), id: self.id.clone() }
    }

    pub fn label(&self) -> String {
        match (self.name.trim(), self.handle.trim()) {
            ("", handle) => handle.to_string(),
            (name, handle) if name == handle || handle.is_empty() => name.to_string(),
            (name, handle) => format!("{name} ({handle})"),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct People {
    pub source: String,
    pub fields: [String; 2],
    pub cached: bool,
    pub people: Vec<Person>,
}

/// Reads a command's JSON answer. `issue list` prints its JSON and exits 1 when a tracker
/// failed, so the caller hands us standard output whatever the exit status was.
pub fn parse<T: for<'de> Deserialize<'de>>(out: &str) -> Option<T> {
    serde_json::from_str(out.trim()).ok()
}

// ---------------------------------------------------------------------------------------------
// Times

/// Seconds since the epoch of an RFC 3339-ish time: `2026-10-22T20:13:09Z`, with fractions, or
/// with an offset (`+02:00`, Jira's `+0200`). A bare date is midnight UTC.
pub fn parse_time(text: &str) -> Option<i64> {
    let text = text.trim();
    let num = |s: &str| s.parse::<i64>().ok();
    let date = text.get(..10)?;
    let mut parts = date.split('-');
    let (y, m, d) = (num(parts.next()?)?, num(parts.next()?)?, num(parts.next()?)?);
    let days = days_from_civil(y, m, d);
    let rest = text.get(10..).unwrap_or("");
    if rest.is_empty() {
        return Some(days * 86_400);
    }
    let rest = rest.strip_prefix(['T', ' '])?;
    let (h, mi, s) = (num(rest.get(0..2)?)?, num(rest.get(3..5)?)?, num(rest.get(6..8).unwrap_or("00"))?);
    let mut tail = rest.get(8..).unwrap_or("");
    if let Some(frac) = tail.strip_prefix('.') {
        tail = frac.trim_start_matches(|c: char| c.is_ascii_digit());
    }
    let offset = match tail {
        "" | "Z" | "z" => 0,
        t if t.starts_with(['+', '-']) => {
            let sign = if t.starts_with('-') { -1 } else { 1 };
            let digits: String = t[1..].chars().filter(char::is_ascii_digit).collect();
            let (oh, om) = (num(digits.get(0..2)?)?, digits.get(2..4).and_then(num).unwrap_or(0));
            sign * (oh * 3600 + om * 60)
        }
        _ => return None,
    };
    Some(days * 86_400 + h * 3600 + mi * 60 + s - offset)
}

/// Days since 1970-01-01 of a proleptic Gregorian date (Howard Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

/// "just now", "5m ago", "3h ago", "4d ago", "2mo ago", "1y ago"; the text itself when it is not
/// a time.
pub fn ago(text: &str, now: i64) -> String {
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

/// Newest first, as the All tab orders a merged list; times that do not parse go last.
pub fn sort_newest_first(issues: &mut [Issue]) {
    issues.sort_by_key(|i| std::cmp::Reverse(parse_time(&i.updated_at).unwrap_or(i64::MIN)));
}

// ---------------------------------------------------------------------------------------------
// Agents, from `config --json`

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Choice {
    pub value: String,
    pub note: String,
    pub dangerous: bool,
}

/// The agents `start` can run, settings' default, and each agent's modes with its chosen one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Agents {
    /// Settings → Agents → default agent; `auto` is "the agent in your tab, otherwise ask".
    pub default: String,
    pub agents: Vec<Choice>,
    /// agent → (its chosen mode, the modes).
    pub modes: HashMap<String, (String, Vec<Choice>)>,
}

/// The agents that take `--effort`, per `start --help`.
pub const EFFORT_AGENTS: [&str; 4] = ["claude", "codex", "grok", "copilot"];

impl Agents {
    pub fn from_config(config: &Value) -> Self {
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
                agents.agents = choices(setting).into_iter().filter(|c| c.value != "auto").collect();
            } else if let Some(agent) = key.strip_prefix("agents.").and_then(|k| k.strip_suffix(".mode")) {
                agents.modes.insert(agent.to_string(), (value, choices(setting)));
            }
        }
        agents
    }

    /// The agent the form starts with: settings' default, else the first one.
    pub fn initial(&self) -> String {
        if self.agents.iter().any(|a| a.value == self.default) {
            return self.default.clone();
        }
        self.agents.first().map(|a| a.value.clone()).unwrap_or_else(|| "claude".into())
    }
}

// ---------------------------------------------------------------------------------------------
// The commands

/// When a scheduled start happens, as the TUI's schedule form asks it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum When {
    At,
    Tomorrow,
    On,
    In,
    ClaudeReset,
    CodexReset,
}

impl When {
    pub const ALL: [When; 6] = [Self::At, Self::Tomorrow, Self::On, Self::In, Self::ClaudeReset, Self::CodexReset];

    pub fn label(self) -> &'static str {
        match self {
            Self::At => "at",
            Self::Tomorrow => "tomorrow at",
            Self::On => "on",
            Self::In => "in",
            Self::ClaudeReset => "Claude reset",
            Self::CodexReset => "Codex reset",
        }
    }

    /// The example its value field shows; `None` when it takes no value.
    pub fn placeholder(self) -> Option<&'static str> {
        match self {
            Self::At => Some("14:30"),
            Self::Tomorrow => Some("9:00"),
            Self::On => Some("2026-10-20 09:00"),
            Self::In => Some("2h, 30m, 1d"),
            Self::ClaudeReset | Self::CodexReset => None,
        }
    }

    /// What `queue add --at` takes; `None` while a value it needs is empty.
    pub fn at(self, value: &str) -> Option<String> {
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

/// What the start form decided.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StartPlan {
    pub issue: Issue,
    pub agent: String,
    /// Only when it differs from the agent's chosen mode.
    pub mode: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub place: Option<Place>,
    /// Only when it differs from the issue's own branch.
    pub branch: Option<String>,
    pub workspace_name: Option<String>,
}

impl StartPlan {
    /// `cornercase start AGENT --issue KEY --source S --project P|--workspace W …  --json`.
    /// No `--view`: with it the CLI's feature check for `--issue` fails (#461); the pane it
    /// prints is focused in our view afterwards.
    pub fn start_args(&self) -> Vec<String> {
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
        a.push("--json".into());
        a
    }

    /// `cornercase queue add --at AT --issue KEY --source S --agent A …`. The queue takes no
    /// model or effort, and names a worktree's branch itself only when told: a worktree place
    /// gets the issue's branch (or the one typed).
    pub fn queue_args(&self, at: &str) -> Vec<String> {
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

/// The pane `start --json` answered with.
pub fn started_pane(out: &str) -> Option<u64> {
    let value: Value = serde_json::from_str(out.trim()).ok()?;
    value
        .pointer("/ids/pane")
        .or_else(|| value.get("pane"))
        .and_then(Value::as_u64)
        .or_else(|| value.as_u64())
}

#[cfg(test)]
mod tests;
