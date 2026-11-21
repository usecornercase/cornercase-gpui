use std::collections::HashSet;
use std::time::Instant;

use super::Bot;
use crate::cli::{Feature, Lacks, newer};
use crate::model::machines::run_on::AWAITED_FOR;
use crate::platform::Os;

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
        Self { id: id.to_string(), boot: boot.map(str::to_string), until: now + AWAITED_FOR }
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
    let Some(lacks) = lacks else { return Making::Offered };
    let words = format!("Making a bot needs {}.", newer(Feature::Bots.since(), lacks, os));
    match lacks {
        Lacks::ThisComputer(_) => Making::Here(words),
        Lacks::Server(_) | Lacks::Machine(..) => Making::Server(words),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Creating {
    pub dialog: u64,
    pub name: String,
    boot: Option<String>,
}

impl Creating {
    pub(crate) fn new(dialog: u64, name: &str, boot: Option<&str>) -> Self {
        Self { dialog, name: name.to_string(), boot: boot.map(str::to_string) }
    }

    pub(crate) fn made(&self, dialog_shown: bool, id: &str, now: Instant) -> Option<Made> {
        dialog_shown.then(|| Made::new(id, self.boot.as_deref(), now))
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Managing {
    pub made: Option<Made>,
    removing: HashSet<String>,
    named: Option<(String, u64)>,
    refused: Option<(u64, String)>,
    secrets_saved: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Closes {
    Nothing,
    Shown,
    Both,
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

    pub(crate) fn person_moved(&mut self) {
        self.made = None;
    }

    fn closes(&self, gone: impl Fn(&str) -> bool, shown: Option<u64>, behind: Option<u64>) -> Closes {
        let Some((_, dialog)) = self.named.as_ref().filter(|(bot, _)| gone(bot)) else { return Closes::Nothing };
        if Some(*dialog) == shown {
            Closes::Shown
        } else if Some(*dialog) == behind {
            Closes::Both
        } else {
            Closes::Nothing
        }
    }

    pub(crate) fn gone_closes(&self, listed: &[String], shown: Option<u64>, behind: Option<u64>) -> Closes {
        self.closes(|bot| !listed.iter().any(|id| id == bot), shown, behind)
    }

    pub(crate) fn keep_listed(&mut self, listed: &[String]) {
        self.removing.retain(|id| listed.contains(id));
    }

    pub(crate) fn name_dialog(&mut self, bot: &str, dialog: u64) {
        self.named = Some((bot.to_string(), dialog));
    }

    pub(crate) fn closes_dialog(&self, gone: &str, shown: Option<u64>, behind: Option<u64>) -> Closes {
        self.closes(|bot| bot == gone, shown, behind)
    }

    pub(crate) fn secret_saved(&mut self) {
        self.secrets_saved += 1;
    }

    pub(crate) fn secrets_saved(&self) -> u64 {
        self.secrets_saved
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

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MenuAsk<P> {
    asked: Option<(String, P)>,
}

impl<P> Default for MenuAsk<P> {
    fn default() -> Self {
        Self { asked: None }
    }
}

impl<P> MenuAsk<P> {
    pub(crate) fn ask(&mut self, bot: &str, at: P) {
        self.asked = Some((bot.to_string(), at));
    }

    pub(crate) fn take(&mut self, list: &[Bot]) -> Option<(Bot, P)> {
        let (id, at) = self.asked.take()?;
        list.iter().find(|bot| bot.id == id).map(|bot| (bot.clone(), at))
    }
}

pub(crate) fn create_failed(name: &str, words: &str) -> String {
    format!("Could not create the bot {name}: {words}")
}

pub(crate) fn remove_failed(name: &str, words: &str) -> String {
    format!("Could not remove {name}: {words}")
}

pub(crate) fn save_failed(name: &str, words: &str) -> String {
    format!("Could not save {name}: {words}")
}

pub(crate) fn rename_failed(name: &str, words: &str) -> String {
    format!("Could not rename {name}: {words}")
}

#[cfg(test)]
mod tests;
