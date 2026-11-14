use std::time::Instant;

use super::ContextThere;
use crate::cli;
use crate::cli::with_boot;
use crate::dialogs::on_machine;
use crate::model::ContextAnswer;
use crate::store::{AgentReach, args, stale_note};

pub(super) const TOO_OLD: &str = "What fills an agent's context needs cornercase 0.12.243 or newer";

pub(super) fn boot_for(there: Option<&ContextThere>) -> Option<&str> {
    there.filter(|there| there.sends_boot).map(|there| there.boot.as_str())
}

fn on_run(command: Vec<String>, boot: Option<&str>) -> Vec<String> {
    match boot {
        Some(boot) => with_boot(command, boot),
        None => command,
    }
}

pub(super) fn cached_args(pane: u64, boot: Option<&str>) -> Vec<String> {
    on_run(args(&["context", "--pane", &pane.to_string(), "--cached", "--json"]), boot)
}

pub(super) fn read_args(pane: u64, boot: Option<&str>) -> Vec<String> {
    on_run(args(&["context", "--pane", &pane.to_string(), "--json"]), boot)
}

pub(super) fn stale_read(machine: &str, result: anyhow::Result<String>) -> anyhow::Result<String> {
    result.map_err(|e| match stale_note(machine, &e) {
        Some(note) => anyhow::anyhow!(note),
        None => e,
    })
}

pub(super) fn landed(outcome: Outcome, now: &AgentReach) -> Outcome {
    match now {
        AgentReach::Ready(_) => outcome,
        AgentReach::Down(why) | AgentReach::Gone(why) => Outcome::Failed(why.clone()),
    }
}

pub(super) fn job_key(pane: u64, there: Option<(&str, &str)>) -> String {
    match there {
        Some((machine, boot)) => format!("context:{machine}:{boot}:{pane}"),
        None => format!("context:{pane}"),
    }
}

pub(super) fn job_words(who: Option<&str>, machine: Option<&str>) -> String {
    let place = on_machine(machine);
    format!("Reading {}'s context{place}…", who.unwrap_or("the agent"))
}

pub(super) fn title(machine: Option<&str>) -> String {
    format!("Context{}", on_machine(machine))
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

    pub(super) fn read_follows(&self, shown: bool) -> bool {
        let more_to_say = self.answer().is_some_and(|a| a.notice.is_none() || a.breakdown.is_some());
        shown && more_to_say && self.failed.is_none() && self.can_read()
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
