use anyhow::bail;
use serde::Deserialize;
use serde_json::Value;

use crate::cli::{Feature, Lacks, newer_here};
use crate::platform::Os;

pub(crate) const NAME_HINT: &str = "a-z, 0-9, - and _, such as campus-password";
pub(crate) const SAVED_AT_ONCE: &str = "A secret is saved or removed at once, not with Save.";
pub(crate) const NEW_SECRET: &str = "New secret";
pub(crate) const SECRET_VALUE: &str = "Secret value";
pub(crate) const REMOVE_SECRET: &str = "Remove secret";
const NAME_MAX: usize = 40;
const READS_HERE: &str =
    "The bot reads this value: it reaches the bot's agent and stays in that agent's own records on this computer.";
const READS_THERE: &str = "The bot reads this value: it reaches the bot's agent and stays in that agent's own records on the computer that runs cornercase.";

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Secret {
    pub name: String,
    pub saved: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Listed {
    pub key: String,
    pub secrets: Vec<Secret>,
}

impl Listed {
    pub(crate) fn names(&self) -> Vec<String> {
        self.secrets.iter().map(|secret| secret.name.clone()).collect()
    }
}

pub(crate) fn listed_of(text: &str) -> anyhow::Result<Listed> {
    let found: Value = serde_json::from_str(text)?;
    let Some(Value::Array(entries)) = found.get("secrets") else { bail!("cornercase listed no secrets") };
    let secrets = entries
        .iter()
        .filter_map(|entry| Secret::deserialize(entry).ok())
        .filter(|secret| !secret.name.is_empty())
        .collect();
    let key = found.get("key").and_then(Value::as_str).unwrap_or_default().to_string();
    Ok(Listed { key, secrets })
}

fn secret_args(command: &str, names: &[&str]) -> Vec<String> {
    let mut args: Vec<String> = ["bot", "secret", command, "--json", "--"].map(str::to_string).to_vec();
    args.extend(names.iter().map(|name| (*name).to_string()));
    args
}

pub(crate) fn secret_list_args(bot: &str) -> Vec<String> {
    secret_args("list", &[bot])
}

pub(crate) fn secret_set_args(bot: &str, name: &str) -> Vec<String> {
    secret_args("set", &[bot, name])
}

pub(crate) fn secret_remove_args(bot: &str, name: &str) -> Vec<String> {
    secret_args("remove", &[bot, name])
}

pub(crate) fn name_refusal(name: &str) -> Option<String> {
    let length = name.chars().count();
    if length == 0 || length > NAME_MAX {
        return Some(format!("A secret's name takes 1 to {NAME_MAX} characters, not {length}."));
    }
    let allowed = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_';
    (!name.chars().all(allowed)).then(|| "A secret's name takes only a-z, 0-9, - and _.".to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SecretEnter {
    FocusName,
    Refused(String),
    FocusValue,
    Save,
}

pub(crate) fn on_secret_enter(name: &str, value_empty: bool) -> SecretEnter {
    if name.is_empty() {
        return SecretEnter::FocusName;
    }
    if let Some(words) = name_refusal(name) {
        return SecretEnter::Refused(words);
    }
    if value_empty { SecretEnter::FocusValue } else { SecretEnter::Save }
}

pub(crate) fn replaces(name: &str, saved: &[String]) -> bool {
    saved.iter().any(|kept| kept == name)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SecretsSection {
    Hidden,
    Lacks(String),
    Shown,
}

pub(crate) fn secrets_section(lacks: Option<Lacks<'_>>, os: Os) -> SecretsSection {
    match lacks {
        None => SecretsSection::Shown,
        Some(Lacks::ThisComputer(has)) => {
            SecretsSection::Lacks(format!("Needs {}.", newer_here(Feature::BotSecrets.since(), has, os)))
        }
        Some(Lacks::Server(_) | Lacks::Machine(..)) => SecretsSection::Hidden,
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct SecretsSeen {
    pub saved: u64,
    pub count: usize,
    pub unreadable: bool,
}

pub(crate) fn rereads(listed: Option<usize>, seen: SecretsSeen, now: SecretsSeen) -> bool {
    let moved = (now.count, now.unreadable) != (seen.count, seen.unreadable);
    now.saved != seen.saved || (moved && (now.unreadable || listed != Some(now.count)))
}

pub(crate) fn what_a_secret_is(here: bool) -> &'static str {
    if here { READS_HERE } else { READS_THERE }
}

pub(crate) fn empty_words(bot: &str) -> String {
    format!("{bot} has no secrets yet. Add… saves a password or a token only {bot} can read.")
}

pub(crate) fn remove_secret_message(secret: &str, bot: &str) -> String {
    format!("Remove {secret} from {bot}? The bot can no longer read it.")
}

pub(crate) fn secrets_read_failed(words: &str) -> String {
    format!("Could not read the secrets: {words}")
}

pub(crate) fn secret_save_failed(words: &str) -> String {
    format!("Could not save the secret: {words}")
}

pub(crate) fn secret_remove_failed(secret: &str, bot: &str, words: &str) -> String {
    format!("Could not remove {secret} from {bot}: {words}")
}

#[cfg(test)]
mod tests;
