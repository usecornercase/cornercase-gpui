use serde::Deserialize;

use crate::cli::Version;
use crate::desktop;

pub(crate) const TUNNEL_HOLD_SINCE: Version = Version::new(0, 13, 50);
pub(crate) const ASKS_ISSUE: &str = "usecornercase/cornercase-terminal#776";
pub(crate) const ELSEWHERE_ISSUE: &str = "usecornercase/cornercase-terminal#787";

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct TunnelSeen {
    pub port: u16,
    pub local: Option<u16>,
    pub state: String,
    pub error: Option<String>,
    pub holder: u64,
    pub kind: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub(crate) struct HeldLine {
    pub port: u16,
    pub local: u16,
}

pub(crate) fn held_line(line: &str) -> Option<HeldLine> {
    let held: HeldLine = serde_json::from_str(line.trim()).ok()?;
    (held.port != 0 && held.local != 0).then_some(held)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Older {
    ThisComputer(Option<Version>),
    Server(Option<Version>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Way {
    Here,
    Waiting,
    Own(Older),
    Cornercase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Release {
    Unread,
    Read(Option<Version>),
}

pub(crate) fn way(host: Option<&str>, here: Release, server: Release) -> Way {
    if host.is_none() {
        return Way::Here;
    }
    let (Release::Read(here), Release::Read(server)) = (here, server) else { return Way::Waiting };
    if !TUNNEL_HOLD_SINCE.reached_by(here) {
        Way::Own(Older::ThisComputer(here))
    } else if !TUNNEL_HOLD_SINCE.reached_by(server) {
        Way::Own(Older::Server(server))
    } else {
        Way::Cornercase
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum Toward {
    Server,
    Machine(String),
}

impl Toward {
    pub(crate) fn key(&self) -> String {
        match self {
            Self::Server => "server".to_string(),
            Self::Machine(name) => format!("machine:{name}"),
        }
    }

    pub(crate) fn machine(&self) -> Option<&str> {
        match self {
            Self::Server => None,
            Self::Machine(name) => Some(name),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Gate {
    Waiting,
    Older(Older),
    Open,
}

pub(crate) fn machine_gate(here: Release, server: Option<Version>) -> Gate {
    let Release::Read(here) = here else { return Gate::Waiting };
    if !TUNNEL_HOLD_SINCE.reached_by(here) {
        Gate::Older(Older::ThisComputer(here))
    } else if !TUNNEL_HOLD_SINCE.reached_by(server) {
        Gate::Older(Older::Server(server))
    } else {
        Gate::Open
    }
}

fn older_words(older: &Older, machine: &str) -> (String, String) {
    let (place, found) = match older {
        Older::ThisComputer(found) => (format!("this {}", desktop::here()), found),
        Older::Server(found) => (machine.to_string(), found),
    };
    let has = match (older, found) {
        (Older::ThisComputer(_), Some(version)) => format!(" (it has {version})"),
        (Older::Server(_), Some(version)) => format!(" (its server runs {version})"),
        (_, None) => String::new(),
    };
    (place, has)
}

pub(crate) fn gate_note(older: &Older, machine: &str) -> String {
    let (place, has) = older_words(older, machine);
    format!("Tunnel needs cornercase {TUNNEL_HOLD_SINCE} or newer on {place}{has}.")
}

pub(crate) fn older_note(older: &Older, host: &str) -> String {
    let (place, has) = older_words(older, host);
    format!(
        "These tunnels are this window's own: cornercase {TUNNEL_HOLD_SINCE} or newer on {place} lists them and \
         brings them back when the window connects{has}."
    )
}

#[cfg(test)]
mod tests;
