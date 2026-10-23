//! What `cornercase config --json` answers (every setting with its value, default, choices and
//! note; the connections; each project's setup and teardown), how the sheet groups it into pages
//! and sections, and the commands that change it.

use serde::Deserialize;
use serde_json::Value;

use crate::store::args;

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Described {
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
pub enum Kind {
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
pub struct Choice {
    pub value: String,
    pub note: String,
    pub dangerous: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Setting {
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
pub enum ConnectionKind {
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
pub enum State {
    #[default]
    Missing,
    Connected,
    Refused,
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Connection {
    pub key: String,
    pub kind: ConnectionKind,
    pub page: String,
    pub section: String,
    pub label: String,
    pub note: String,
    pub state: State,
    pub account: Option<String>,
    /// Where a token comes from: `saved` or `env`.
    pub from: Option<String>,
    /// The environment variable a token comes from.
    pub env: Option<String>,
    pub count: Option<usize>,
    pub field: Option<String>,
    pub removable: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct ProjectEntry {
    pub folder: String,
    pub key: String,
    pub project: Option<u64>,
    pub repository: bool,
    pub setup: String,
    pub teardown: String,
}

/// What `config export --json` answers.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Exported {
    pub text: String,
    pub settings: usize,
    pub repositories: usize,
    pub left_out: Vec<String>,
    pub warnings: Vec<String>,
}

pub fn parse(text: &str) -> anyhow::Result<Described> {
    Ok(serde_json::from_str(text)?)
}

pub fn parse_export(text: &str) -> anyhow::Result<Exported> {
    Ok(serde_json::from_str(text)?)
}

/// A JSON value as the text `config set` takes: `true`/`false`, a number, a comma-separated list,
/// or the text itself.
pub fn value_text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Array(items) => items.iter().map(value_text).collect::<Vec<_>>().join(","),
        other => other.to_string(),
    }
}

impl Setting {
    pub fn text(&self) -> String {
        value_text(&self.value)
    }

    pub fn is_default(&self) -> bool {
        self.value == self.default
    }

    pub fn on(&self) -> bool {
        self.value.as_bool().unwrap_or(false)
    }

    /// A list setting's items, in their order.
    pub fn items(&self) -> Vec<String> {
        match &self.value {
            Value::Array(items) => items.iter().map(value_text).collect(),
            _ => Vec::new(),
        }
    }

    /// What a switch's state reads as (`shown`, `dimmed`, `every hour`), without the `[x]`.
    pub fn switch_text(&self) -> &str {
        self.shown.trim_start_matches("[x]").trim_start_matches("[ ]").trim()
    }

    pub fn edited_as_text(&self) -> bool {
        matches!(self.kind, Kind::Text | Kind::Number | Kind::Key | Kind::Other)
    }

    /// The choices fit in a row of segments: a few short ones, none dangerous.
    pub fn as_segments(&self) -> bool {
        self.choices.len() <= 4 && self.choices.iter().all(|c| c.value.chars().count() <= 18 && !c.dangerous)
    }

    pub fn choice(&self, value: &str) -> Option<&Choice> {
        self.choices.iter().find(|c| c.value == value)
    }
}

/// `config set KEY VALUE`, safe for values that start with `-` (agent arguments).
pub fn set_args(key: &str, value: &str) -> Vec<String> {
    args(&["config", "set", "--json", "--", key, value])
}

/// `config project`: a project's setup or teardown, by id when it is open, else by folder.
pub fn project_args(entry: &ProjectEntry, which: Script, text: &str) -> Vec<String> {
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

/// A failed command's message without the `cornercase config: error:` around it.
pub fn clean_error(error: &anyhow::Error) -> String {
    let text = error.to_string();
    let text = match text.split_once(": ") {
        Some((head, rest)) if head.starts_with("cornercase ") => rest.to_string(),
        _ => text,
    };
    let text = text.strip_prefix("Error: ").or_else(|| text.strip_prefix("error: ")).unwrap_or(&text);
    text.trim().to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Script {
    Setup,
    Teardown,
}

impl Script {
    pub fn name(self) -> &'static str {
        match self {
            Self::Setup => "setup",
            Self::Teardown => "teardown",
        }
    }

    pub fn of(self, entry: &ProjectEntry) -> &str {
        match self {
            Self::Setup => &entry.setup,
            Self::Teardown => &entry.teardown,
        }
    }
}

/// A whole number, as `fetch_minutes` takes.
pub fn is_number(text: &str) -> bool {
    !text.trim().is_empty() && text.trim().chars().all(|c| c.is_ascii_digit())
}

// ---------------------------------------------------------------------------------------------
// Lists (`issue_tabs`): which items are in, in which order.

/// Adds an item at the end, or removes it.
pub fn list_toggle(items: &[String], item: &str) -> Vec<String> {
    if items.iter().any(|i| i == item) {
        items.iter().filter(|i| *i != item).cloned().collect()
    } else {
        let mut out = items.to_vec();
        out.push(item.to_string());
        out
    }
}

/// Moves the item at `index` one place left (`-1`) or right (`1`); out of range leaves it.
pub fn list_move(items: &[String], index: usize, by: isize) -> Vec<String> {
    let mut out = items.to_vec();
    let Some(to) = index.checked_add_signed(by) else { return out };
    if index < out.len() && to < out.len() {
        out.swap(index, to);
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Connections.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Good,
    Bad,
    Quiet,
}

impl Connection {
    /// The token comes from an environment variable, which wins over a saved one.
    pub fn from_env(&self) -> bool {
        self.from.as_deref() == Some("env")
    }

    /// Takes a token typed into the sheet.
    pub fn takes_secret(&self) -> bool {
        matches!(self.kind, ConnectionKind::Tracker | ConnectionKind::Bot) && !self.from_env()
    }

    /// What its state reads as, and how it is coloured.
    pub fn status(&self) -> (String, Tone) {
        if self.kind == ConnectionKind::Browsers {
            return match self.count.unwrap_or(0) {
                0 => ("none paired".into(), Tone::Quiet),
                1 => ("1 paired".into(), Tone::Good),
                n => (format!("{n} paired"), Tone::Good),
            };
        }
        let source = match (self.from_env(), &self.env) {
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

// ---------------------------------------------------------------------------------------------
// Pages and sections.

/// The sheet's own pages after the settings' ones.
pub const PROJECTS: &str = "Projects";
pub const SHARE: &str = "Share";

/// The order of the TUI's tabs; pages it does not know go after them.
const ORDER: [&str; 5] = ["Worktrees", "Agents", "Issues", "UI", "Remote"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    Setting(usize),
    Connection(usize),
    /// An agent's mode and extra arguments (`agents.<kind>.mode`, `agents.<kind>.extra`), on one row.
    Agent { kind: String, mode: Option<usize>, extra: Option<usize> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub title: String,
    pub items: Vec<Item>,
}

/// The settings' pages, in the TUI's order.
pub fn pages(described: &Described) -> Vec<String> {
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

/// A page's sections, in the order they first appear (connections first, as the TUI lists the
/// trackers' accounts before the sources), each connection after its section's settings.
pub fn sections(described: &Described, page: &str) -> Vec<Section> {
    let mut sections: Vec<Section> = Vec::new();
    let mut titles: Vec<&str> = Vec::new();
    let connections = described.connections.iter().filter(|c| c.page == page).map(|c| c.section.as_str());
    let settings = described.settings.iter().filter(|s| s.page == page).map(|s| s.section.as_str());
    for title in connections.chain(settings) {
        if !titles.contains(&title) {
            titles.push(title);
        }
    }
    // Untitled rows go first, as they open the page.
    titles.sort_by_key(|t| !t.is_empty());
    for title in titles {
        let mut items = Vec::new();
        for (i, setting) in described.settings.iter().enumerate() {
            if setting.page != page || setting.section != title {
                continue;
            }
            match agent_part(&setting.key) {
                Some((kind, part)) => {
                    let slot = items.iter_mut().find_map(|item| match item {
                        Item::Agent { kind: k, mode, extra } if k == kind => Some((mode, extra)),
                        _ => None,
                    });
                    let (mode, extra) = match slot {
                        Some(slot) => slot,
                        None => {
                            items.push(Item::Agent { kind: kind.to_string(), mode: None, extra: None });
                            match items.last_mut() {
                                Some(Item::Agent { mode, extra, .. }) => (mode, extra),
                                _ => continue,
                            }
                        }
                    };
                    if part == "mode" {
                        *mode = Some(i);
                    } else {
                        *extra = Some(i);
                    }
                }
                None => items.push(Item::Setting(i)),
            }
        }
        for (i, connection) in described.connections.iter().enumerate() {
            if connection.page == page && connection.section == title {
                items.push(Item::Connection(i));
            }
        }
        sections.push(Section { title: title.to_string(), items });
    }
    sections
}

/// `agents.claude.mode` → `("claude", "mode")`.
fn agent_part(key: &str) -> Option<(&str, &str)> {
    let rest = key.strip_prefix("agents.")?;
    let (kind, part) = rest.rsplit_once('.')?;
    matches!(part, "mode" | "extra").then_some((kind, part))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
      "revision": 7, "path": "/home/me/.config/cornercase/config.json",
      "settings": [
        {"key":"worktrees_dir","page":"Worktrees","section":"","label":"worktrees folder","type":"text",
         "value":"~/wt","default":"~/.cornercase/worktrees","shown":"~/wt","note":"n","field":"worktrees folder",
         "dangerous":false,"restart":false},
        {"key":"fetch_minutes","page":"Worktrees","section":"","label":"fetch","type":"number","value":5,"default":5,
         "shown":"5 min","note":""},
        {"key":"jira_site","page":"Issues","section":"Jira","label":"Jira site","type":"text","value":"","default":""},
        {"key":"issue_tabs","page":"Issues","section":"Sources shown","label":"sources","type":"list",
         "value":["all","github"],"default":["all","github","linear"],
         "choices":[{"value":"all"},{"value":"github"},{"value":"linear"}]},
        {"key":"agent","page":"Agents","section":"Agent","label":"default agent","type":"choice","value":"auto",
         "choices":[{"value":"auto","note":"ask"},{"value":"claude","note":"claude"}]},
        {"key":"agents.claude.mode","page":"Agents","section":"How each agent starts","type":"choice",
         "value":"default","choices":[{"value":"default"},{"value":"skip permissions (dangerous)","dangerous":true}]},
        {"key":"agents.claude.extra","page":"Agents","section":"How each agent starts","type":"text","value":""},
        {"key":"agents.amp.extra","page":"Agents","section":"How each agent starts","type":"text","value":"--x"},
        {"key":"counts","page":"UI","section":"","type":"switch","value":true,"default":true,"shown":"[x] shown"},
        {"key":"remote","page":"Remote","section":"Browser","type":"switch","value":false,"shown":"[ ] off"},
        {"key":"future","page":"Later","section":"","type":"colour","value":"red"}
      ],
      "connections": [
        {"key":"linear","kind":"tracker","page":"Issues","section":"Accounts","label":"Linear API key",
         "state":"connected","account":"me@example.com","from":"saved","removable":true},
        {"key":"jira","kind":"tracker","page":"Issues","section":"Jira","label":"Jira API token","state":"missing"},
        {"key":"browsers","kind":"browsers","page":"Remote","section":"Browser","count":2,"state":"connected"},
        {"key":"telegram","kind":"bot","page":"Remote","section":"Telegram","state":"refused","from":"env",
         "env":"TELEGRAM_TOKEN"}
      ],
      "projects": [
        {"folder":"/code/shop","key":"","project":4,"repository":true,"setup":"npm ci","teardown":""},
        {"folder":"/code/gone","key":"","repository":false,"setup":"","teardown":"rm -rf x"}
      ]
    }"#;

    #[test]
    fn parses_every_part() {
        let d = parse(SAMPLE).unwrap();
        assert_eq!(d.revision, 7);
        assert_eq!(d.settings.len(), 11);
        assert_eq!(d.settings[1].kind, Kind::Number);
        assert_eq!(d.settings[10].kind, Kind::Other);
        assert_eq!(d.connections[0].state, State::Connected);
        assert_eq!(d.connections[2].kind, ConnectionKind::Browsers);
        assert_eq!(d.projects[0].project, Some(4));
        assert_eq!(d.projects[1].project, None);
        assert!(parse("{}").is_ok());
        assert!(parse("not json").is_err());
    }

    #[test]
    fn values_read_as_set_takes_them() {
        let d = parse(SAMPLE).unwrap();
        assert_eq!(d.settings[0].text(), "~/wt");
        assert_eq!(d.settings[1].text(), "5");
        assert_eq!(d.settings[3].text(), "all,github");
        assert_eq!(d.settings[3].items(), vec!["all", "github"]);
        assert_eq!(d.settings[8].text(), "true");
        assert!(d.settings[8].on());
        assert_eq!(d.settings[8].switch_text(), "shown");
        assert_eq!(d.settings[9].switch_text(), "off");
        assert!(!d.settings[0].is_default());
        assert!(d.settings[1].is_default());
        assert!(d.settings[4].as_segments());
        assert!(!d.settings[5].as_segments());
    }

    #[test]
    fn set_passes_values_that_look_like_options() {
        assert_eq!(set_args("agents.amp.extra", "--model x"), [
            "config",
            "set",
            "--json",
            "--",
            "agents.amp.extra",
            "--model x"
        ]);
    }

    #[test]
    fn project_commands_name_the_project_or_its_folder() {
        let d = parse(SAMPLE).unwrap();
        assert_eq!(project_args(&d.projects[0], Script::Setup, "-x"), [
            "config",
            "project",
            "--json",
            "--project",
            "4",
            "--setup=-x"
        ]);
        assert_eq!(project_args(&d.projects[1], Script::Teardown, ""), [
            "config",
            "project",
            "--json",
            "--teardown=",
            "--",
            "/code/gone"
        ]);
    }

    #[test]
    fn pages_follow_the_tui_order() {
        let d = parse(SAMPLE).unwrap();
        assert_eq!(pages(&d), ["Worktrees", "Agents", "Issues", "UI", "Remote", "Later"]);
    }

    #[test]
    fn sections_group_connections_and_agents() {
        let d = parse(SAMPLE).unwrap();
        let issues = sections(&d, "Issues");
        let titles: Vec<&str> = issues.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(titles, ["Accounts", "Jira", "Sources shown"]);
        assert_eq!(issues[1].items, [Item::Setting(2), Item::Connection(1)]);

        let agents = sections(&d, "Agents");
        assert_eq!(agents[1].items, [
            Item::Agent { kind: "claude".into(), mode: Some(5), extra: Some(6) },
            Item::Agent { kind: "amp".into(), mode: None, extra: Some(7) },
        ]);

        let remote = sections(&d, "Remote");
        assert_eq!(remote[0].items, [Item::Setting(9), Item::Connection(2)]);
        assert_eq!(remote[1].title, "Telegram");
    }

    #[test]
    fn lists_toggle_and_move() {
        let items: Vec<String> = vec!["all".into(), "github".into()];
        assert_eq!(list_toggle(&items, "linear"), ["all", "github", "linear"]);
        assert_eq!(list_toggle(&items, "all"), ["github"]);
        assert_eq!(list_move(&items, 1, -1), ["github", "all"]);
        assert_eq!(list_move(&items, 0, -1), ["all", "github"]);
        assert_eq!(list_move(&items, 1, 1), ["all", "github"]);
    }

    #[test]
    fn connections_say_their_state() {
        let d = parse(SAMPLE).unwrap();
        assert_eq!(d.connections[0].status(), ("connected as me@example.com".into(), Tone::Good));
        assert_eq!(d.connections[1].status(), ("not set".into(), Tone::Quiet));
        assert_eq!(d.connections[2].status(), ("2 paired".into(), Tone::Good));
        assert_eq!(d.connections[3].status(), ("refused · from $TELEGRAM_TOKEN".into(), Tone::Bad));
        assert!(d.connections[0].takes_secret());
        assert!(!d.connections[3].takes_secret());
        assert!(!d.connections[2].takes_secret());
    }

    #[test]
    fn errors_lose_the_command_name() {
        let e = anyhow::anyhow!("cornercase config: error: `fetch_minutes` takes a whole number");
        assert_eq!(clean_error(&e), "`fetch_minutes` takes a whole number");
        let e = anyhow::anyhow!("cornercase config: Error: the token was refused");
        assert_eq!(clean_error(&e), "the token was refused");
        assert_eq!(clean_error(&anyhow::anyhow!("could not run")), "could not run");
    }

    #[test]
    fn numbers_are_whole() {
        assert!(is_number("10"));
        assert!(is_number(" 0 "));
        assert!(!is_number(""));
        assert!(!is_number("-1"));
        assert!(!is_number("1.5"));
    }

    #[test]
    fn exports_parse() {
        let e = parse_export(r#"{"text":"version = 1\n","settings":49,"repositories":1,"left_out":["jira_email"]}"#)
            .unwrap();
        assert_eq!(e.settings, 49);
        assert_eq!(e.left_out, ["jira_email"]);
        assert!(e.warnings.is_empty());
    }
}
