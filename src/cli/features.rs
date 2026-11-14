use super::Version;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Feature {
    ModeDefault,
    FlowCloseWhenMerged,
    QueueResults,
    FlowFreeBuilds,
    MachineTunnels,
}

impl Feature {
    pub(crate) const fn since(self) -> Version {
        match self {
            Self::ModeDefault => Version::new(0, 13, 28),
            Self::FlowCloseWhenMerged => Version::new(0, 13, 33),
            Self::QueueResults => Version::new(0, 13, 51),
            Self::FlowFreeBuilds => Version::new(0, 13, 64),
            Self::MachineTunnels => Version::new(0, 13, 67),
        }
    }

    pub(crate) fn on(self, release: Option<Version>) -> Has {
        if self.since().reached_by(release) { Has::Yes } else { Has::No(release) }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Has {
    Unread,
    Yes,
    No(Option<Version>),
}

#[cfg(test)]
mod tests;
