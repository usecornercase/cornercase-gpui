use std::collections::BTreeSet;

use super::held::Hold;
use crate::cli::Target;
use crate::model::MachineSeen;
use crate::model::machines::Machine;
use crate::model::tunnels::{Gate, Toward, Way};
use crate::store::Reach;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MachineLink {
    Up { boot: String, destination: String },
    Waiting(Option<String>),
    Older(String),
    Lost,
}

impl MachineLink {
    pub(crate) fn of(listed: Option<&Machine>, known: bool, seen: Option<&MachineSeen>, reach: Option<&Reach>) -> Self {
        let Some(listed) = listed else { return if known { Self::Lost } else { Self::Waiting(None) } };
        let (Some(seen), Some(reach)) = (seen, reach) else { return Self::Lost };
        match reach {
            Reach::Refused(_) => Self::Lost,
            Reach::Down(_) | Reach::Waiting(_) => Self::Waiting(Some(listed.destination.clone())),
            Reach::Older(_) => Self::Older(listed.destination.clone()),
            Reach::Up(_) => Self::Up { boot: seen.boot.clone(), destination: listed.destination.clone() },
        }
    }

    pub(crate) fn gated(self, gate: Gate) -> Self {
        let (Self::Up { destination, .. } | Self::Older(destination)) = &self else { return self };
        match gate {
            Gate::Open => self,
            gate if gate.known_older() => Self::Lost,
            Gate::Waiting | Gate::Older(_) if matches!(self, Self::Older(_)) => self,
            Gate::Waiting | Gate::Older(_) => Self::Waiting(Some(destination.clone())),
        }
    }

    pub(crate) fn stays_for(&self, destination: &str) -> bool {
        let same = |listed: &str| destination.is_empty() || listed == destination;
        match self {
            Self::Lost => false,
            Self::Up { destination: listed, .. } | Self::Older(listed) => same(listed),
            Self::Waiting(listed) => listed.as_deref().is_none_or(same),
        }
    }

    pub(crate) fn keeps_taker(&self, destination: &str) -> bool {
        !matches!(self, Self::Older(_)) && self.stays_for(destination)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Linking<'a> {
    Link { boot: &'a str, destination: &'a str },
    Stay,
    Pause,
    Lose,
}

pub(crate) fn linking<'a>(link: &'a MachineLink, linked: Option<&str>) -> Linking<'a> {
    match link {
        MachineLink::Up { boot, .. } if linked == Some(boot.as_str()) => Linking::Stay,
        MachineLink::Up { boot, destination } => Linking::Link { boot, destination },
        MachineLink::Waiting(_) | MachineLink::Older(_) => Linking::Pause,
        MachineLink::Lost => Linking::Lose,
    }
}

pub(crate) fn tunnel_target(reach: Reach) -> Result<Target, String> {
    match reach {
        Reach::Up(target) => Ok(target),
        Reach::Older(note) => Err(note.tunnels),
        Reach::Waiting(why) | Reach::Down(why) | Reach::Refused(why) => Err(why),
    }
}

pub(crate) fn followed<'a>(
    names: impl Iterator<Item = String>,
    towards: impl Iterator<Item = &'a Toward>,
) -> BTreeSet<String> {
    names.chain(towards.filter_map(|toward| toward.machine().map(str::to_string))).collect()
}

pub(crate) fn split_ended(holds: Vec<Hold>, way: &Way) -> (Vec<Hold>, Vec<Hold>) {
    holds.into_iter().partition(|hold| hold.toward == Toward::Server && *way != Way::Cornercase)
}

pub(crate) fn split_stale(holds: Vec<Hold>, toward: &Toward, link: &MachineLink) -> (Vec<Hold>, Vec<Hold>) {
    holds.into_iter().partition(|hold| hold.toward == *toward && !link.stays_for(&hold.destination))
}

#[cfg(test)]
mod tests;
