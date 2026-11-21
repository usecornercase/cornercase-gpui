use crate::model::bots::{BotKey, Home};
use crate::model::machines::seen::MachinePlace;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) enum Showing {
    #[default]
    Local,
    Machine(MachinePlace),
    Bot(BotKey),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Moved {
    Local,
    Machine(MachinePlace),
    Bot(BotKey),
    Removed(BotKey),
    Listed(Home, Vec<String>),
}

impl Showing {
    pub(crate) fn machine(&self) -> Option<&MachinePlace> {
        match self {
            Self::Machine(place) => Some(place),
            _ => None,
        }
    }

    pub(crate) fn bot(&self) -> Option<&BotKey> {
        match self {
            Self::Bot(key) => Some(key),
            _ => None,
        }
    }

    pub(crate) fn after(self, moved: Moved) -> Self {
        match (moved, self) {
            (Moved::Local, _) => Self::Local,
            (Moved::Machine(place), _) => Self::Machine(place),
            (Moved::Bot(key), _) => Self::Bot(key),
            (Moved::Removed(gone), Self::Bot(key)) if key == gone => Self::Local,
            (Moved::Listed(home, ids), Self::Bot(key)) if key.home == home && !ids.contains(&key.id) => Self::Local,
            (_, kept) => kept,
        }
    }
}

pub(crate) fn awaits(select: bool, asked: Option<u64>, shown: Option<u64>) -> bool {
    select && asked.is_some() && asked == shown
}

#[cfg(test)]
mod tests;
