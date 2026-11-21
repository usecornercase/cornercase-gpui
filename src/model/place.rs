use crate::model::machines::seen::MachinePlace;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) enum Showing {
    #[default]
    Local,
    Machine(MachinePlace),
    Bot(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Moved {
    Local,
    Machine(MachinePlace),
    Bot(String),
    Removed(String),
    Listed(Vec<String>),
}

impl Showing {
    pub(crate) fn machine(&self) -> Option<&MachinePlace> {
        match self {
            Self::Machine(place) => Some(place),
            _ => None,
        }
    }

    pub(crate) fn bot(&self) -> Option<&str> {
        match self {
            Self::Bot(id) => Some(id),
            _ => None,
        }
    }

    pub(crate) fn after(self, moved: Moved) -> Self {
        match (moved, self) {
            (Moved::Local, _) => Self::Local,
            (Moved::Machine(place), _) => Self::Machine(place),
            (Moved::Bot(id), _) => Self::Bot(id),
            (Moved::Removed(gone), Self::Bot(id)) if id == gone => Self::Local,
            (Moved::Listed(ids), Self::Bot(id)) if !ids.contains(&id) => Self::Local,
            (_, kept) => kept,
        }
    }
}

pub(crate) fn awaits(select: bool, asked: Option<u64>, shown: Option<u64>) -> bool {
    select && asked.is_some() && asked == shown
}

#[cfg(test)]
mod tests;
