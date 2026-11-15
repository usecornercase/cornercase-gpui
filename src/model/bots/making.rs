use std::collections::HashSet;
use std::time::{Duration, Instant};

use super::Bot;
use crate::cli::{Feature, Lacks, newer_here};
use crate::platform::Os;

pub(crate) const MADE_FOR: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Made {
    pub id: String,
    boot: Option<String>,
    until: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Arrival {
    Listed,
    NotYet,
    Gone,
}

impl Made {
    pub(crate) fn new(id: &str, boot: Option<&str>, now: Instant) -> Self {
        Self { id: id.to_string(), boot: boot.map(str::to_string), until: now + MADE_FOR }
    }

    pub(crate) fn arrival(&self, list: &[Bot], boot: Option<&str>, now: Instant) -> Arrival {
        let other_run = matches!((&self.boot, boot), (Some(then), Some(now)) if then != now);
        if now >= self.until || other_run {
            return Arrival::Gone;
        }
        if list.iter().any(|bot| bot.id == self.id) { Arrival::Listed } else { Arrival::NotYet }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Making {
    Offered,
    Here(String),
    Server(String),
}

pub(crate) fn making(lacks: Option<Lacks<'_>>, os: Os) -> Making {
    let since = Feature::Bots.since();
    match lacks {
        None => Making::Offered,
        Some(Lacks::ThisComputer(has)) => Making::Here(format!("Making a bot needs {}.", newer_here(since, has, os))),
        Some(Lacks::Server(release) | Lacks::Machine(_, release)) => {
            let runs = release.map(|release| format!(": the server runs {release}")).unwrap_or_default();
            Making::Server(format!("Making a bot needs cornercase {since} or newer{runs}."))
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Managing {
    pub made: Option<Made>,
    removing: HashSet<String>,
    named: Option<(String, u64)>,
    refused: Option<(u64, String)>,
}

impl Managing {
    pub(crate) fn asked_to_remove(&mut self, id: &str) {
        self.removing.insert(id.to_string());
    }

    pub(crate) fn removal_failed(&mut self, id: &str) {
        self.removing.remove(id);
    }

    pub(crate) fn asked(&self, id: &str) -> bool {
        self.removing.contains(id)
    }

    pub(crate) fn told(&self, id: &str, shown: bool) -> bool {
        shown && !self.asked(id)
    }

    pub(crate) fn keep_listed(&mut self, listed: &[String]) {
        self.removing.retain(|id| listed.contains(id));
    }

    pub(crate) fn name_dialog(&mut self, bot: &str, dialog: u64) {
        self.named = Some((bot.to_string(), dialog));
    }

    pub(crate) fn closes_dialog(&self, gone: &str, shown: Option<u64>) -> bool {
        self.named.as_ref().is_some_and(|(bot, dialog)| bot == gone && Some(*dialog) == shown)
    }

    pub(crate) fn refuse(&mut self, dialog: u64, words: String) {
        self.refused = Some((dialog, words));
    }

    pub(crate) fn refusal(&self, dialog: u64) -> Option<&str> {
        self.refused.as_ref().filter(|(shown, _)| *shown == dialog).map(|(_, words)| words.as_str())
    }

    pub(crate) fn forget_refusal(&mut self, dialog: u64) -> bool {
        let held = self.refusal(dialog).is_some();
        if held {
            self.refused = None;
        }
        held
    }
}

#[cfg(test)]
mod tests;
