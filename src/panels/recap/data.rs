use std::collections::BTreeMap;
use std::rc::Rc;

use gpui::{ListAlignment, ListState, px};
use serde::Deserialize;

use super::names;
use crate::panels::flows::model::ago;
use crate::panels::issues::markdown::Block;
use crate::store::{Busy, args};

const OVERDRAW: f32 = 400.;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Choice {
    Now,
    Yesterday,
    Week,
}

impl Choice {
    pub(super) const ALL: [Choice; 3] = [Choice::Now, Choice::Yesterday, Choice::Week];

    fn name(self) -> &'static str {
        match self {
            Self::Now => "now",
            Self::Yesterday => "yesterday",
            Self::Week => "week",
        }
    }

    pub(super) fn title(self) -> &'static str {
        match self {
            Self::Now => "Now",
            Self::Yesterday => "Yesterday",
            Self::Week => "7 days",
        }
    }

    pub(super) fn index(self) -> usize {
        match self {
            Self::Now => 0,
            Self::Yesterday => 1,
            Self::Week => 2,
        }
    }

    pub(super) fn key(self) -> String {
        format!("recap:{}", self.name())
    }

    pub(super) fn busy(self) -> Busy {
        Busy::new(self.key(), "Writing the recap…")
    }

    pub(super) fn command(self, fresh: bool) -> Vec<String> {
        let mut command = args(&["recap", self.name(), "--json"]);
        if fresh {
            command.push("--fresh".to_string());
        }
        command
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(super) struct Named {
    pub label: String,
    pub path: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(super) struct Place {
    pub kind: String,
    pub project: Option<Named>,
    pub workspace: Option<Named>,
    pub tab: Option<String>,
    pub pane: Option<u64>,
    pub boot: Option<String>,
    pub number: Option<u64>,
    pub url: Option<String>,
}

impl Place {
    pub(super) fn is_pull(&self) -> bool {
        self.kind == "pull"
    }

    pub(super) fn name(&self) -> String {
        let named = [&self.project, &self.workspace].into_iter().flatten().map(|named| named.label.as_str());
        named.chain(self.tab.as_deref()).collect::<Vec<_>>().join(" › ")
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(super) struct Answer {
    pub label: Option<String>,
    pub written_at: Option<String>,
    pub agent: Option<String>,
    pub title: Option<String>,
    pub model: Option<String>,
    pub conversations: u64,
    pub text: Option<String>,
    pub places: BTreeMap<String, Place>,
    pub loading: bool,
    pub progress: Option<u64>,
    pub empty: Option<String>,
    pub error: Option<String>,
}

pub(super) struct Page {
    pub answer: Answer,
    pub blocks: Vec<Block>,
    pub list: ListState,
}

impl Page {
    fn new(answer: Answer) -> Self {
        let blocks = names::blocks(answer.text.as_deref().unwrap_or_default(), &answer.places);
        let list = ListState::new(blocks.len(), ListAlignment::Top, px(OVERDRAW));
        Self { answer, blocks, list }
    }

    pub(super) fn byline(&self, now: i64) -> String {
        let answer = &self.answer;
        let who = answer.title.clone().or_else(|| answer.agent.clone());
        let read = match answer.conversations {
            1 => "read 1 conversation".to_string(),
            many => format!("read {many} conversations"),
        };
        let written = answer.written_at.as_deref().and_then(|time| written(time, now));
        who.into_iter().chain(answer.model.clone()).chain(Some(read)).chain(written).collect::<Vec<_>>().join(" · ")
    }
}

fn written(time: &str, now: i64) -> Option<String> {
    match ago(time, now).as_str() {
        "" => None,
        "now" => Some("written just now".to_string()),
        age => Some(format!("written {age} ago")),
    }
}

#[derive(Default)]
pub(super) struct Slot {
    pub page: Option<Rc<Page>>,
    pub empty: Option<String>,
    pub error: Option<String>,
    pub progress: Option<u64>,
}

impl Slot {
    pub(super) fn answered(&mut self, answer: Result<Answer, String>) -> bool {
        match answer {
            Ok(answer) => self.read(answer),
            Err(error) => {
                self.error = Some(error);
                self.empty = None;
                self.progress = None;
                false
            }
        }
    }

    fn read(&mut self, mut answer: Answer) -> bool {
        let runs = std::mem::take(&mut answer.loading);
        self.progress = answer.progress.take().filter(|_| runs);
        self.error = answer.error.take();
        self.empty = answer.empty.take();
        if answer.text.is_some() {
            if self.page.as_ref().is_none_or(|page| page.answer != answer) {
                self.page = Some(Rc::new(Page::new(answer)));
            }
        } else if self.empty.is_some() {
            self.page = None;
        }
        runs
    }
}

pub(super) fn writing(progress: Option<u64>) -> String {
    match progress {
        Some(1) => "An agent is writing the recap… 1 step".to_string(),
        Some(steps) if steps > 0 => format!("An agent is writing the recap… {steps} steps"),
        _ => "An agent is writing the recap…".to_string(),
    }
}

#[cfg(test)]
mod tests;
