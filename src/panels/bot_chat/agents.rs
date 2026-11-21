use std::collections::HashSet;

use super::entries::Entry;
use crate::model::bots::{self, Home, Kind, Line};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum State {
    Started,
    Finished,
    Failed,
    Waiting,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentLine {
    pub id: u64,
    pub state: State,
    pub words: String,
    pub body: String,
    pub pane: Option<u64>,
}

pub(super) fn state_of(line: &Line) -> Option<State> {
    match line.kind {
        Kind::Started => Some(State::Started),
        Kind::Finished if bots::failed(&line.text) => Some(State::Failed),
        Kind::Finished => Some(State::Finished),
        Kind::Waiting => Some(State::Waiting),
        _ => None,
    }
}

pub(super) fn agent_line(line: &Line, state: State) -> AgentLine {
    AgentLine {
        id: line.id,
        state,
        words: bots::words(&line.text).to_string(),
        body: line.body.as_deref().map(str::trim).unwrap_or_default().to_string(),
        pane: line.agent.as_ref().map(|agent| agent.pane),
    }
}

pub(crate) fn linked(entries: Vec<Entry>, shows: &HashSet<u64>) -> Vec<Entry> {
    entries
        .into_iter()
        .map(|entry| match entry {
            Entry::Agent(mut line) => {
                if !shows.contains(&line.id) {
                    line.pane = None;
                }
                Entry::Agent(line)
            }
            other => other,
        })
        .collect()
}

pub(crate) fn folds(line: &AgentLine) -> bool {
    matches!(line.state, State::Finished | State::Failed) && !line.body.is_empty()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Stop {
    Show(u64),
    Fold(u64),
}

impl Stop {
    pub(crate) fn line(self) -> u64 {
        match self {
            Self::Show(id) | Self::Fold(id) => id,
        }
    }
}

pub(crate) fn stops(entries: &[Entry]) -> Vec<Stop> {
    let agents = entries.iter().filter_map(|entry| match entry {
        Entry::Agent(line) => Some(line),
        _ => None,
    });
    agents
        .flat_map(|line| {
            let show = line.pane.map(|_| Stop::Show(line.id));
            let fold = folds(line).then_some(Stop::Fold(line.id));
            show.into_iter().chain(fold)
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Link {
    Show(u64),
    Closed,
    There(String),
}

pub(crate) fn link(pane: Option<u64>, home: &Home) -> Link {
    match (home.machine(), pane) {
        (Some(machine), _) => Link::There(machine.to_string()),
        (None, Some(pane)) => Link::Show(pane),
        (None, None) => Link::Closed,
    }
}
