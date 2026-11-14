use super::Version;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Feature {
    MachineTunnels,
}

impl Feature {
    pub(crate) const fn since(self) -> Version {
        match self {
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
