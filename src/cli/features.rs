use super::Version;
use crate::desktop;
use crate::platform::Os;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Feature {
    ModeDefault,
    FlowCloseWhenMerged,
    QueueResults,
    FlowFreeBuilds,
    MachineTunnels,
    Bots,
    BotSetup,
}

impl Feature {
    pub(crate) const fn since(self) -> Version {
        match self {
            Self::ModeDefault => Version::new(0, 13, 28),
            Self::FlowCloseWhenMerged => Version::new(0, 13, 33),
            Self::QueueResults => Version::new(0, 13, 51),
            Self::FlowFreeBuilds => Version::new(0, 13, 64),
            Self::MachineTunnels => Version::new(0, 13, 67),
            Self::Bots => Version::new(0, 13, 77),
            Self::BotSetup => Version::new(0, 13, 89),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Release {
    Unread,
    Read(Option<Version>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Releases<'a> {
    pub server: Option<Version>,
    pub here: Release,
    pub machine: Option<(&'a str, Option<Version>)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Lacks<'a> {
    Server(Option<Version>),
    ThisComputer(Option<Version>),
    Machine(&'a str, Option<Version>),
}

impl<'a> Releases<'a> {
    pub(crate) fn lacks(&self, feature: Feature) -> Option<Lacks<'a>> {
        if feature.on(self.server) != Has::Yes {
            return Some(Lacks::Server(self.server));
        }
        if let Release::Read(here) = self.here
            && !feature.since().reached_by(here)
        {
            return Some(Lacks::ThisComputer(here));
        }
        self.machine
            .filter(|(_, release)| feature.on(*release) != Has::Yes)
            .map(|(machine, release)| Lacks::Machine(machine, release))
    }
}

pub(crate) fn it_has(release: Option<Version>) -> String {
    release.map(|release| format!(" (it has {release})")).unwrap_or_default()
}

pub(crate) fn newer(since: Version, lacks: Lacks<'_>, os: Os) -> String {
    let (place, who, release) = match lacks {
        Lacks::ThisComputer(release) => return newer_here(since, release, os),
        Lacks::Server(release) => (String::new(), "the server", release),
        Lacks::Machine(machine, release) => (format!(" on {machine}"), "it", release),
    };
    let runs = release.map(|release| format!(": {who} runs {release}")).unwrap_or_default();
    format!("cornercase {since} or newer{place}{runs}")
}

pub(crate) fn newer_here(since: Version, release: Option<Version>, os: Os) -> String {
    format!("cornercase {since} or newer on this {}{}", desktop::machine(os), it_has(release))
}

#[cfg(test)]
mod tests;
