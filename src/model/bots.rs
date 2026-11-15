use std::collections::{HashMap, HashSet};

use serde::Deserialize;
use serde_json::Value;

use crate::cli::{Feature, Lacks, newer_here};
use crate::platform::Os;

pub(crate) use self::form::{
    AgentInfo, CORNERCASE, Editing, FULL, NotSent, Save, Typed, agents_of, chosen_at_open, id_of, kept_accounts,
    new_args, rename_args,
};
pub(crate) use self::manage::{
    Arrival, Creating, Making, Managing, MenuAsk, create_failed, making, remove_failed, rename_failed, save_failed,
};
pub(crate) use self::talk::{
    Drafts, Goes, Refill, Said, on_return, refill, refused_goes, reset_key, say_args, say_key, sendable, stop_key,
};

mod form;
mod manage;
mod talk;

pub(crate) const KEPT: usize = 1_000;

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct Bot {
    pub id: String,
    pub name: String,
    pub agent: String,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub account: Option<String>,
    pub state: String,
    pub unread: usize,
    pub held: usize,
    #[serde(deserialize_with = "text_or_empty")]
    pub folder: String,
    #[serde(deserialize_with = "text_or_empty")]
    pub access: String,
}

fn set(value: Option<&String>) -> Option<String> {
    value.filter(|value| !value.is_empty()).cloned()
}

fn text_or_empty<'de, D: serde::Deserializer<'de>>(found: D) -> Result<String, D::Error> {
    Ok(Value::deserialize(found)?.as_str().map(str::to_string).unwrap_or_default())
}

impl Bot {
    fn settled_by(&self, newer: &Bot) -> bool {
        (self.thinking() && !newer.thinking()) || newer.unread > self.unread
    }

    fn shown_alike(&self, other: &Bot) -> bool {
        let same = |a: &Option<String>, b: &Option<String>| a == b;
        self.id == other.id
            && self.name == other.name
            && self.agent == other.agent
            && same(&self.model, &other.model)
            && same(&self.effort, &other.effort)
            && same(&self.account, &other.account)
            && self.state == other.state
            && self.unread == other.unread
    }

    pub(crate) fn thinking(&self) -> bool {
        self.state == "thinking"
    }

    pub(crate) fn detail(&self) -> String {
        match set(self.model.as_ref()) {
            Some(model) => format!("{} · {model}", self.agent),
            None => self.agent.clone(),
        }
    }

    pub(crate) fn facts(&self) -> String {
        [
            Some(self.agent.clone()).filter(|agent| !agent.is_empty()),
            set(self.model.as_ref()),
            set(self.effort.as_ref()),
            set(self.account.as_ref()),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ")
    }
}

pub(crate) fn in_app(app: &Value) -> Vec<Bot> {
    let Some(Value::Array(entries)) = app.get("bots") else { return Vec::new() };
    entries.iter().filter_map(|entry| Bot::deserialize(entry).ok()).filter(|bot| !bot.id.is_empty()).collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Kind {
    You,
    Bot,
    Step,
    Error,
    Notice,
    Other(String),
}

impl From<&str> for Kind {
    fn from(kind: &str) -> Self {
        match kind {
            "you" => Self::You,
            "bot" => Self::Bot,
            "step" => Self::Step,
            "error" => Self::Error,
            "notice" => Self::Notice,
            other => Self::Other(other.to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Line {
    pub id: u64,
    pub at: i64,
    pub kind: Kind,
    pub text: String,
    pub held: bool,
}

fn present<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    value.get(key).filter(|found| !found.is_null())
}

pub(crate) fn line_of(value: &Value) -> Option<Line> {
    let id = value.get("id")?.as_u64()?;
    let kind = value.get("kind")?.as_str()?;
    let text = match present(value, "text") {
        None => String::new(),
        Some(text) => text.as_str()?.to_string(),
    };
    let at = present(value, "at").map_or(Some(0), Value::as_i64)?;
    let held = present(value, "held").map_or(Some(false), Value::as_bool)?;
    Some(Line { id, at, kind: Kind::from(kind), text, held })
}

pub(crate) fn history_of(text: &str) -> anyhow::Result<Vec<Line>> {
    let value: Value = serde_json::from_str(text)?;
    let Some(Value::Array(lines)) = value.get("lines") else { anyhow::bail!("no lines in the answer") };
    Ok(lines.iter().filter_map(line_of).collect())
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub(crate) struct BotEvent {
    pub bot: String,
    #[serde(default)]
    pub line: Option<Value>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub change: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Heard {
    Line(String, Line),
    State(String, bool),
    Removed(String),
    Nothing,
}

impl BotEvent {
    pub(crate) fn heard(&self) -> Heard {
        let bot = self.bot.clone();
        if let Some(line) = self.line.as_ref() {
            return line_of(line).map_or(Heard::Nothing, |line| Heard::Line(bot, line));
        }
        if let Some(state) = &self.state {
            return Heard::State(bot, state == "thinking");
        }
        match self.change.as_deref() {
            Some("removed") => Heard::Removed(bot),
            _ => Heard::Nothing,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Read {
    Asking,
    Done,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Chat {
    pub bot: String,
    pub lines: Vec<Line>,
    pub read: Read,
    pub asked: u64,
    pub born: u64,
    pending: Option<u64>,
    revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CatchUp {
    Nothing,
    Whole,
    After(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Added {
    No,
    Yes,
    Gap(u64),
}

impl Chat {
    pub(crate) fn new(bot: &str, asked: u64) -> Self {
        Self {
            bot: bot.to_string(),
            lines: Vec::new(),
            read: Read::Asking,
            asked,
            born: asked,
            pending: None,
            revision: 0,
        }
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn failed(&self) -> bool {
        matches!(self.read, Read::Failed(_))
    }

    pub(crate) fn catch_up(&self) -> CatchUp {
        match (&self.read, self.last()) {
            (Read::Done, Some(last)) => CatchUp::After(last),
            (Read::Done, None) | (Read::Failed(_), _) => CatchUp::Whole,
            (Read::Asking, _) => CatchUp::Nothing,
        }
    }

    pub(crate) fn defer(&mut self, after: u64) {
        self.pending = Some(self.pending.map_or(after, |earlier| earlier.min(after)));
    }

    pub(crate) fn take_pending(&mut self) -> Option<u64> {
        self.pending.take()
    }

    pub(crate) fn awaits(&self, bot: &str, asked: u64) -> bool {
        self.bot == bot && self.asked == asked
    }

    pub(crate) fn last(&self) -> Option<u64> {
        self.lines.last().map(|line| line.id)
    }

    pub(crate) fn earlier(&self) -> bool {
        self.lines.first().is_some_and(|line| line.id > 1)
    }

    pub(crate) fn take(&mut self, lines: Vec<Line>) {
        self.merge(lines);
        self.read = Read::Done;
        self.revision += 1;
    }

    pub(crate) fn fail(&mut self, reason: String) {
        self.read = Read::Failed(reason);
        self.revision += 1;
    }

    pub(crate) fn heard(&mut self, line: Line) -> Added {
        let read = self.read == Read::Done;
        let after = self.last().unwrap_or(0);
        let gap = (read && line.id > after + 1).then_some(after);
        let new = self.lines.binary_search_by_key(&line.id, |l| l.id).is_err();
        self.merge(vec![line]);
        self.revision += u64::from(new);
        match (new, gap) {
            (false, _) => Added::No,
            (true, Some(last)) => Added::Gap(last),
            (true, None) => Added::Yes,
        }
    }

    fn merge(&mut self, lines: Vec<Line>) {
        let mut by_id: HashMap<u64, Line> = self.lines.drain(..).map(|line| (line.id, line)).collect();
        for line in lines {
            by_id.insert(line.id, line);
        }
        let mut merged: Vec<Line> = by_id.into_values().collect();
        merged.sort_by_key(|line| line.id);
        let extra = merged.len().saturating_sub(KEPT);
        merged.drain(..extra);
        self.lines = merged;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Offer {
    Hidden,
    Rows,
    Lacks(String),
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Bots {
    pub list: Vec<Bot>,
    pub boot: Option<String>,
    pub chat: Option<Chat>,
    pub active: bool,
    pub seen: Option<(String, usize)>,
    pub drafts: Drafts,
    pub manage: Managing,
    stopping: HashSet<String>,
    revision: u64,
    asked: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct AppMoved {
    pub boot_changed: bool,
    pub listed: Vec<String>,
    pub settled: Vec<String>,
}

impl Bots {
    pub(crate) fn new() -> Self {
        Self { active: true, ..Self::default() }
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision + self.chat.as_ref().map_or(0, Chat::revision)
    }

    pub(crate) fn bump(&mut self) {
        self.revision += 1;
    }

    pub(crate) fn next_ask(&mut self) -> u64 {
        self.asked += 1;
        self.asked
    }

    pub(crate) fn find(&self, id: &str) -> Option<&Bot> {
        self.list.iter().find(|bot| bot.id == id)
    }

    pub(crate) fn thinking(&self) -> bool {
        self.list.iter().any(Bot::thinking)
    }

    pub(crate) fn unread(&self) -> usize {
        self.list.iter().map(|bot| bot.unread).sum()
    }

    pub(crate) fn take_app(&mut self, app: &Value) -> AppMoved {
        let list = in_app(app);
        let settled = list
            .iter()
            .filter(|new| self.find(&new.id).is_some_and(|old| old.settled_by(new)))
            .map(|bot| bot.id.clone())
            .collect();
        let boot = app.get("boot").and_then(Value::as_str).map(str::to_string);
        let boot_changed = self.boot.is_some() && boot != self.boot;
        self.boot = boot;
        let same = list.len() == self.list.len() && list.iter().zip(&self.list).all(|(new, old)| new.shown_alike(old));
        if !same {
            self.revision += 1;
        }
        self.list = list;
        let unread = |id: &str| self.find(id).is_some_and(|bot| bot.unread > 0);
        if self.seen.as_ref().is_some_and(|(id, _)| !unread(id)) {
            self.seen = None;
        }
        let listed: Vec<String> = self.list.iter().map(|bot| bot.id.clone()).collect();
        self.drafts.keep_only(&listed);
        self.manage.keep_listed(&listed);
        let thinking: HashSet<String> =
            self.list.iter().filter(|bot| bot.thinking()).map(|bot| bot.id.clone()).collect();
        self.stopping.retain(|id| thinking.contains(id));
        AppMoved { boot_changed, listed, settled }
    }

    pub(crate) fn state_heard(&mut self, id: &str, thinking: bool) -> bool {
        let Some(bot) = self.list.iter_mut().find(|bot| bot.id == id) else { return false };
        let state = if thinking { "thinking" } else { "idle" };
        let changed = bot.state != state;
        if !thinking {
            self.stopping.remove(id);
        }
        if changed {
            bot.state = state.to_string();
            self.revision += 1;
        }
        changed
    }

    pub(crate) fn stop_asked(&mut self, id: &str) {
        self.stopping.insert(id.to_string());
    }

    pub(crate) fn stopping(&self, id: &str) -> bool {
        self.stopping.contains(id) && self.find(id).is_some_and(Bot::thinking)
    }

    pub(crate) fn stop_failed(&mut self, id: &str) {
        self.stopping.remove(id);
    }

    pub(crate) fn heard(&mut self, bot: &str, line: Line) -> Option<Added> {
        if self.chat.as_ref().is_none_or(|chat| chat.bot != bot) {
            return None;
        }
        if matches!(line.kind, Kind::Bot | Kind::Error) {
            self.seen = None;
        }
        self.chat.as_mut().map(|chat| chat.heard(line))
    }

    pub(crate) fn forget_seen(&mut self) {
        self.seen = None;
    }

    pub(crate) fn seen_due(&self, shown: Option<&str>) -> Option<String> {
        let bot = self.find(shown?)?;
        let due = self.active && bot.unread > 0 && self.seen.as_ref() != Some(&(bot.id.clone(), bot.unread));
        due.then(|| bot.id.clone())
    }

    pub(crate) fn can_show(&self, id: &str, offer: &Offer) -> bool {
        *offer == Offer::Rows && self.find(id).is_some()
    }

    pub(crate) fn offer(&self, lacks: Option<Lacks<'_>>, os: Os) -> Offer {
        if self.list.is_empty() {
            return Offer::Hidden;
        }
        match lacks {
            Some(Lacks::ThisComputer(has)) => {
                Offer::Lacks(format!("Showing your bots needs {}.", newer_here(Feature::Bots.since(), has, os)))
            }
            _ => Offer::Rows,
        }
    }
}

#[cfg(test)]
mod tests;
