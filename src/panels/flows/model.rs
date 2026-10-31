use serde::Deserialize;

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct Listing {
    pub flows: Vec<FlowInfo>,
    pub broken: Vec<Broken>,
    pub hidden: Vec<FlowInfo>,
    pub builtins: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct FlowInfo {
    pub name: String,
    pub origin: String,
    pub path: Option<String>,
    pub description: String,
    pub problems: Vec<String>,
    pub check: Option<String>,
    pub flow: FlowSpec,
    pub edited: bool,
    pub default: bool,
    #[serde(skip)]
    pub raw: serde_json::Value,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct Broken {
    pub path: String,
    pub error: String,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct FlowSpec {
    pub name: String,
    pub description: String,
    pub notes: String,
    pub end: String,
    pub orchestrator: RoleSpec,
    pub roles: Vec<RoleSpec>,
    pub stages: Vec<StageSpec>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct RoleSpec {
    pub name: String,
    pub agent: String,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub args: Vec<String>,
    pub env: serde_json::Map<String, serde_json::Value>,
    pub edits: bool,
    pub worktree: String,
    pub prompt: String,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct StageSpec {
    pub roles: Vec<String>,
    pub back: Option<BackSpec>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct BackSpec {
    pub to: usize,
    pub when: String,
    pub at_most: u32,
}

impl BackSpec {
    pub(crate) fn label(&self, rounds: Option<u32>, short: bool) -> String {
        let (when, most) = (&self.when, self.at_most);
        match (rounds, short) {
            (Some(rounds), true) => format!("{when} {rounds}/{most}"),
            (Some(rounds), false) => format!("{when} · {rounds}/{most}"),
            (None, true) => format!("{when} ≤ {most}"),
            (None, false) => format!("{when} · at most {most}"),
        }
    }
}

impl FlowInfo {
    pub(crate) fn key(&self) -> String {
        format!("{}\u{1f}{}\u{1f}{}", self.origin, self.name, self.path.as_deref().unwrap_or_default())
    }

    pub(crate) fn agents(&self) -> usize {
        self.flow.roles.len() + 1
    }

    pub(crate) fn can_start(&self) -> bool {
        self.problems.is_empty()
    }

    pub(crate) fn tags(&self) -> Vec<&'static str> {
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
    pub(crate) fn role(&self, name: &str) -> Option<&RoleSpec> {
        if name == self.orchestrator.name || name == "orchestrator" {
            return Some(&self.orchestrator);
        }
        self.roles.iter().find(|r| r.name == name)
    }

    pub(crate) fn is_lead(&self, name: &str) -> bool {
        name == "orchestrator" || name == self.orchestrator.name
    }

    pub(crate) fn every_role(&self) -> impl Iterator<Item = &RoleSpec> {
        std::iter::once(&self.orchestrator).chain(&self.roles)
    }

    pub(crate) fn colour_index(&self, name: &str) -> Option<usize> {
        self.roles.iter().position(|r| r.name == name)
    }

    pub(crate) fn end_label(&self) -> &'static str {
        match self.end.as_str() {
            "commits" => "leave the commits",
            _ => "open a pull request",
        }
    }

    pub(crate) fn ends_in_commits(&self) -> bool {
        self.end == "commits"
    }

    pub(crate) fn loops(&self) -> usize {
        self.stages.iter().filter(|s| s.back.is_some()).count()
    }

    pub(crate) fn outline(&self) -> String {
        let mut parts = vec![plural(self.stages.len(), "stage")];
        if self.loops() > 0 {
            parts.push(plural(self.loops(), "loop"));
        }
        parts.join(" · ")
    }
}

pub(crate) fn plural(count: usize, word: &str) -> String {
    if count == 1 { format!("1 {word}") } else { format!("{count} {word}s") }
}

pub(crate) fn initials(name: &str) -> String {
    let words: Vec<&str> = name.split_whitespace().collect();
    let firsts: String = words.iter().take(2).filter_map(|w| w.chars().next()).flat_map(char::to_uppercase).collect();
    if firsts.is_empty() { "?".into() } else { firsts }
}

impl RoleSpec {
    pub(crate) fn agent_facts(&self) -> Vec<String> {
        [Some(self.agent.clone()), self.model.clone(), self.effort.clone()].into_iter().flatten().collect()
    }
}

pub(crate) fn parse_listing(text: &str) -> anyhow::Result<Listing> {
    let value: serde_json::Value = serde_json::from_str(text)?;
    let mut listing: Listing = serde_json::from_value(value.clone())?;
    keep_raw(&mut listing.flows, &value["flows"]);
    keep_raw(&mut listing.hidden, &value["hidden"]);
    Ok(listing)
}

fn keep_raw(flows: &mut [FlowInfo], entries: &serde_json::Value) {
    for (flow, entry) in flows.iter_mut().zip(entries.as_array().into_iter().flatten()) {
        flow.raw = entry["flow"].clone();
    }
}

pub(crate) fn project_args(project: Option<u64>) -> Vec<String> {
    project.map(|p| vec!["--project".to_string(), p.to_string()]).unwrap_or_default()
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct Runs {
    pub revision: u64,
    pub runs: Vec<Run>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct Run {
    #[serde(rename = "run")]
    pub id: u64,
    pub project: u64,
    pub workspace: u64,
    pub flow: FlowSpec,
    pub task: String,
    pub dir: Option<String>,
    pub roles: Vec<Member>,
    pub stage: Option<usize>,
    pub rounds: Vec<u32>,
    pub holder: Option<String>,
    pub last: Option<Step>,
    pub outside: Option<Edge>,
    pub ended: bool,
    pub messages: u64,
    pub state: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct Member {
    pub role: String,
    pub pane: Option<u64>,
    pub tab: Option<u64>,
    pub status: Option<String>,
    pub closed: bool,
    pub awaited: Option<String>,
    pub worktree: Option<String>,
    pub branch: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct Step {
    pub edge: Option<Edge>,
    pub from: Option<String>,
    pub to: String,
    pub time: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub(crate) enum Edge {
    Forward { stage: usize },
    Loop { stage: usize },
    Outside { from: Option<usize>, to: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Progress {
    Passed,
    Active,
    Ahead,
}

impl Run {
    pub(crate) fn closed(&self) -> bool {
        self.state.as_deref() == Some("closed")
    }

    pub(crate) fn progress(&self, stage: usize) -> Progress {
        match self.stage {
            _ if self.ended => Progress::Passed,
            Some(active) if stage < active => Progress::Passed,
            Some(active) if stage == active => Progress::Active,
            _ => Progress::Ahead,
        }
    }

    pub(crate) fn over_limit(&self, stage: usize) -> bool {
        let rounds = self.rounds.get(stage).copied().unwrap_or_default();
        self.flow.stages.get(stage).and_then(|s| s.back.as_ref()).is_some_and(|back| rounds > back.at_most)
    }

    pub(crate) fn orchestrator(&self) -> Option<&Member> {
        self.roles.first()
    }

    pub(crate) fn member(&self, role: &str) -> Option<&Member> {
        self.roles.iter().find(|m| m.role == role)
    }

    pub(crate) fn summary(&self) -> String {
        let position = match self.stage {
            Some(stage) => format!("stage {} of {}", stage + 1, self.flow.stages.len()),
            None => "not started".to_string(),
        };
        let loops: String = self
            .rounds
            .iter()
            .enumerate()
            .filter(|(_, rounds)| **rounds > 0)
            .filter_map(|(stage, rounds)| {
                let back = self.flow.stages.get(stage)?.back.as_ref()?;
                Some(format!(" · {} {rounds}/{}", back.when, back.at_most))
            })
            .collect();
        position + &loops
    }

    pub(crate) fn loudest(&self) -> Option<&str> {
        self.roles
            .iter()
            .filter(|m| !m.closed)
            .filter_map(|m| m.status.as_deref())
            .max_by_key(|s| crate::model::status_rank(Some(s)))
    }

    pub(crate) fn headline(&self) -> &str {
        self.task.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or_default()
    }

    pub(crate) fn lead_pane(&self) -> Option<u64> {
        let alive = |m: &&Member| !m.closed && m.pane.is_some();
        self.orchestrator().filter(alive).or_else(|| self.roles.iter().find(alive)).and_then(|m| m.pane)
    }
}

pub(crate) fn parse_runs(line: &str) -> anyhow::Result<Runs> {
    Ok(serde_json::from_str(line)?)
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct Message {
    pub run: u64,
    pub workspace: u64,
    pub seq: u64,
    pub time: String,
    pub from: String,
    pub to: String,
    pub kind: String,
    pub text: Option<String>,
    pub bytes: u64,
    pub edge: Option<Edge>,
}

impl Message {
    pub(crate) fn verb(&self) -> &str {
        match self.kind.as_str() {
            "send" => "sent",
            "keys" => "pressed keys",
            "read" => "read",
            "wait" => "waited for",
            other => other,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Frame {
    Message(Message),
    Dropped(u64),
}

pub(crate) fn parse_frame(line: &str) -> anyhow::Result<Frame> {
    let value: serde_json::Value = serde_json::from_str(line)?;
    if value.get("event").and_then(serde_json::Value::as_str) == Some("dropped") {
        return Ok(Frame::Dropped(value.get("count").and_then(serde_json::Value::as_u64).unwrap_or_default()));
    }
    Ok(Frame::Message(serde_json::from_value(value)?))
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct Files {
    pub dir: String,
    #[serde(rename = "files")]
    pub entries: Vec<RunFile>,
    pub more: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct RunFile {
    pub name: String,
    pub path: String,
    pub bytes: u64,
    pub modified: Option<String>,
}

pub(crate) fn parse_files(text: &str) -> anyhow::Result<Files> {
    Ok(serde_json::from_str(text)?)
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct Started {
    pub run: u64,
    pub workspace: u64,
    pub branch: String,
    pub note: Option<String>,
}

pub(crate) fn same_name(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

pub(crate) fn unique_name(base: &str, taken: &[&str]) -> String {
    let base = base.trim();
    let free = |name: &str| !taken.iter().any(|t| same_name(t, name));
    if free(base) {
        return base.to_string();
    }
    (2..=taken.len() + 2).map(|n| format!("{base} {n}")).find(|name| free(name)).unwrap_or_default()
}

pub(crate) fn moved(names: &[String], index: usize, delta: isize) -> Option<Vec<String>> {
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

pub(crate) fn epoch(time: &str) -> Option<i64> {
    let (date, rest) = time.split_once('T')?;
    let mut date_parts = date.splitn(3, '-').map(|p| p.parse::<i64>().ok());
    let (year, month, day) = (date_parts.next()??, date_parts.next()??, date_parts.next()??);
    let clock = rest.trim_end_matches('Z').split('.').next()?;
    let mut clock_parts = clock.splitn(3, ':').map(|p| p.parse::<i64>().ok());
    let (hour, minute) = (clock_parts.next()??, clock_parts.next()??);
    let second = clock_parts.next().flatten().unwrap_or(0);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some(days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second)
}

pub(crate) fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

pub(crate) fn ago(time: &str, now: i64) -> String {
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

pub(crate) fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_secs()).unwrap_or_default())
        .unwrap_or_default()
}

#[expect(clippy::cast_precision_loss, reason = "a size for people keeps one decimal")]
pub(crate) fn size(bytes: u64) -> String {
    match bytes {
        0..1000 => format!("{bytes} B"),
        1000..1_000_000 => format!("{:.1} KB", bytes as f64 / 1000.),
        _ => format!("{:.1} MB", bytes as f64 / 1_000_000.),
    }
}

#[cfg(test)]
mod tests;
