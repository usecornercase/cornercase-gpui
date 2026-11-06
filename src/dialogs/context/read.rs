use std::time::Instant;

use crate::cli;
use crate::model::ContextAnswer;
use crate::store::args;

pub(super) const TOO_OLD: &str = "What fills an agent's context needs cornercase 0.12.243 or newer";

pub(super) fn cached_args(pane: u64) -> Vec<String> {
    args(&["context", "--pane", &pane.to_string(), "--cached", "--json"])
}

pub(super) fn read_args(pane: u64) -> Vec<String> {
    args(&["context", "--pane", &pane.to_string(), "--json"])
}

fn too_old(reason: &str) -> bool {
    reason.contains("has no `context` command") || reason.contains("unrecognized subcommand 'context'")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Outcome {
    Answer(Box<ContextAnswer>),
    Failed(String),
    TooOld,
}

pub(super) fn outcome_of(result: anyhow::Result<String>) -> Outcome {
    let text = match result {
        Ok(text) => text,
        Err(e) => {
            let reason = cli::reason(&e);
            return if too_old(&reason) { Outcome::TooOld } else { Outcome::Failed(reason) };
        }
    };
    match serde_json::from_str(&text) {
        Ok(answer) => Outcome::Answer(Box::new(answer)),
        Err(e) => Outcome::Failed(format!("cannot read the answer of cornercase context: {e}")),
    }
}

fn read_ago(secs: u64) -> String {
    match secs {
        0..60 => "Read just now".into(),
        60..3_600 => format!("Read {}m ago", secs / 60),
        3_600..86_400 => format!("Read {}h ago", secs / 3_600),
        _ => format!("Read {}d ago", secs / 86_400),
    }
}

#[derive(Debug, Default)]
pub(super) struct Reads {
    running: bool,
    last: Option<(ContextAnswer, Instant)>,
    failed: Option<String>,
    too_old: bool,
}

impl Reads {
    pub(super) fn start(&mut self) {
        self.running = true;
    }

    pub(super) fn settle(&mut self, outcome: Outcome, now: Instant) {
        self.running = false;
        self.failed = None;
        match outcome {
            Outcome::Answer(answer) => self.last = Some((*answer, now)),
            Outcome::Failed(why) => self.failed = Some(why),
            Outcome::TooOld => self.too_old = true,
        }
    }

    pub(super) fn running(&self) -> bool {
        self.running
    }

    pub(super) fn too_old(&self) -> bool {
        self.too_old
    }

    pub(super) fn answer(&self) -> Option<&ContextAnswer> {
        self.last.as_ref().map(|(answer, _)| answer)
    }

    pub(super) fn can_read(&self) -> bool {
        !self.running && !self.too_old
    }

    pub(super) fn read_follows(&self) -> bool {
        let more_to_say = self.answer().is_some_and(|a| a.notice.is_none() || a.breakdown.is_some());
        more_to_say && self.failed.is_none() && self.can_read()
    }

    pub(super) fn waits_for_numbers(&self) -> bool {
        self.running && self.answer().is_some_and(|a| a.breakdown.is_none() && a.notice.is_none())
    }

    pub(super) fn failure(&self) -> Option<&str> {
        if self.running {
            return None;
        }
        self.failed.as_deref().or_else(|| self.answer()?.error.as_deref())
    }

    pub(super) fn age_note(&self, now: Instant) -> Option<String> {
        let (answer, at) = self.last.as_ref()?;
        answer.breakdown.as_ref()?;
        Some(read_ago(answer.age? + now.saturating_duration_since(*at).as_secs()))
    }
}

#[cfg(test)]
mod tests;
