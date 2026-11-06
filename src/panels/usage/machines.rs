use std::iter::once;

use serde::Deserialize;

use super::data::{Plans, Stats, ago, parse_utc};
use crate::model::machines::{LinkState, Machine};

const EVERY: &str = "all";
const NOT_SEEN: &str = "not seen yet";
const NOT_SUMMED: &str = "not summed";
const PEAK: &str = "most at once: the largest peak of one machine, not a sum";
const LAST_NUMBERS_FIRST: &str = "showing the last numbers first needs usecornercase/cornercase-terminal#675";
const NO_TOKENS: &str = "with no prompt and no tokens. Cursor CLI, Gemini CLI, opencode, Grok Build and Copilot have no \
                         plan limits cornercase can read.";

pub(super) type Link = (String, LinkState);

pub(super) fn links(machines: &[Machine]) -> Vec<Link> {
    machines.iter().filter(|m| m.problem.is_none()).map(|m| (m.name.clone(), m.state)).collect()
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub(super) enum Pick {
    Here,
    Machine(String),
    #[default]
    All,
}

pub(super) fn choices(links: &[Link], here: &str) -> Vec<(String, Pick)> {
    if links.is_empty() {
        return Vec::new();
    }
    let machines = links.iter().map(|(name, _)| (name.clone(), Pick::Machine(name.clone())));
    once((here.to_string(), Pick::Here)).chain(machines).chain(once((EVERY.to_string(), Pick::All))).collect()
}

impl Pick {
    pub(super) fn shown(&self, links: &[Link]) -> Self {
        match self {
            _ if links.is_empty() => Self::Here,
            Self::Machine(name) if !links.iter().any(|(listed, _)| listed == name) => Self::All,
            pick => pick.clone(),
        }
    }

    fn command(&self, words: &[&str], output: &str) -> Vec<String> {
        let machine = match self {
            Self::Here => None,
            Self::Machine(name) => Some(name.as_str()),
            Self::All => Some(EVERY),
        };
        let machine = machine.into_iter().flat_map(|name| ["--machine", name]);
        words.iter().copied().chain(machine).chain(once(output)).map(str::to_string).collect()
    }

    pub(super) fn usage(&self, cached: bool) -> Vec<String> {
        self.command(if cached { &["usage", "--cached"] } else { &["usage"] }, "--json")
    }

    pub(super) fn stats(&self) -> Vec<String> {
        self.command(&["stats"], "--json")
    }

    pub(super) fn share(&self) -> Vec<String> {
        self.command(&["stats"], "--share")
    }

    pub(super) fn asking_limits(&self) -> String {
        match self {
            Self::Here => "Asking Claude Code and Codex for their limits…".to_string(),
            Self::Machine(name) => format!("Asking {name} for its limits…"),
            Self::All => "Asking every machine for its limits…".to_string(),
        }
    }

    pub(super) fn asking_stats(&self) -> String {
        match self {
            Self::Here => "Reading the stats…".to_string(),
            Self::Machine(name) => format!("Asking {name} for its stats… {LAST_NUMBERS_FIRST}"),
            Self::All => format!("Asking every machine for its stats… {LAST_NUMBERS_FIRST}"),
        }
    }

    pub(super) fn asked_on(&self) -> String {
        match self {
            Self::Here => format!("Asked on the server's machine {NO_TOKENS}"),
            Self::Machine(name) => format!("Asked on {name} {NO_TOKENS}"),
            Self::All => format!("Asked on each machine {NO_TOKENS}"),
        }
    }

    pub(super) fn counted_on(&self) -> &'static str {
        match self {
            Self::Here => "your machine",
            Self::Machine(_) | Self::All => "your machines",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(super) struct MachineRead {
    pub name: String,
    pub state: LinkState,
    pub seen: Option<String>,
    pub read_at: Option<String>,
    pub error: Option<String>,
}

fn since(stamp: Option<&str>, now: i64) -> Option<String> {
    Some(ago(now - parse_utc(stamp?)?))
}

impl MachineRead {
    pub(super) fn read(&self) -> Option<i64> {
        parse_utc(self.read_at.as_deref()?)
    }

    pub(super) fn unread(&self) -> Option<&str> {
        self.read_at.is_none().then(|| self.error.as_deref().unwrap_or(NOT_SEEN))
    }

    pub(super) fn seen(&self, now: i64) -> Option<String> {
        (self.state != LinkState::Linked).then(|| {
            since(self.seen.as_deref(), now).map_or_else(|| NOT_SEEN.to_string(), |seen| format!("seen {seen}"))
        })
    }

    pub(super) fn failed(&self, now: i64) -> Option<String> {
        let why = self.error.as_deref().filter(|_| self.state == LinkState::Linked)?;
        Some(format!("{why} · numbers read {}", since(self.read_at.as_deref(), now)?))
    }

    fn note(&self, now: i64, summed: bool) -> Option<String> {
        let name = &self.name;
        if let Some(why) = self.unread() {
            return Some(if summed { format!("{name}: {NOT_SUMMED} · {why}") } else { format!("{name}: {why}") });
        }
        self.seen(now).or_else(|| self.failed(now)).map(|why| format!("{name}: {why}"))
    }

    fn old(&self) -> bool {
        self.state != LinkState::Linked || self.error.is_some() || self.read_at.is_none()
    }
}

impl Plans {
    pub(super) fn old(&self) -> bool {
        self.machine.iter().chain(self.machines.iter().map(|there| &there.machine)).any(MachineRead::old)
    }
}

impl Stats {
    pub(super) fn old(&self) -> bool {
        self.machine.iter().chain(&self.machines).any(MachineRead::old)
    }

    pub(super) fn machine_notes(&self, now: i64) -> Vec<String> {
        let one = self.machine.iter().filter_map(|machine| machine.note(now, false));
        one.chain(self.machines.iter().filter_map(|machine| machine.note(now, true))).collect()
    }

    pub(super) fn notes(&self, now: i64) -> Vec<String> {
        let peak = (!self.machines.is_empty()).then(|| PEAK.to_string());
        peak.into_iter().chain(self.machine_notes(now)).collect()
    }
}

#[cfg(test)]
mod tests;
