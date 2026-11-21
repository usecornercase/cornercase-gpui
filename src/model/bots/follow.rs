use serde_json::Value;

use super::Line;
use crate::cli::{Feature, Lacks, newer_here};
use crate::platform::Os;

const GLYPHS: [char; 4] = ['▶', '✓', '✗', '!'];
const FAILED: char = '✗';

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentRef {
    pub pane: u64,
    pub boot: String,
}

pub(crate) fn agent_of(line: &Value) -> Option<AgentRef> {
    let agent = line.get("agent")?.as_object()?;
    let pane = agent.get("pane")?.as_u64()?;
    let boot = agent.get("boot")?.as_str()?.to_string();
    Some(AgentRef { pane, boot })
}

pub(crate) fn words(text: &str) -> &str {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) if GLYPHS.contains(&first) => chars.as_str().trim_start(),
        _ => text,
    }
}

pub(crate) fn failed(text: &str) -> bool {
    text.starts_with(FAILED)
}

pub(crate) fn showable(lines: &[Line], boot: &str, here: impl Fn(u64) -> bool) -> Vec<u64> {
    if boot.is_empty() {
        return Vec::new();
    }
    let mut ids: Vec<u64> = lines
        .iter()
        .filter(|line| line.agent.as_ref().is_some_and(|agent| agent.boot == boot && here(agent.pane)))
        .map(|line| line.id)
        .collect();
    ids.sort_unstable();
    ids
}

pub(crate) fn by(name: &str) -> String {
    format!("by {name}")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FollowRow {
    Hidden,
    Lacks(String),
    Box,
}

pub(crate) fn follow_row(lacks: Option<Lacks<'_>>, os: Os) -> FollowRow {
    match lacks {
        Some(Lacks::ThisComputer(has)) => {
            FollowRow::Lacks(format!("Needs {}.", newer_here(Feature::BotFollow.since(), has, os)))
        }
        Some(_) => FollowRow::Hidden,
        None => FollowRow::Box,
    }
}

pub(crate) fn follow_sent(row: &FollowRow, on: bool) -> Option<bool> {
    (*row == FollowRow::Box).then_some(on)
}

pub(crate) fn follow_note(on: bool) -> &'static str {
    if on {
        "When one finishes, its last message comes to this chat and the bot answers there."
    } else {
        "It is told nothing of the agents it starts. Their rows still say it started them."
    }
}

#[cfg(test)]
mod tests;
