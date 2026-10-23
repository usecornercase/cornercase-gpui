//! What the flow commands answer, read into local types: `flow list --json` (the flows, whole),
//! `flow runs --follow` (where each run stands), `flow messages --follow --json` (what the team
//! sent each other) and `flow files --json`. Plus the small pure rules the panel needs: unique
//! names, the order after a move, a run's one-line summary and how long ago a time was.

use serde::Deserialize;

// ---------------------------------------------------------------------------------------------
// `flow list --json`

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Listing {
    pub flows: Vec<FlowInfo>,
    pub broken: Vec<Broken>,
    /// The deleted built-ins, with their flow.
    pub hidden: Vec<FlowInfo>,
    /// Every built-in name, reserved even while hidden.
    pub builtins: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct FlowInfo {
    pub name: String,
    /// `built-in`, `user` (yours) or `project`.
    pub origin: String,
    pub path: Option<String>,
    pub description: String,
    pub problems: Vec<String>,
    /// What a project's flow runs unattended (`env`, `args`), as a sentence.
    pub check: Option<String>,
    pub flow: FlowSpec,
    /// Your version of a built-in.
    pub edited: bool,
    pub default: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Broken {
    pub path: String,
    pub error: String,
}

/// The flow as the contract writes it (`FlowSpec`): `back.to` counts stages from 1.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct FlowSpec {
    pub name: String,
    pub description: String,
    pub notes: String,
    /// `pull_request` or `commits`.
    pub end: String,
    pub orchestrator: RoleSpec,
    pub roles: Vec<RoleSpec>,
    pub stages: Vec<StageSpec>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct RoleSpec {
    pub name: String,
    pub agent: String,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub args: Vec<String>,
    pub env: serde_json::Map<String, serde_json::Value>,
    pub edits: bool,
    /// `shared` or `own`.
    pub worktree: String,
    pub prompt: String,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct StageSpec {
    pub roles: Vec<String>,
    pub back: Option<BackSpec>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct BackSpec {
    /// The stage it goes back to, counted from 1.
    pub to: usize,
    pub when: String,
    pub at_most: u32,
}

impl FlowInfo {
    /// What tells two listed flows apart: a project's flow may share a name with yours.
    pub fn key(&self) -> String {
        format!("{}\u{1f}{}\u{1f}{}", self.origin, self.name, self.path.as_deref().unwrap_or_default())
    }

    /// The orchestrator and every role.
    pub fn agents(&self) -> usize {
        self.flow.roles.len() + 1
    }

    pub fn can_start(&self) -> bool {
        self.problems.is_empty()
    }

    /// The muted tags the TUI's lists show: only the uncommon cases.
    pub fn tags(&self) -> Vec<&'static str> {
        let mut tags = Vec::new();
        if self.default {
            tags.push("default");
        }
        match self.origin.as_str() {
            "project" => tags.push("this project"),
            "user" if self.edited => tags.push("edited"),
            "user" => tags.push("yours"),
            _ => {}
        }
        tags
    }
}

impl FlowSpec {
    /// A role of the plan by name, the orchestrator included.
    pub fn role(&self, name: &str) -> Option<&RoleSpec> {
        if name == self.orchestrator.name || name == "orchestrator" {
            return Some(&self.orchestrator);
        }
        self.roles.iter().find(|r| r.name == name)
    }

    /// The colour slot of a role: its place among the roles (the orchestrator has none).
    pub fn colour_index(&self, name: &str) -> Option<usize> {
        self.roles.iter().position(|r| r.name == name)
    }

    pub fn end_label(&self) -> &'static str {
        match self.end.as_str() {
            "commits" => "leave the commits",
            _ => "open a pull request",
        }
    }
}

pub fn parse_listing(text: &str) -> anyhow::Result<Listing> {
    Ok(serde_json::from_str(text)?)
}

// ---------------------------------------------------------------------------------------------
// `flow runs --follow`

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Runs {
    pub revision: u64,
    pub runs: Vec<Run>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Run {
    pub run: u64,
    pub project: u64,
    pub workspace: u64,
    /// The plan it was briefed with, frozen.
    pub flow: FlowSpec,
    pub task: String,
    pub dir: Option<String>,
    /// The orchestrator first.
    pub roles: Vec<Member>,
    /// The active stage, counted from 0.
    pub stage: Option<usize>,
    /// The rounds of each stage's loop.
    pub rounds: Vec<u32>,
    pub holder: Option<String>,
    pub last: Option<Step>,
    pub outside: Option<Edge>,
    /// The orchestrator marked its pane for review: the run reached its end.
    pub ended: bool,
    pub messages: u64,
    /// `closed` when its agents are closed (to reopen or forget), else absent.
    pub state: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Member {
    pub role: String,
    pub pane: Option<u64>,
    pub tab: Option<u64>,
    pub status: Option<String>,
    pub closed: bool,
    /// `working` while it holds work the orchestrator sent, `finished` until it is told.
    pub awaited: Option<String>,
    pub worktree: Option<String>,
    pub branch: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Step {
    pub edge: Option<Edge>,
    pub from: Option<String>,
    pub to: String,
    pub time: String,
}

/// A step along the plan. Stages count from 0 here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Edge {
    /// From `stage` into the next one.
    Forward { stage: usize },
    /// A round of `stage`'s loop.
    Loop { stage: usize },
    /// Anywhere else.
    Outside { from: Option<usize>, to: usize },
}

impl Run {
    pub fn closed(&self) -> bool {
        self.state.as_deref() == Some("closed")
    }

    pub fn orchestrator(&self) -> Option<&Member> {
        self.roles.first()
    }

    pub fn member(&self, role: &str) -> Option<&Member> {
        self.roles.iter().find(|m| m.role == role)
    }

    /// Where it stands, in a few words: `stage 2 of 3 · findings 1/2`, as `flow runs` prints it.
    pub fn summary(&self) -> String {
        let mut out = match self.stage {
            Some(stage) => format!("stage {} of {}", stage + 1, self.flow.stages.len()),
            None => "not started".to_string(),
        };
        for (stage, rounds) in self.rounds.iter().enumerate().filter(|(_, r)| **r > 0) {
            if let Some(back) = self.flow.stages.get(stage).and_then(|s| s.back.as_ref()) {
                out.push_str(&format!(" · {} {rounds}/{}", back.when, back.at_most));
            }
        }
        out
    }

    /// The loudest state among the members, to badge the run in a list.
    pub fn loudest(&self) -> Option<&str> {
        self.roles
            .iter()
            .filter(|m| !m.closed)
            .filter_map(|m| m.status.as_deref())
            .max_by_key(|s| crate::model::status_rank(Some(s)))
    }

    /// The task's first line.
    pub fn headline(&self) -> &str {
        self.task.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or_default()
    }
}

pub fn parse_runs(line: &str) -> anyhow::Result<Runs> {
    Ok(serde_json::from_str(line)?)
}

// ---------------------------------------------------------------------------------------------
// `flow messages --follow --json`

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Message {
    pub run: u64,
    pub workspace: u64,
    /// Counted from 1 for the run.
    pub seq: u64,
    pub time: String,
    pub from: String,
    pub to: String,
    /// `send`, `keys`, `read` or `wait`.
    pub kind: String,
    pub text: Option<String>,
    /// The real size: the text is cut at 32 KiB.
    pub bytes: u64,
    pub edge: Option<Edge>,
}

impl Message {
    pub fn verb(&self) -> &str {
        match self.kind.as_str() {
            "send" => "sent",
            "keys" => "pressed keys",
            "read" => "read",
            "wait" => "waited for",
            other => other,
        }
    }
}

/// One line of the messages stream.
#[derive(Debug, Clone, PartialEq)]
pub enum Frame {
    Message(Message),
    /// The command fell behind and the server dropped this many: read again to catch up.
    Dropped(u64),
}

pub fn parse_frame(line: &str) -> anyhow::Result<Frame> {
    let value: serde_json::Value = serde_json::from_str(line)?;
    if value.get("event").and_then(serde_json::Value::as_str) == Some("dropped") {
        return Ok(Frame::Dropped(value.get("count").and_then(serde_json::Value::as_u64).unwrap_or_default()));
    }
    Ok(Frame::Message(serde_json::from_value(value)?))
}

// ---------------------------------------------------------------------------------------------
// `flow files --json`

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Files {
    pub dir: String,
    pub files: Vec<RunFile>,
    pub more: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct RunFile {
    pub name: String,
    pub path: String,
    pub bytes: u64,
    pub modified: Option<String>,
}

pub fn parse_files(text: &str) -> anyhow::Result<Files> {
    Ok(serde_json::from_str(text)?)
}

/// `flow run --json`: the new run's workspace.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Started {
    pub run: u64,
    pub workspace: u64,
    pub branch: String,
    pub note: Option<String>,
}

// ---------------------------------------------------------------------------------------------
// Pure rules.

/// Names compare trimmed and ignoring case, as cornercase's `store::same_name` does.
pub fn same_name(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

/// `Quick` becomes `Quick 2` (then 3…) when the name is taken, built-ins counted.
pub fn unique_name(base: &str, taken: &[&str]) -> String {
    let base = base.trim();
    let free = |name: &str| !taken.iter().any(|t| same_name(t, name));
    if free(base) {
        return base.to_string();
    }
    (2..).map(|n| format!("{base} {n}")).find(|name| free(name)).unwrap_or_default()
}

/// The names in list order with the one at `index` moved by `delta`, once each (the order
/// command refuses a name twice; yours and a project's flow of one name share a place).
pub fn moved(names: &[String], index: usize, delta: isize) -> Option<Vec<String>> {
    let mut unique: Vec<String> = Vec::new();
    for name in names {
        if !unique.iter().any(|n| same_name(n, name)) {
            unique.push(name.clone());
        }
    }
    let name = names.get(index)?;
    let from = unique.iter().position(|n| same_name(n, name))?;
    let to = from.checked_add_signed(delta).filter(|to| *to < unique.len())?;
    let item = unique.remove(from);
    unique.insert(to, item);
    Some(unique)
}

/// Seconds since the epoch of a UTC time as cornercase writes it (`2026-10-18T09:21:00.000Z`).
pub fn epoch(time: &str) -> Option<i64> {
    let (date, rest) = time.split_once('T')?;
    let mut d = date.splitn(3, '-').map(|p| p.parse::<i64>().ok());
    let (y, m, day) = (d.next()??, d.next()??, d.next()??);
    let clock = rest.trim_end_matches('Z');
    let clock = clock.split('.').next()?;
    let mut c = clock.splitn(3, ':').map(|p| p.parse::<i64>().ok());
    let (h, min, s) = (c.next()??, c.next()??, c.next().flatten().unwrap_or(0));
    if !(1..=12).contains(&m) || !(1..=31).contains(&day) {
        return None;
    }
    // Days from the civil date (Howard Hinnant's algorithm).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + h * 3600 + min * 60 + s)
}

/// How long ago, shortly: `now`, `12s`, `4m`, `2h`, `3d`.
pub fn ago(time: &str, now: i64) -> String {
    let Some(then) = epoch(time) else { return String::new() };
    let secs = (now - then).max(0);
    match secs {
        0..=4 => "now".into(),
        5..=59 => format!("{secs}s"),
        60..=3599 => format!("{}m", secs / 60),
        3600..=86_399 => format!("{}h", secs / 3600),
        _ => format!("{}d", secs / 86_400),
    }
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_secs()).unwrap_or_default())
        .unwrap_or_default()
}

/// A size for people: `512 B`, `3.4 KB`, `1.2 MB`.
pub fn size(bytes: u64) -> String {
    match bytes {
        0..1000 => format!("{bytes} B"),
        1000..1_000_000 => format!("{:.1} KB", bytes as f64 / 1000.),
        _ => format!("{:.1} MB", bytes as f64 / 1_000_000.),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUNS: &str = include_str!("fixtures/runs.json");
    const LIST: &str = include_str!("fixtures/list.json");

    #[test]
    fn reads_a_run_snapshot() {
        let runs = parse_runs(RUNS).expect("runs");
        assert_eq!(runs.revision, 3);
        let run = &runs.runs[0];
        assert_eq!((run.run, run.workspace, run.stage), (7, 3, Some(0)));
        assert_eq!(run.flow.stages[1].back.as_ref().map(|b| b.to), Some(1));
        assert_eq!(run.orchestrator().map(|m| m.pane), Some(Some(10)));
        assert_eq!(run.member("coder").and_then(|m| m.branch.as_deref()), Some("fix--coder"));
        assert!(run.member("reviewer").is_some_and(|m| m.closed && m.pane == Some(12) && m.tab.is_none()));
        assert_eq!(run.last.as_ref().and_then(|s| s.edge), Some(Edge::Loop { stage: 1 }));
        assert_eq!(run.outside, Some(Edge::Outside { from: Some(0), to: 1 }));
        assert!(run.ended && !run.closed());
        assert_eq!(run.summary(), "stage 1 of 2 · findings 1/2");
        assert_eq!(run.loudest(), Some("pending"));
        assert_eq!(run.flow.role("orchestrator").map(|r| r.model.as_deref()), Some(Some("opus")));
        assert_eq!(run.flow.colour_index("reviewer"), Some(1));
    }

    #[test]
    fn reads_a_closed_run_and_an_empty_one() {
        let line = r#"{"revision":1,"runs":[{"run":2,"workspace":5,"flow":{"name":"Q","stages":[{"roles":["c"],"back":null}]},"roles":[],"stage":null,"rounds":[0],"state":"closed"}]}"#;
        let run = &parse_runs(line).expect("runs").runs[0];
        assert!(run.closed());
        assert_eq!(run.summary(), "not started");
        assert_eq!(parse_runs(r#"{"revision":1,"runs":[],"boot":"x","version":"0.12"}"#).expect("empty").runs.len(), 0);
    }

    #[test]
    fn reads_the_flow_list() {
        let list = parse_listing(LIST).expect("list");
        assert_eq!(list.builtins.len(), 2);
        let mine = &list.flows[1];
        assert_eq!((mine.origin.as_str(), mine.edited, mine.default), ("user", true, true));
        assert_eq!(mine.tags(), vec!["default", "edited"]);
        assert_eq!(mine.path.as_deref(), Some("/c/flows/quick.toml"));
        assert_eq!(mine.agents(), 3);
        assert_eq!(list.flows[0].tags(), Vec::<&str>::new());
        assert!(!list.flows[2].can_start());
        assert_eq!(list.flows[2].tags(), vec!["this project"]);
        assert_eq!(list.flows[2].check.as_deref(), Some("it sets X for coder"));
        assert_eq!(list.hidden[0].name, "Arena");
        assert_eq!(list.broken[0].error, "line 3: expected `=`");
        assert_eq!(list.flows[0].flow.end_label(), "open a pull request");
        assert_eq!(list.flows[1].flow.end_label(), "leave the commits");
        assert_ne!(list.flows[1].key(), list.flows[2].key());
    }

    #[test]
    fn reads_message_frames() {
        let line = r#"{"run":7,"workspace":3,"seq":4,"time":"2026-10-18T09:24:00.000Z","from":"orchestrator","to":"coder","kind":"send","text":"Fix the findings.","bytes":17,"edge":{"kind":"loop","stage":1}}"#;
        let Frame::Message(m) = parse_frame(line).expect("message") else { panic!("not a message") };
        assert_eq!((m.seq, m.verb(), m.edge), (4, "sent", Some(Edge::Loop { stage: 1 })));
        let read = r#"{"run":7,"workspace":3,"seq":3,"time":"t","from":"orchestrator","to":"reviewer","kind":"read","text":null,"bytes":0,"edge":null}"#;
        let Frame::Message(m) = parse_frame(read).expect("read") else { panic!("not a message") };
        assert_eq!((m.text.as_deref(), m.edge, m.verb()), (None, None, "read"));
        assert_eq!(parse_frame(r#"{"event":"dropped","count":5}"#).expect("dropped"), Frame::Dropped(5));
        assert!(parse_frame("not json").is_err());
    }

    #[test]
    fn reads_run_files() {
        let text = r#"{"workspace":3,"run":7,"dir":"/r","files":[{"name":"review.md","path":"/r/review.md","bytes":2048,"modified":"2026-10-18T09:24:00.000Z"}],"more":false}"#;
        let files = parse_files(text).expect("files");
        assert_eq!((files.dir.as_str(), files.files[0].bytes), ("/r", 2048));
    }

    #[test]
    fn makes_names_unique_ignoring_case() {
        assert_eq!(unique_name("Mine", &["Quick"]), "Mine");
        assert_eq!(unique_name("Quick", &["quick ", "Quick 2"]), "Quick 3");
        assert_eq!(unique_name(" Arena", &["arena"]), "Arena 2");
    }

    #[test]
    fn moves_a_flow_in_the_order() {
        let names: Vec<String> = ["A", "B", "b", "C"].iter().map(|s| s.to_string()).collect();
        assert_eq!(moved(&names, 0, 1), Some(vec!["B".into(), "A".into(), "C".into()]));
        assert_eq!(moved(&names, 3, -1), Some(vec!["A".into(), "C".into(), "B".into()]));
        assert_eq!(moved(&names, 2, -1), Some(vec!["B".into(), "A".into(), "C".into()]));
        assert_eq!(moved(&names, 0, -1), None);
        assert_eq!(moved(&names, 3, 1), None);
        assert_eq!(moved(&names, 9, 1), None);
    }

    #[test]
    fn reads_times() {
        assert_eq!(epoch("1970-01-01T00:00:00.000Z"), Some(0));
        assert_eq!(epoch("2026-10-18T09:21:00.000Z"), Some(1_792_315_260));
        assert_eq!(epoch("2000-03-01T00:00:01Z"), Some(951_868_801));
        assert_eq!(epoch("garbage"), None);
        let t = "2026-10-18T09:21:00.000Z";
        let at = epoch(t).unwrap_or_default();
        assert_eq!(ago(t, at + 2), "now");
        assert_eq!(ago(t, at + 30), "30s");
        assert_eq!(ago(t, at + 600), "10m");
        assert_eq!(ago(t, at + 7200), "2h");
        assert_eq!(ago(t, at + 3 * 86_400), "3d");
        assert_eq!(ago("bad", at), "");
        assert_eq!(size(512), "512 B");
        assert_eq!(size(3400), "3.4 KB");
    }
}
