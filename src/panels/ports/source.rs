use super::data::Port;
use super::view::{PortView, view_here, view_there};
use crate::model::{Aim, Session, ThereId};
use crate::store::MachineRead;

pub(crate) enum Listed {
    None,
    Here(Vec<Port>),
    There(Vec<Port<ThereId>>),
}

pub(super) enum Source {
    Here(Option<u64>),
    There(Box<MachineRead>),
}

pub(super) fn source_of(machine: Option<MachineRead>, shown: Option<u64>) -> Source {
    match machine {
        Some(read) => Source::There(Box::new(read)),
        None => Source::Here(shown),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Scope {
    Here { all: bool, workspace: Option<u64> },
    There { all: bool, aim: Aim, destination: String },
}

pub(super) fn scope_of(source: &Source, all: bool) -> Scope {
    match source {
        Source::Here(workspace) => Scope::Here { all, workspace: *workspace },
        Source::There(read) => Scope::There { all, aim: read.aim.clone(), destination: read.destination.clone() },
    }
}

pub(super) fn views(listed: &Listed, session: Option<&Session>, aim: Option<&Aim>, all: bool) -> Vec<PortView> {
    match listed {
        Listed::None => Vec::new(),
        Listed::Here(ports) => ports.iter().map(|port| view_here(port, session, all)).collect(),
        Listed::There(ports) => {
            let seen = aim.and_then(|aim| session?.machine(&aim.machine));
            ports.iter().map(|port| view_there(port, seen, all)).collect()
        }
    }
}

#[cfg(test)]
mod tests;
