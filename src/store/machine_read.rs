use gpui::Context;

use crate::cli::{self, Target};
use crate::model::machines::{LinkState, Machine, MachineIcon, NextTry};
use crate::model::{Aim, MachinePlace, MachineSeen};
use crate::platform::paths;

use super::Store;
use super::machines_there::{CliRelease, Homes, machine_target};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Reach {
    Up(Target),
    Down(String),
    Refused(String),
}

impl Reach {
    pub(crate) fn target(&self) -> Option<&Target> {
        match self {
            Self::Up(target) => Some(target),
            Self::Down(_) | Self::Refused(_) => None,
        }
    }

    pub(crate) fn why_not(&self) -> Option<&str> {
        match self {
            Self::Up(_) => None,
            Self::Down(why) | Self::Refused(why) => Some(why),
        }
    }
}

#[derive(Default)]
pub(super) struct ThereState {
    pub(super) homes: Homes,
    pub(super) release: CliRelease,
    pub(super) tab: Option<TabThere>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AgentReach {
    Ready(Target),
    Down(String),
    Gone(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TabThere {
    pub machine: String,
    pub boot: String,
    pub target: Target,
    pub down: Option<String>,
    pub agents: Vec<u64>,
}

fn reach_agent(tab: Option<&TabThere>, machine: &str, boot: &str, pane: u64) -> AgentReach {
    let Some(tab) = tab.filter(|tab| tab.machine == machine) else {
        return AgentReach::Gone(format!("The tab on {machine} is no longer shown"));
    };
    if tab.boot != boot {
        return AgentReach::Gone(format!("{machine} restarted"));
    }
    if let Some(why) = &tab.down {
        return AgentReach::Down(why.clone());
    }
    if tab.agents.contains(&pane) {
        return AgentReach::Ready(tab.target.clone());
    }
    AgentReach::Gone(format!("that agent is no longer on {machine}"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MachineRead {
    pub aim: Aim,
    pub destination: String,
    pub icon: MachineIcon,
    pub place: String,
    pub path: String,
    pub folder: String,
    pub reach: Reach,
    pub retry_at: Option<String>,
}

impl MachineRead {
    pub(crate) fn next_try(&self, now: i64) -> Option<NextTry> {
        NextTry::at(self.retry_at.as_deref(), now)
    }
}

fn reach(window: &Target, list: &[Machine], seen: &MachineSeen) -> Reach {
    let comes_back = matches!(seen.state, LinkState::Unreachable | LinkState::Connecting);
    match machine_target(window, list, seen) {
        Ok(target) => Reach::Up(target),
        Err(why) if comes_back && window.host().is_none() => Reach::Down(why),
        Err(why) => Reach::Refused(why),
    }
}

fn read_of(
    place: &MachinePlace,
    seen: &MachineSeen,
    window: &Target,
    list: &[Machine],
    home: Option<&str>,
) -> MachineRead {
    let listed = seen.place_name(place.workspace).filter(|_| seen.boot == place.boot);
    let reached_at = list.iter().find(|machine| machine.name == seen.name);
    MachineRead {
        aim: place.aim(),
        destination: reached_at.map_or_else(String::new, |machine| machine.destination.clone()),
        icon: seen.icon,
        place: listed.unwrap_or_else(|| place.name().to_string()),
        path: place.path().to_string(),
        folder: home.map_or_else(|| place.path().to_string(), |home| paths::home_relative(place.path(), home)),
        reach: reach(window, list, seen),
        retry_at: seen.retry_at.clone(),
    }
}

impl Store {
    pub(crate) fn set_tab_there(&mut self, tab: Option<TabThere>, cx: &mut Context<Self>) {
        if self.there.tab != tab {
            self.there.tab = tab;
            cx.notify();
        }
    }

    pub(crate) fn agent_reach(&self, machine: &str, boot: &str, pane: u64) -> AgentReach {
        reach_agent(self.there.tab.as_ref(), machine, boot, pane)
    }

    pub(crate) fn machine_read(&self) -> Option<MachineRead> {
        let place = self.machine_place.as_ref()?;
        let seen = self.session.as_ref()?.machine(&place.machine)?;
        Some(read_of(place, seen, &cli::target(), &self.machines, self.machine_home(&place.machine, &place.boot)))
    }
}

#[cfg(test)]
mod tests;
