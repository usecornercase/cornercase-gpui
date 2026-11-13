use anyhow::{Context as _, Result, bail};
use serde::Deserialize;
use serde_json::Value;

use super::data::Entry;
use super::read::Keep;
use super::time::{Civil, local, relative, when_label};
use crate::cli::Version;
use crate::model::PullRequest;
use crate::panels::flows::model::epoch;
use crate::ui::markdown::{self, Block};
use crate::ui::selection;

pub(crate) const QUEUE_RESULTS_SINCE: Version = Version::new(0, 13, 51);
pub(super) const CUT_NOTE: &str = "Only the first 4 KiB of this message were kept.";
pub(super) const EARLIER_NOTE: &str = "Earlier messages are left out: these are the newest 1,000.";
pub(super) const NO_MESSAGE: &str = "No message was kept.";
const EARLIER_LINE: &str = "(earlier messages are left out)";
const MESSAGE_CUT_LINE: &str = "… cut at 32 KiB";
pub(super) const MESSAGE_CUT: &str = "Cut at 32 KiB.";

pub(super) fn results_offered(server: Option<Version>) -> bool {
    QUEUE_RESULTS_SINCE.reached_by(server)
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(super) struct Kept {
    pub(super) at: i64,
    pub(super) text: String,
    pub(super) cut: bool,
    pub(super) written: Option<String>,
    pub(super) agent: Option<String>,
    pub(super) branch: Option<String>,
    pub(super) pull_request: Option<PullRequest>,
    pub(super) conversation: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(super) struct Said {
    pub(super) role: String,
    pub(super) text: String,
    pub(super) written: Option<String>,
    pub(super) cut: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Talk {
    Said(Spoken),
    Gone,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Spoken {
    pub(super) messages: Vec<Said>,
    pub(super) earlier: bool,
    pub(super) blocks: Vec<Vec<Block>>,
    pub(super) lines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Answer {
    pub(super) entry: Entry,
    pub(super) result: Option<Kept>,
    pub(super) note: Option<String>,
    pub(super) blocks: Vec<Block>,
    pub(super) lines: Vec<String>,
    pub(super) talk: Option<Talk>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Fact {
    pub(super) label: &'static str,
    pub(super) value: String,
    pub(super) pull: Option<PullRequest>,
}

pub(super) fn message_lines(text: &str) -> Vec<String> {
    text.replace("\r\n", "\n").lines().map(selection::tabs_drawn).collect()
}

fn note_of(value: &Value) -> Option<String> {
    let note = value.get("note")?.as_str()?.trim();
    (!note.is_empty()).then(|| note.to_string())
}

fn messages_of(value: &Value) -> Vec<Said> {
    let items = value.get("messages").and_then(Value::as_array);
    items.map(|items| items.iter().filter_map(|m| Said::deserialize(m).ok()).collect()).unwrap_or_default()
}

fn talk_of(value: &Value) -> Talk {
    if !value.get("messages").is_some_and(Value::is_array) {
        return Talk::Gone;
    }
    let messages = messages_of(value);
    let earlier = value.get("earlier").and_then(Value::as_bool).unwrap_or(false);
    let blocks = messages.iter().map(|m| markdown::parse(&m.text)).collect();
    let lines = talk_lines(&messages, earlier);
    Talk::Said(Spoken { messages, earlier, blocks, lines })
}

impl Answer {
    pub(super) fn read(out: &str, talk: bool) -> Result<Self> {
        let value: Value = serde_json::from_str(out).context("the answer is not JSON")?;
        if !value.is_object() {
            bail!("the answer is not an object");
        }
        let entry = Entry::deserialize(&value).context("the answer holds no entry")?;
        let result = value.get("result").filter(|r| r.is_object()).and_then(|r| Kept::deserialize(r).ok());
        let text = result.as_ref().map(|r| r.text.as_str()).unwrap_or_default();
        let (blocks, lines) = (markdown::parse(text), message_lines(text));
        let talk = talk.then(|| talk_of(&value));
        Ok(Self { entry, result, note: note_of(&value), blocks, lines, talk })
    }

    pub(super) fn can_talk(&self) -> bool {
        self.result.as_ref().is_some_and(|r| r.conversation)
    }
}

pub(super) fn time_words(at: i64, today: Civil, now: i64) -> String {
    let when = local(at).map_or_else(|| at.to_string(), |at| when_label(at, today));
    format!("{when} · {}", relative(at, now))
}

pub(super) fn facts(entry: &Entry, result: Option<&Kept>, today: Civil, now: i64) -> Vec<Fact> {
    let fact = |label, value: String| Fact { label, value, pull: None };
    let agent = result.and_then(|r| r.agent.clone()).unwrap_or_else(|| entry.who());
    let branch = result.and_then(|r| r.branch.clone()).or_else(|| entry.place.branch.clone());
    let mut facts = vec![fact("Scheduled", time_words(entry.at, today, now))];
    facts.extend(entry.started.map(|at| fact("Started", time_words(at, today, now))));
    facts.extend(result.map(|r| fact("Result", time_words(r.at, today, now))));
    facts.push(fact("Agent", agent));
    let pull = result.and_then(|r| r.pull_request.clone());
    facts.extend(pull.map(|pr| Fact { label: "Pull request", value: pr.title.clone(), pull: Some(pr) }));
    facts.extend(branch.map(|branch| fact("Branch", branch)));
    facts
}

pub(super) fn role_word(role: &str) -> String {
    match role {
        "prompt" => "Prompt".into(),
        "reply" => "Reply".into(),
        other => super::data::sentence(other),
    }
}

pub(super) fn said_heading(said: &Said, today: Civil) -> String {
    let word = role_word(&said.role);
    match said.written.as_deref().and_then(epoch).and_then(local) {
        Some(at) => format!("{word} · {}", when_label(at, today)),
        None => word,
    }
}

fn said_line(said: &Said) -> String {
    let word = role_word(&said.role);
    match said.written.as_deref().and_then(epoch).and_then(local) {
        Some(at) => format!("{word} · {} {}", at.date(), at.clock()),
        None => word,
    }
}

pub(super) fn talk_lines(messages: &[Said], earlier: bool) -> Vec<String> {
    let mut lines = Vec::new();
    if earlier {
        lines.extend([EARLIER_LINE.to_string(), String::new()]);
    }
    for said in messages {
        lines.push(said_line(said));
        lines.extend(message_lines(&said.text));
        if said.cut {
            lines.push(MESSAGE_CUT_LINE.to_string());
        }
        lines.push(String::new());
    }
    lines
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Page {
    Message,
    Conversation,
}

pub(super) fn shown_lines(answer: &Answer, page: Page) -> &[String] {
    match (page, &answer.talk) {
        (Page::Conversation, Some(Talk::Said(said))) => &said.lines,
        _ => &answer.lines,
    }
}

pub(super) fn can_copy(answer: &Answer, page: Page) -> bool {
    match (page, &answer.talk) {
        (Page::Conversation, Some(Talk::Said(said))) => said.lines.iter().any(|line| !line.trim().is_empty()),
        (Page::Conversation, _) => false,
        _ => answer.result.as_ref().is_some_and(|r| !r.text.trim().is_empty()),
    }
}

pub(super) fn copied(answer: &Answer, page: Page) -> Option<String> {
    let text = match (page, &answer.talk) {
        (Page::Conversation, Some(Talk::Said(said))) => said.lines.join("\n"),
        (Page::Conversation, _) => String::new(),
        _ => answer.result.as_ref().map(|r| r.text.clone()).unwrap_or_default(),
    };
    (!text.trim().is_empty()).then_some(text)
}

pub(super) fn merged(before: Option<Answer>, mut answer: Answer, keep: Keep) -> (Answer, bool) {
    let (new_talk, old_talk) = (answer.talk.take(), before.as_ref().and_then(|old| old.talk.clone()));
    answer.talk = if keep.talk { new_talk } else { old_talk };
    let changed = before.is_none_or(|old| old.lines != answer.lines || old.talk != answer.talk);
    (answer, changed)
}

pub(super) fn with_talk(mut shown: Answer, answer: Answer) -> (Answer, bool) {
    let changed = shown.talk != answer.talk;
    shown.talk = answer.talk;
    (shown, changed)
}

pub(super) fn page_after(answer: &Answer, page: Page) -> Page {
    if matches!(answer.talk, Some(Talk::Gone)) || !answer.can_talk() { Page::Message } else { page }
}

pub(super) fn notes(answer: Option<&Answer>, page: Page) -> Vec<String> {
    let Some(answer) = answer else { return Vec::new() };
    let mut notes: Vec<String> = answer.note.iter().map(|note| super::data::sentence(note)).collect();
    if page == Page::Message && answer.result.as_ref().is_some_and(|r| r.cut) {
        notes.push(CUT_NOTE.to_string());
    }
    if page == Page::Conversation && matches!(&answer.talk, Some(Talk::Said(said)) if said.earlier) {
        notes.push(EARLIER_NOTE.to_string());
    }
    notes
}

#[cfg(test)]
mod tests;
