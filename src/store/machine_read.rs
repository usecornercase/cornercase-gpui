use crate::cli::{self, Target};
use crate::model::machines::{LinkState, Machine, MachineIcon};
use crate::model::{Aim, MachinePlace, MachineSeen};
use crate::platform::paths;

use super::Store;
use super::machines_there::machine_target;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MachineRead {
    pub aim: Aim,
    pub icon: MachineIcon,
    pub place: String,
    pub path: String,
    pub folder: String,
    pub reach: Reach,
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
    MachineRead {
        aim: place.aim(),
        icon: seen.icon,
        place: listed.unwrap_or_else(|| place.name().to_string()),
        path: place.path().to_string(),
        folder: home.map_or_else(|| place.path().to_string(), |home| paths::home_relative(place.path(), home)),
        reach: reach(window, list, seen),
    }
}

impl Store {
    pub(crate) fn machine_read(&self) -> Option<MachineRead> {
        let place = self.machine_place.as_ref()?;
        let seen = self.session.as_ref()?.machine(&place.machine)?;
        Some(read_of(place, seen, &cli::target(), &self.machines, self.machine_home(&place.boot)))
    }
}

#[cfg(test)]
mod tests;
