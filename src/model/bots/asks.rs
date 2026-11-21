use std::collections::HashSet;

use serde::Deserialize;
use serde_json::{Map, Value};

use super::{Bot, CORNERCASE, FULL};
use crate::cli::{Feature, Lacks, newer_here};
use crate::platform::Os;

pub(crate) const ASKS_FOR_A_SECRET: &str = "asks for a secret";
pub(crate) const IT_SAYS: &str = "It says:";
pub(crate) const REPLACES: &str = "replaces the saved one";
pub(crate) const ONE_LINE: &str =
    "This field takes one line of text with no tab: pipe the value in with cornercase bot secret set.";
const KEPT_HERE: &str = "The bot will read these values. They are kept encrypted on this computer, for this bot only.";
const KEPT_THERE: &str =
    "The bot will read these values. They are kept encrypted on the computer that runs cornercase, for this bot only.";
const THROUGH_CORNERCASE: &str = "through cornercase";
const FULL_ACCESS: &str = "full access";

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Ask {
    pub id: u64,
    pub names: Vec<String>,
    pub about: String,
    pub replace: bool,
    pub routine: Option<u64>,
    pub missing: Vec<String>,
}

impl Ask {
    pub(crate) fn fields(&self) -> &[String] {
        if self.missing.is_empty() { &self.names } else { &self.missing }
    }

    pub(crate) fn joined(&self) -> String {
        self.names.join(", ")
    }
}

pub(super) fn listed<'de, D: serde::Deserializer<'de>>(found: D) -> Result<Vec<Ask>, D::Error> {
    let Value::Array(entries) = Value::deserialize(found)? else { return Ok(Vec::new()) };
    let read = entries.iter().filter_map(|entry| Ask::deserialize(entry).ok());
    Ok(read.filter(|ask| ask.id > 0 && !ask.names.is_empty()).collect())
}

pub(crate) fn title(name: &str) -> String {
    format!("{name} {ASKS_FOR_A_SECRET}")
}

pub(crate) fn fields_after(have: &[String], ask: &Ask) -> Vec<String> {
    let missing = ask.fields();
    ask.names.iter().filter(|name| have.contains(name) || missing.contains(name)).cloned().collect()
}

pub(crate) fn give_json(ask: &Ask, typed: &[(String, String)]) -> Option<String> {
    let mut values = Map::new();
    for (name, value) in typed.iter().filter(|(name, _)| ask.names.contains(name)) {
        if value.is_empty() {
            return None;
        }
        values.insert(name.clone(), Value::String(value.clone()));
    }
    let whole = ask.fields().iter().all(|name| values.contains_key(name));
    whole.then(|| Value::Object(values).to_string())
}

fn secret_args(verb: &str, bot: &str, ask: u64) -> Vec<String> {
    ["bot", "secret", verb, bot, &ask.to_string(), "--json"].map(str::to_string).to_vec()
}

pub(crate) fn give_args(bot: &str, ask: u64) -> Vec<String> {
    secret_args("give", bot, ask)
}

pub(crate) fn decline_args(bot: &str, ask: u64) -> Vec<String> {
    secret_args("decline", bot, ask)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Enter {
    Save,
    Focus(usize),
}

pub(crate) fn on_enter(empty: &[bool]) -> Enter {
    empty.iter().position(|empty| *empty).map_or(Enter::Save, Enter::Focus)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum By {
    Itself,
    Person,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Keys {
    EmptyField,
    Draft,
    Elsewhere,
}

pub(crate) fn keys_of(in_field: bool, blank: bool) -> Keys {
    match (in_field, blank) {
        (true, true) => Keys::EmptyField,
        (true, false) => Keys::Draft,
        (false, _) => Keys::Elsewhere,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Here {
    pub shown: bool,
    pub active: bool,
    pub covered: bool,
    pub keys: Keys,
}

pub(crate) fn opens(by: By, here: Here, dialog_open: bool) -> bool {
    let settled = here.shown && here.active && !here.covered && here.keys == Keys::EmptyField;
    !dialog_open && (by == By::Person || settled)
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Known(Option<HashSet<u64>>);

impl Known {
    pub(crate) fn arrived(&mut self, list: &[Bot]) -> Vec<(String, u64)> {
        let now: Vec<(String, u64)> =
            list.iter().flat_map(|bot| bot.asks.iter().map(|ask| (bot.id.clone(), ask.id))).collect();
        let arrived = match &self.0 {
            None => Vec::new(),
            Some(known) => now.iter().filter(|(_, id)| !known.contains(id)).cloned().collect(),
        };
        self.0 = Some(now.iter().map(|(_, id)| *id).collect());
        arrived
    }
}

pub(crate) fn counted_as(was: usize, is: usize, arrived: usize) -> (usize, usize) {
    let rise = is.saturating_sub(was);
    (was + arrived.min(rise), is)
}

pub(crate) fn tells_ask(bot: &str, (shown, active): (Option<&str>, bool), offered: bool) -> bool {
    offered && !(active && shown == Some(bot))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AskOpen {
    pub bot: String,
    pub ask: u64,
    pub dialog: u64,
}

impl AskOpen {
    pub(crate) fn ended(&self, list: &[Bot], shown: Option<u64>) -> bool {
        let waits =
            list.iter().find(|bot| bot.id == self.bot).is_some_and(|bot| bot.asks.iter().any(|a| a.id == self.ask));
        shown == Some(self.dialog) && !waits
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Asking {
    pub known: Known,
    pub open: Option<AskOpen>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Give {
    Offered,
    Here(String),
    No,
}

pub(crate) fn giving(lacks: Option<Lacks<'_>>, os: Os) -> Give {
    match lacks {
        None => Give::Offered,
        Some(Lacks::ThisComputer(has)) => {
            Give::Here(format!("Giving it needs {}.", newer_here(Feature::BotAsks.since(), has, os)))
        }
        Some(Lacks::Server(_) | Lacks::Machine(..)) => Give::No,
    }
}

pub(crate) fn access_words(access: &str) -> Option<(&'static str, bool)> {
    match access {
        CORNERCASE => Some((THROUGH_CORNERCASE, false)),
        FULL => Some((FULL_ACCESS, true)),
        _ => None,
    }
}

pub(crate) fn what_happens(here: bool) -> &'static str {
    if here { KEPT_HERE } else { KEPT_THERE }
}

#[cfg(test)]
mod tests;
