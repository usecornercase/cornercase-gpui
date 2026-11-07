use std::rc::Rc;

use serde::Deserialize;

use crate::cli;
use crate::panels::settings::widgets::capitalised;
use crate::store;
use crate::ui::qr::Matrix;

const NOT_PAIRED: &str = "no phone was paired";
const OWN_NAME: &str = "cornercase";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PairKey {
    Browsers,
    Telegram,
    Discord,
}

impl PairKey {
    pub(crate) fn of(key: &str) -> Option<Self> {
        match key {
            "browsers" => Some(Self::Browsers),
            "telegram.chat" => Some(Self::Telegram),
            "discord.chat" => Some(Self::Discord),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Browsers => "browsers",
            Self::Telegram => "telegram.chat",
            Self::Discord => "discord.chat",
        }
    }

    pub(super) fn title(self) -> &'static str {
        match self {
            Self::Browsers => "Remote",
            Self::Telegram => "Telegram",
            Self::Discord => "Discord",
        }
    }

    pub(super) fn args(self) -> Vec<String> {
        store::args(&["config", "pair", "--json", "--", self.name()])
    }

    fn connected(self, chat: Option<&str>) -> String {
        match (self, chat.filter(|name| !name.is_empty())) {
            (Self::Browsers, _) => "A browser connected".to_string(),
            (_, Some(name)) => format!("{} connected with {name}", self.title()),
            (_, None) => format!("{} connected", self.title()),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
struct Shown {
    url: Option<String>,
    lines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct Ended {
    paired: bool,
    #[serde(default)]
    chat: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
enum Line {
    Ended(Ended),
    Shown(Shown),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Linked {
    pub url: String,
    pub code: Option<Rc<Matrix>>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Reading {
    Asking,
    Starting(Vec<String>),
    Showing(Linked),
    Refused(String),
}

fn sentence(line: &str) -> String {
    if line.starts_with(OWN_NAME) { line.to_string() } else { capitalised(line) }
}

fn spoken(lines: &[String]) -> Vec<String> {
    lines.iter().map(|line| sentence(line)).collect()
}

fn showing(shown: Shown) -> Option<Reading> {
    let notes = spoken(&shown.lines);
    match shown.url {
        Some(url) => Some(Reading::Showing(Linked { code: Matrix::of(&url).map(Rc::new), url, notes })),
        None if notes.is_empty() => None,
        None => Some(Reading::Starting(notes)),
    }
}

fn refused(words: &str) -> Reading {
    let words = words.trim();
    Reading::Refused(sentence(if words.is_empty() { NOT_PAIRED } else { words }))
}

impl Reading {
    pub(super) fn line(&mut self, key: PairKey, text: &str) -> Option<String> {
        match serde_json::from_str::<Line>(text).ok()? {
            Line::Ended(Ended { paired: true, chat, .. }) => return Some(key.connected(chat.as_deref())),
            Line::Ended(Ended { error, .. }) => *self = refused(error.as_deref().unwrap_or_default()),
            Line::Shown(shown) => {
                if let Some(reading) = showing(shown) {
                    *self = reading;
                }
            }
        }
        None
    }

    pub(super) fn ended(&mut self, stderr: &str) {
        if !matches!(self, Self::Refused(_)) {
            *self = refused(&cli::reason(&anyhow::Error::msg(stderr.trim().to_string())));
        }
    }
}

#[cfg(test)]
mod tests;
