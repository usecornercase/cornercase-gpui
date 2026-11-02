use crate::cli;
use crate::model::{PullRequest, PullRequestAnswer};

pub(super) const TOO_OLD: &str = "Every check with its link needs cornercase 0.12.237 or newer";

const NONE: &str = "No pull request";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Answer {
    Found(Box<PullRequest>),
    Missing(String),
    Failed(String),
    TooOld,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Note<'a> {
    Asking,
    Failed(&'a str),
    Said(&'a str),
}

pub(super) fn args(workspace: u64) -> Vec<String> {
    crate::store::args(&["pull-request", "--workspace", &workspace.to_string(), "--json"])
}

pub(super) fn too_old(reason: &str) -> bool {
    let named = reason.contains("pull-request");
    named && (reason.contains("has no `pull-request` command") || reason.contains("unrecognized subcommand"))
}

pub(super) fn answer_of(workspace: u64, result: anyhow::Result<String>) -> Option<Answer> {
    let text = match result {
        Ok(text) => text,
        Err(e) => return Some(failure(cli::reason(&e))),
    };
    match serde_json::from_str::<PullRequestAnswer>(&text) {
        Ok(read) if read.workspace != workspace => None,
        Ok(read) => Some(match read.pull_request {
            Some(found) => Answer::Found(Box::new(found)),
            None => Answer::Missing(read.reason.unwrap_or_else(|| NONE.to_string())),
        }),
        Err(e) => Some(Answer::Failed(format!("cannot read the answer of cornercase pull-request: {e}"))),
    }
}

fn failure(reason: String) -> Answer {
    if too_old(&reason) { Answer::TooOld } else { Answer::Failed(reason) }
}

#[derive(Debug, Default)]
pub(super) struct Reads {
    asked: u64,
    answered: u64,
    asked_for: Option<PullRequest>,
    answer: Option<Answer>,
}

impl Reads {
    pub(super) fn due(&self, view: &PullRequest) -> bool {
        self.answer != Some(Answer::TooOld) && self.asked_for.as_ref() != Some(view)
    }

    pub(super) fn start(&mut self, view: &PullRequest) -> u64 {
        self.asked += 1;
        self.asked_for = Some(view.clone());
        self.asked
    }

    pub(super) fn settle(&mut self, generation: u64, answer: Option<Answer>) -> bool {
        if generation <= self.answered || generation > self.asked {
            return false;
        }
        self.answered = generation;
        if answer.is_some() {
            self.answer = answer;
        }
        true
    }

    pub(super) fn asking(&self) -> bool {
        self.answered < self.asked
    }

    pub(super) fn fill(&self, view: &mut PullRequest) {
        if let Some(Answer::Found(found)) = &self.answer
            && found.number == view.number
            && found.url == view.url
        {
            view.all_checks.clone_from(&found.all_checks);
        }
    }

    pub(super) fn note(&self) -> Option<Note<'_>> {
        if self.asking() {
            return Some(Note::Asking);
        }
        match self.answer.as_ref()? {
            Answer::Found(_) => None,
            Answer::Missing(why) => Some(Note::Said(why)),
            Answer::Failed(why) => Some(Note::Failed(why)),
            Answer::TooOld => Some(Note::Said(TOO_OLD)),
        }
    }
}

#[cfg(test)]
mod tests;
