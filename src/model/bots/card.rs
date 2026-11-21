use anyhow::bail;
use serde_json::Value;

use super::{Bot, access_words, set};
use crate::cli::{Feature, Lacks, newer_here};
use crate::panels::flows::model::plural;
use crate::panels::queue::{Entry, repeat_words};
use crate::platform::Os;

const THE_AGENTS_OWN: &str = "the agent's own";
const UNREADABLE: &str = "secrets cannot be read";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tone {
    Plain,
    Danger,
    Waiting,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CardRow {
    pub label: &'static str,
    pub value: String,
    pub tone: Tone,
}

fn row(label: &'static str, value: String, tone: Tone) -> CardRow {
    CardRow { label, value, tone }
}

pub(crate) fn secrets_words(count: usize, unreadable: bool) -> Option<(String, bool)> {
    if unreadable {
        return Some((UNREADABLE.to_string(), true));
    }
    (count > 0).then(|| (plural(count, "secret"), false))
}

fn access_row(access: &str) -> Option<CardRow> {
    match access_words(access) {
        Some((words, full)) => Some(row("Access", words.to_string(), if full { Tone::Danger } else { Tone::Plain })),
        None => (!access.is_empty()).then(|| row("Access", access.to_string(), Tone::Plain)),
    }
}

pub(crate) fn card_rows(bot: &Bot, made: Option<String>) -> Vec<CardRow> {
    let plain = |label, value: Option<String>| value.map(|value| row(label, value, Tone::Plain));
    let model = set(bot.model.as_ref()).unwrap_or_else(|| THE_AGENTS_OWN.to_string());
    let secrets = secrets_words(bot.secrets, bot.secrets_unreadable)
        .map(|(words, unreadable)| row("Secrets", words, if unreadable { Tone::Danger } else { Tone::Plain }));
    let asks = bot.asks.iter().map(|ask| row("Asks", format!("for a secret: {}", ask.joined()), Tone::Waiting));
    let mut rows: Vec<CardRow> = [
        plain("Agent", Some(bot.agent.clone()).filter(|agent| !agent.is_empty())),
        Some(row("Model", model, Tone::Plain)),
        plain("Effort", set(bot.effort.as_ref())),
        plain("Account", set(bot.account.as_ref())),
        access_row(&bot.access),
        secrets,
    ]
    .into_iter()
    .flatten()
    .collect();
    rows.extend(asks);
    rows.extend(plain("Made", made));
    rows
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ListRow {
    pub id: u64,
    pub name: String,
    pub detail: String,
}

pub(crate) fn routine_rows(entries: &[Entry], bot: &str, next: impl Fn(i64) -> String) -> Vec<ListRow> {
    entries
        .iter()
        .filter(|entry| entry.repeat.is_some() && entry.place.bot.as_deref() == Some(bot))
        .map(|entry| {
            let first = if entry.paused {
                "paused".to_string()
            } else {
                format!("next {}", next(entry.next.unwrap_or(entry.at)))
            };
            let detail = format!("{first} · ↻ {}", repeat_words(entry));
            ListRow { id: entry.id, name: entry.label.clone(), detail }
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AboutGate {
    Hidden,
    Lacks(String),
    Read,
}

pub(crate) fn about_gate(lacks: Option<Lacks<'_>>, os: Os) -> AboutGate {
    match lacks {
        None => AboutGate::Read,
        Some(Lacks::ThisComputer(has)) => {
            AboutGate::Lacks(format!("Needs {}.", newer_here(Feature::BotAboutRead.since(), has, os)))
        }
        Some(Lacks::Server(_) | Lacks::Machine(..)) => AboutGate::Hidden,
    }
}

pub(crate) fn about_args(bot: &str) -> Vec<String> {
    ["bot", "about", "--json", "--", bot].map(str::to_string).to_vec()
}

pub(crate) fn about_of(out: &str) -> anyhow::Result<String> {
    let found: Value = serde_json::from_str(out)?;
    match found.get("about") {
        Some(Value::String(about)) => Ok(about.clone()),
        _ => bail!("cornercase did not say what the bot is for"),
    }
}

pub(crate) fn about_failed(why: &str) -> String {
    format!("Could not read what it is for: {why}")
}

#[cfg(test)]
mod tests;
