use serde::Deserialize;

use super::super::seen::{ReadReleases, Spot, ThereId};
use super::issue::IssueThere;
use crate::model::accounts::{AccountsInfo, LoginState, Provider};
use crate::model::agent_settings::Agents;
use crate::store::{args, read_refusal};

const UNREADABLE: &str = "its answer could not be read";
const UNNAMED: &str = "the project";
const LONGEST: usize = 160;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) enum Asked<T> {
    #[default]
    Unasked,
    Asking,
    Read(T),
    Failed(String),
}

impl<T: for<'de> Deserialize<'de>> Asked<T> {
    pub(crate) fn answered(result: anyhow::Result<String>) -> Self {
        match result {
            Ok(out) => read(Ok(&out)),
            Err(e) => read(Err(crate::cli::reason(&e))),
        }
    }
}

impl<T> Asked<T> {
    pub(crate) fn asking(&self) -> bool {
        matches!(self, Self::Asking)
    }

    pub(crate) fn read(&self) -> Option<&T> {
        match self {
            Self::Read(read) => Some(read),
            _ => None,
        }
    }

    fn failure(&self) -> Option<&str> {
        match self {
            Self::Failed(why) => Some(why),
            _ => None,
        }
    }

    fn hold(&self) -> Option<Hold> {
        match self {
            Self::Unasked | Self::Asking => Some(Hold::Waiting),
            Self::Failed(why) => Some(Hold::Why(why.clone())),
            Self::Read(_) => None,
        }
    }

    fn forget_failure(&mut self) {
        if matches!(self, Self::Failed(_)) {
            *self = Self::Unasked;
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[expect(clippy::struct_excessive_bools, reason = "one flag per check a dialog wants, as the checks are")]
pub(crate) struct Wanted {
    pub accounts: bool,
    pub accounts_hold: bool,
    pub tracker: Option<String>,
    pub flows: bool,
    pub modes: bool,
    pub issue: Option<String>,
}

impl Wanted {
    fn issue_args(&self) -> Vec<String> {
        let key = self.issue.as_deref().unwrap_or_default();
        let source = self.tracker.as_deref().unwrap_or_default();
        args(&["issue", "read", key, "--source", source, "--json"])
    }
}

pub(crate) fn check_failure(machine: &str, error: &anyhow::Error) -> String {
    read_refusal(machine, error).unwrap_or_else(|| crate::cli::reason(error))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Check {
    Accounts,
    Trackers,
    Flows,
    Modes,
    Issue,
}

impl Check {
    pub(crate) fn args(self, spot: Option<&Spot>, wanted: &Wanted, releases: ReadReleases) -> Vec<String> {
        let project = spot.map(|spot| spot.asking_args(releases)).unwrap_or_default();
        match self {
            Self::Accounts => args(&["account", "list", "--check", "--json"]),
            Self::Trackers => [args(&["issue", "sources", "--json"]), project].concat(),
            Self::Flows => [args(&["flow", "list", "--json"]), project].concat(),
            Self::Modes => args(&["config", "--json"]),
            Self::Issue => [wanted.issue_args(), project].concat(),
        }
    }

    pub(crate) fn doing(self, machine: &str) -> String {
        match self {
            Self::Accounts => format!("Asking {machine} for its accounts…"),
            Self::Trackers => format!("Checking the tracker on {machine}…"),
            Self::Flows => format!("Reading the flows on {machine}…"),
            Self::Modes => format!("Reading the modes on {machine}…"),
            Self::Issue => format!("Reading the issue on {machine}…"),
        }
    }

    pub(crate) fn failed(self, machine: &str, why: &str) -> String {
        let what = match self {
            Self::Accounts => "its accounts",
            Self::Trackers => "its trackers",
            Self::Flows => "its flows",
            Self::Modes => "its modes",
            Self::Issue => "the issue",
        };
        format!("Could not read {what} on {machine}: {why}")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Hold {
    Waiting,
    Why(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Checks {
    generation: u64,
    boot: Option<String>,
    linked: bool,
    project: Option<ThereId>,
    pub accounts: Asked<AccountsInfo>,
    pub trackers: Asked<Trackers>,
    pub flows: Asked<()>,
    pub modes: Asked<Agents>,
    pub issue: Asked<IssueThere>,
}

impl Checks {
    pub(crate) fn restart(&mut self) {
        *self = Self { generation: self.generation + 1, ..Self::default() };
    }

    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) fn current(&self, generation: u64, check: Check, project: Option<ThereId>) -> bool {
        generation == self.generation && (matches!(check, Check::Accounts | Check::Modes) || project == self.project)
    }

    pub(crate) fn follow(&mut self, boot: &str, linked: bool, project: Option<ThereId>) {
        if self.boot.as_deref().is_some_and(|known| known != boot) {
            self.restart();
        }
        if linked && !self.linked {
            self.forget_failures();
        }
        if project != self.project {
            self.trackers = Asked::Unasked;
            self.flows = Asked::Unasked;
            self.issue = Asked::Unasked;
        }
        self.boot = Some(boot.to_string());
        self.linked = linked;
        self.project = project;
    }

    pub(crate) fn forget_failures(&mut self) {
        self.accounts.forget_failure();
        self.trackers.forget_failure();
        self.flows.forget_failure();
        self.modes.forget_failure();
        self.issue.forget_failure();
    }

    pub(crate) fn due(&self, wanted: &Wanted) -> Vec<Check> {
        let accounts = (wanted.accounts && self.accounts == Asked::Unasked).then_some(Check::Accounts);
        let modes = (wanted.modes && self.modes == Asked::Unasked).then_some(Check::Modes);
        [accounts, self.due_there(wanted), self.due_flows(wanted), modes, self.due_issue(wanted)]
            .into_iter()
            .flatten()
            .collect()
    }

    fn due_there(&self, wanted: &Wanted) -> Option<Check> {
        let asks = self.project.is_some() && wanted.tracker.is_some() && self.trackers == Asked::Unasked;
        asks.then_some(Check::Trackers)
    }

    fn due_flows(&self, wanted: &Wanted) -> Option<Check> {
        let asks = self.project.is_some() && wanted.flows && self.flows == Asked::Unasked;
        asks.then_some(Check::Flows)
    }

    fn due_issue(&self, wanted: &Wanted) -> Option<Check> {
        let source = wanted.tracker.as_deref()?;
        let asks = self.project.is_some() && wanted.issue.is_some() && self.issue == Asked::Unasked;
        let readable = self.trackers.read().is_some_and(|trackers| trackers.readable(source));
        (asks && readable).then_some(Check::Issue)
    }

    pub(crate) fn asked(&mut self, check: Check) {
        match check {
            Check::Accounts => self.accounts = Asked::Asking,
            Check::Trackers => self.trackers = Asked::Asking,
            Check::Flows => self.flows = Asked::Asking,
            Check::Modes => self.modes = Asked::Asking,
            Check::Issue => self.issue = Asked::Asking,
        }
    }

    pub(crate) fn heard(&mut self, check: Check, answer: Result<&str, String>) {
        match check {
            Check::Accounts => self.accounts = read(answer),
            Check::Trackers => self.trackers = read(answer),
            Check::Flows => self.flows = answer.map_or_else(|why| Asked::Failed(clipped(&why)), |_| Asked::Read(())),
            Check::Modes => self.modes = read(answer),
            Check::Issue => self.issue = read(answer),
        }
    }

    pub(crate) fn hold(&self, wanted: &Wanted, machine: &str, project: &str) -> Option<Hold> {
        let trackers = wanted.tracker.as_deref().and_then(|source| self.tracker_hold(source, machine, project));
        let flows = wanted.flows.then(|| self.flows.hold()).flatten();
        let accounts = wanted.accounts_hold.then(|| self.accounts.hold()).flatten();
        trackers.or(flows).or(accounts)
    }

    fn tracker_hold(&self, source: &str, machine: &str, project: &str) -> Option<Hold> {
        match self.trackers.read() {
            Some(trackers) if trackers.failing(source) => None,
            Some(trackers) => trackers.lacks(source, machine, project).map(Hold::Why),
            None => self.trackers.hold(),
        }
    }

    pub(crate) fn tracker_note(&self, wanted: &Wanted, machine: &str, project: &str) -> Option<String> {
        self.trackers.read()?.lacks(wanted.tracker.as_deref()?, machine, project)
    }

    pub(crate) fn asking(&self) -> Vec<Check> {
        let asking = [
            (self.accounts.asking(), Check::Accounts),
            (self.trackers.asking(), Check::Trackers),
            (self.flows.asking(), Check::Flows),
            (self.modes.asking(), Check::Modes),
            (self.issue.asking(), Check::Issue),
        ];
        asking.into_iter().filter(|(asking, _)| *asking).map(|(_, check)| check).collect()
    }

    pub(crate) fn failures(&self) -> Vec<(Check, &str)> {
        let failed = [
            (self.accounts.failure(), Check::Accounts),
            (self.trackers.failure(), Check::Trackers),
            (self.flows.failure(), Check::Flows),
            (self.modes.failure(), Check::Modes),
            (self.issue.failure(), Check::Issue),
        ];
        failed.into_iter().filter_map(|(why, check)| Some((check, why?))).collect()
    }
}

fn read<T: for<'de> Deserialize<'de>>(answer: Result<&str, String>) -> Asked<T> {
    match answer {
        Ok(out) => serde_json::from_str(out.trim()).map_or_else(|_| Asked::Failed(UNREADABLE.to_string()), Asked::Read),
        Err(why) => Asked::Failed(clipped(&why)),
    }
}

pub(crate) fn clipped(text: &str) -> String {
    let line = text.split(char::is_control).map(str::trim).filter(|part| !part.is_empty()).collect::<Vec<_>>();
    let line = line.join(" ");
    if line.chars().count() <= LONGEST {
        return line;
    }
    let kept: String = line.chars().take(LONGEST).collect();
    format!("{}…", kept.trim_end())
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum TrackerState {
    Connected,
    Missing,
    Failing,
    #[default]
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Tracker {
    pub source: String,
    pub name: String,
    pub state: TrackerState,
    pub missing: Vec<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Trackers {
    pub sources: Vec<Tracker>,
}

impl Trackers {
    fn readable(&self, source: &str) -> bool {
        let states = [TrackerState::Connected, TrackerState::Failing];
        self.sources.iter().any(|tracker| tracker.source == source && states.contains(&tracker.state))
    }

    fn failing(&self, source: &str) -> bool {
        self.sources.iter().any(|tracker| tracker.source == source && tracker.state == TrackerState::Failing)
    }

    pub(crate) fn lacks(&self, source: &str, machine: &str, project: &str) -> Option<String> {
        let Some(found) = self.sources.iter().find(|tracker| tracker.source == source) else {
            return Some(format!("{machine} cannot read {source} issues"));
        };
        let project = Some(clipped(project)).filter(|name| !name.is_empty());
        let project = project.as_deref().unwrap_or(UNNAMED);
        let name = Some(clipped(&found.name)).filter(|name| !name.is_empty());
        found.lacking(name.as_deref().unwrap_or(source), machine, project)
    }
}

impl Tracker {
    fn lacking(&self, name: &str, machine: &str, project: &str) -> Option<String> {
        let wants = |what: &str| self.missing.iter().any(|missing| missing == what);
        match self.state {
            TrackerState::Connected => None,
            TrackerState::Missing if wants("token") => Some(format!("{machine} has no {name} token for {project}")),
            TrackerState::Missing if wants("repository") => {
                Some(format!("{project} on {machine} is not a {name} repository"))
            }
            TrackerState::Missing => Some(format!("{machine} lacks {} for {name} in {project}", self.missing_text())),
            TrackerState::Failing => Some(format!("{name} on {machine}: {}", self.reason_text())),
            TrackerState::Other => Some(format!("{machine} cannot tell whether {name} works for {project}")),
        }
    }

    fn missing_text(&self) -> String {
        let said: Vec<String> = self.missing.iter().map(|missing| clipped(missing)).collect();
        let said: Vec<String> = said.into_iter().filter(|missing| !missing.is_empty()).collect();
        if said.is_empty() { "settings".to_string() } else { said.join(", ") }
    }

    fn reason_text(&self) -> String {
        Some(clipped(self.reason.as_deref().unwrap_or_default()))
            .filter(|reason| !reason.is_empty())
            .unwrap_or_else(|| "failing".to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentThere {
    NotInstalled,
    SignedOut,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentNote {
    pub agent: String,
    pub account: Option<String>,
    pub state: AgentThere,
}

impl AgentNote {
    pub(crate) fn text(&self, machine: &str) -> String {
        match self.state {
            AgentThere::NotInstalled => format!("{} is not installed on {machine}", self.agent),
            AgentThere::SignedOut => format!("{} is not logged in on {machine}", self.agent),
        }
    }

    pub(crate) fn button(&self) -> &'static str {
        match self.state {
            AgentThere::NotInstalled => "Open a shell there",
            AgentThere::SignedOut => "Open login tab",
        }
    }
}

fn signed_out(provider: &Provider, folder: &str) -> bool {
    let login = match folder {
        "" => provider.system_login.as_ref(),
        name => provider.accounts.iter().find(|account| account.name == name).and_then(|a| a.login.as_ref()),
    };
    login.is_some_and(|login| login.state == LoginState::SignedOut)
}

impl AccountsInfo {
    fn agent_state(&self, agent: &str, picked: Option<&str>) -> Option<AgentThere> {
        let found = self.agents.iter().find(|known| known.agent == agent)?;
        if !found.installed {
            return Some(AgentThere::NotInstalled);
        }
        let provider = self.providers.iter().find(|provider| provider.id == agent)?;
        let folder = picked.unwrap_or(&provider.active);
        (self.tokens.is_empty() && signed_out(provider, folder)).then_some(AgentThere::SignedOut)
    }

    pub(crate) fn agent_note(&self, agents: &[String], picked: impl Fn(&str) -> Option<String>) -> Option<AgentNote> {
        agents.iter().find_map(|agent| {
            let chosen = picked(agent);
            let state = self.agent_state(agent, chosen.as_deref())?;
            let provider = self.providers.iter().find(|provider| provider.id == *agent);
            let account = provider.map(|provider| chosen.unwrap_or_else(|| provider.active.clone()));
            Some(AgentNote { agent: agent.clone(), account, state })
        })
    }

    pub(crate) fn command_of(&self, agent: &str) -> Option<&str> {
        let found = self.agents.iter().find(|known| known.agent == agent && known.installed)?;
        Some(found.command.as_str()).filter(|command| !command.trim().is_empty()).or(Some(&found.agent))
    }
}

#[cfg(test)]
mod tests;
