use std::collections::HashMap;

use super::BotKey;

pub(crate) fn sendable(text: &str) -> bool {
    !text.trim().is_empty()
}

pub(crate) fn on_return(text: &str, on_its_way: bool) -> Option<String> {
    (sendable(text) && !on_its_way).then(|| text.to_string())
}

pub(crate) fn say_args(bot: &str) -> Vec<String> {
    ["bot", "say", bot, "--json", "-"].map(str::to_string).to_vec()
}

pub(crate) fn setup_args(bot: &str) -> Vec<String> {
    ["bot", "setup", bot, "--json"].map(str::to_string).to_vec()
}

pub(crate) fn say_key(bot: &BotKey) -> String {
    format!("bot-say:{}", bot.tag())
}

pub(crate) fn stop_key(bot: &BotKey) -> String {
    format!("bot-stop:{}", bot.tag())
}

pub(crate) fn reset_key(bot: &BotKey) -> String {
    format!("bot-reset:{}", bot.tag())
}

pub(crate) fn restored(failed: &str, typed: &str) -> String {
    if typed.is_empty() { failed.to_string() } else { format!("{failed}\n{typed}") }
}

pub(crate) fn with_example(draft: &str, example: &str) -> String {
    if sendable(draft) { format!("{draft}\n{example}") } else { example.to_string() }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Said {
    Typed,
    Again,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Goes {
    Draft,
    Toast,
    Nowhere,
}

pub(crate) fn refused_goes(said: Said, listed: bool) -> Goes {
    match (said, listed) {
        (Said::Typed, true) => Goes::Draft,
        (Said::Typed, false) => Goes::Toast,
        (Said::Again, _) => Goes::Nowhere,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Refill {
    Keep,
    Forget,
    Fill,
}

pub(crate) fn refill(owner: Option<&BotKey>, shown: Option<&BotKey>, listed: bool, seen: u64, revision: u64) -> Refill {
    match shown {
        None if owner.is_some() && !listed => Refill::Forget,
        Some(shown) if owner != Some(shown) || seen != revision => Refill::Fill,
        _ => Refill::Keep,
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Drafts {
    texts: HashMap<String, String>,
    revision: u64,
}

impl Drafts {
    pub(crate) fn keep(&mut self, bot: &str, text: String) {
        if text.is_empty() {
            self.texts.remove(bot);
        } else {
            self.texts.insert(bot.to_string(), text);
        }
    }

    pub(crate) fn text(&self, bot: &str) -> &str {
        self.texts.get(bot).map_or("", String::as_str)
    }

    pub(crate) fn put_back(&mut self, bot: &str, failed: &str) {
        let text = restored(failed, self.text(bot));
        self.keep(bot, text);
        self.revision += 1;
    }

    pub(crate) fn add_example(&mut self, bot: &str, example: &str) {
        let text = with_example(self.text(bot), example);
        self.keep(bot, text);
        self.revision += 1;
    }

    pub(crate) fn keep_only(&mut self, listed: &[String]) {
        self.texts.retain(|bot, _| listed.contains(bot));
    }

    pub(crate) fn take(&mut self, bot: &str) -> Option<String> {
        self.texts.remove(bot)
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }
}

#[cfg(test)]
mod tests;
