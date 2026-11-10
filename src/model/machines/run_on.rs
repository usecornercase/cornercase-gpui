use std::time::{Duration, Instant};

use serde::Deserialize;

use super::seen::{MachinePlace, MachineSeen, Spot, ThereId};
use super::{Machine, MachineIcon};
use crate::cli::Version;
use crate::model::{Project, Session};

pub(crate) mod checks;
pub(crate) mod clone;
pub(crate) mod issue;

use clone::Cloned;

pub(crate) const NO_REMOTE: &str = "This project has no git remote, so it runs on this computer only: a remote is how a machine finds the same project.";
const ANSWER_UNREAD: &str = "its answer could not be read";
const FROM_ANOTHER_SERVER: &str = "Its address means nothing from this computer";
const AWAITED_FOR: Duration = Duration::from_secs(30);

pub(crate) fn server_release(machine: Option<&str>, here: Option<&Session>, list: &[Machine]) -> Option<Version> {
    let text = match machine {
        None => return here?.release(),
        Some(name) => list.iter().find(|listed| listed.name == name)?.server_version.as_deref()?,
    };
    Version::parse(text)
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Source<'a, T> {
    Here,
    There(Option<&'a T>),
}

impl<T> Clone for Source<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Source<'_, T> {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Offer {
    pub name: String,
    pub icon: MachineIcon,
    pub why: Option<String>,
    pub said: Option<String>,
}

fn why_not(machine: &Machine, seen: &[MachineSeen], host: Option<&str>, now: i64) -> Option<String> {
    if let Some(host) = host {
        return Some(format!("linked from {host}"));
    }
    if !machine.linked() {
        return Some(machine.words(now));
    }
    (!seen.iter().any(|there| there.name == machine.name)).then(|| "not seen yet".to_string())
}

fn said(machine: &Machine, host: Option<&str>) -> Option<String> {
    let said = machine.problem.clone().or_else(|| machine.error.clone());
    host.map(|_| FROM_ANOTHER_SERVER.to_string()).or(said)
}

pub(crate) fn offers(list: &[Machine], seen: &[MachineSeen], host: Option<&str>, now: i64) -> Vec<Offer> {
    let offer = |machine: &Machine| Offer {
        name: machine.name.clone(),
        icon: machine.icon,
        why: why_not(machine, seen, host, now),
        said: said(machine, host),
    };
    list.iter().map(offer).collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum There {
    Refused(String),
    NoRemote,
    NotCloned,
    Cloning,
    Listing,
    Ready(Spot),
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Looking<'a> {
    pub machine: &'a str,
    pub project: Option<&'a Project>,
    pub list: &'a [Machine],
    pub seen: &'a [MachineSeen],
    pub cloning: bool,
    pub cloned: &'a [Cloned],
    pub now: Instant,
}

impl Looking<'_> {
    pub(crate) fn there(&self) -> There {
        let name = self.machine;
        if !self.list.iter().any(|machine| machine.name == name) {
            return There::Refused(format!("{name} is no longer listed"));
        }
        let Some(project) = self.project else { return There::Refused("Choose a project first".into()) };
        if project.repo.is_none() {
            return There::NoRemote;
        }
        let Some(seen) = self.seen.iter().find(|there| there.name == name).filter(|there| there.linked()) else {
            return There::Refused(self.not_linked());
        };
        if self.cloning {
            return There::Cloning;
        }
        self.found(seen, project)
    }

    fn not_linked(&self) -> String {
        let seen = self.seen.iter().find(|there| there.name == self.machine);
        seen.map_or_else(|| format!("{} is not linked", self.machine), MachineSeen::link_note)
    }

    fn found(&self, seen: &MachineSeen, project: &Project) -> There {
        if let Some(there) = seen.pair_of(project) {
            return There::Ready(seen.spot(there));
        }
        let mut cloned = self.cloned.iter().rev().filter(|cloned| cloned.of(seen, project.id));
        if let Some(there) = cloned.clone().find_map(|cloned| seen.project(cloned.there)) {
            return There::Ready(seen.spot(there));
        }
        if cloned.any(|cloned| cloned.listing_soon(self.now)) {
            return There::Listing;
        }
        There::NotCloned
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Starting {
    Agent { agent: String, issue: String },
    Workspace { name: String },
    Flow { name: String },
    LoginTab { agent: String },
}

impl Starting {
    pub(crate) fn doing(&self, machine: &str) -> String {
        match self {
            Self::Agent { agent, issue } => format!("Starting {agent} on {issue} on {machine}…"),
            Self::Workspace { name } => format!("Creating {name} on {machine}…"),
            Self::Flow { name } => format!("Starting {name} on {machine}…"),
            Self::LoginTab { agent } => format!("Opening a tab for {agent} on {machine}…"),
        }
    }

    pub(crate) fn done(&self, machine: &str, answer: &Answer) -> String {
        match self {
            Self::Agent { agent, issue } => format!("{agent} is starting on {issue} on {machine}"),
            Self::Workspace { name } => format!("Created {name} on {machine}"),
            Self::Flow { name } => format!(
                "Started {name} on {machine}{}. Follow it in its tab: the run page reads this computer's runs",
                answer.branch.as_deref().map(|branch| format!(", on {branch}")).unwrap_or_default()
            ),
            Self::LoginTab { agent } => format!("Opened a tab for {agent} on {machine}"),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Answer {
    pub workspace: Option<ThereId>,
    pub tab: Option<ThereId>,
    pub branch: Option<String>,
    pub note: Option<String>,
    pub members: Vec<AnswerMember>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct AnswerMember {
    pub tab: Option<ThereId>,
}

pub(crate) fn parse_answer(out: &str) -> Result<Answer, String> {
    let answer: Answer = serde_json::from_str(out.trim()).map_err(|_| ANSWER_UNREAD.to_string())?;
    if answer.item().is_none() {
        return Err(ANSWER_UNREAD.to_string());
    }
    Ok(answer)
}

impl Answer {
    fn item(&self) -> Option<Item> {
        let tab = self.members.first().and_then(|first| first.tab).or(self.tab);
        tab.map(Item::Tab).or(self.workspace.map(Item::Workspace))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Item {
    Tab(ThereId),
    Workspace(ThereId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Awaited {
    pub machine: String,
    pub boot: String,
    item: Item,
    until: Instant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Look {
    Listed(MachinePlace),
    NotYet,
    Gone,
}

impl Awaited {
    pub(crate) fn answered(spot: &Spot, answer: &Answer, now: Instant) -> Option<Self> {
        Some(Self {
            machine: spot.machine.clone(),
            boot: spot.boot.clone(),
            item: answer.item()?,
            until: now + AWAITED_FOR,
        })
    }

    pub(crate) fn look(&self, machines: &[MachineSeen], now: Instant) -> Look {
        if now >= self.until {
            return Look::Gone;
        }
        let Some(seen) = machines.iter().find(|there| there.name == self.machine) else { return Look::NotYet };
        if seen.boot != self.boot {
            return Look::Gone;
        }
        self.listed(seen).map_or(Look::NotYet, Look::Listed)
    }

    fn listed(&self, seen: &MachineSeen) -> Option<MachinePlace> {
        match &self.item {
            Item::Tab(id) => {
                let ws = seen.workspaces().find(|ws| ws.tabs.iter().any(|tab| tab.id == *id))?;
                Some(MachinePlace::at(seen, ws, Some(*id)))
            }
            Item::Workspace(id) => Some(MachinePlace::at(seen, seen.workspace(*id)?, None)),
        }
    }
}

#[cfg(test)]
mod tests;
