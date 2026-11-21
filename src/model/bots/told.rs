use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::{Ask, Kind, Line, title};
use crate::ui::markdown::{self, Block};

pub(crate) const PAIRED_WITHIN: Duration = Duration::from_secs(2);
pub(crate) const FIRST_LINE: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Answer {
    pub bot: String,
    pub id: u64,
    pub failed: bool,
    pub run: bool,
    pub text: String,
}

pub(crate) fn answer_of(bot: &str, line: &Line, run: bool) -> Option<Answer> {
    let failed = match line.kind {
        Kind::Bot => false,
        Kind::Error => true,
        _ => return None,
    };
    Some(Answer { bot: bot.to_string(), id: line.id, failed, run, text: line.text.clone() })
}

pub(crate) fn tells(answer: &Answer, (shown, active): (Option<&str>, bool), offered: bool) -> bool {
    let run_failed = answer.run && answer.failed;
    let on_screen = active && shown == Some(answer.bot.as_str());
    offered && !run_failed && !on_screen
}

pub(crate) fn read_since(was: usize, is: usize) -> bool {
    was > 0 && is == 0
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Answers {
    heard: Vec<(Answer, Instant)>,
    owed: Vec<(String, Instant)>,
    told: HashMap<String, u64>,
}

impl Answers {
    fn prune(&mut self, now: Instant) {
        let fresh = |at: &Instant| now.saturating_duration_since(*at) <= PAIRED_WITHIN;
        self.heard.retain(|(_, at)| fresh(at));
        self.owed.retain(|(_, at)| fresh(at));
    }

    pub(crate) fn heard(&mut self, answer: Answer, now: Instant) -> Option<Answer> {
        self.prune(now);
        let told = self.told.get(&answer.bot).is_some_and(|last| *last >= answer.id);
        let twice = self.heard.iter().any(|(held, _)| held.bot == answer.bot && held.id == answer.id);
        if told || twice {
            return None;
        }
        if let Some(at) = self.owed.iter().position(|(bot, _)| *bot == answer.bot) {
            self.owed.remove(at);
            self.told.insert(answer.bot.clone(), answer.id);
            return Some(answer);
        }
        self.heard.push((answer, now));
        None
    }

    pub(crate) fn counted(&mut self, bot: &str, (was, is): (usize, usize), now: Instant) -> Vec<Answer> {
        self.prune(now);
        if is == 0 {
            self.owed.retain(|(owed, _)| owed != bot);
        }
        if read_since(was, is) {
            self.heard.retain(|(answer, _)| answer.bot != bot);
        }
        let rise = is.saturating_sub(was);
        if rise == 0 {
            return Vec::new();
        }
        let mut mine: Vec<u64> = self.heard.iter().filter(|(a, _)| a.bot == bot).map(|(a, _)| a.id).collect();
        mine.sort_unstable();
        let taken = mine.split_off(mine.len().saturating_sub(rise));
        let (due, kept): (Vec<_>, Vec<_>) =
            std::mem::take(&mut self.heard).into_iter().partition(|(a, _)| a.bot == bot && taken.contains(&a.id));
        self.heard = kept;
        let mut due: Vec<Answer> = due.into_iter().map(|(answer, _)| answer).collect();
        due.sort_by_key(|answer| answer.id);
        if let Some(newest) = due.last() {
            let last = self.told.entry(bot.to_string()).or_insert(0);
            *last = (*last).max(newest.id);
        }
        for _ in due.len()..rise {
            self.owed.push((bot.to_string(), now));
        }
        due
    }

    pub(crate) fn keep_listed(&mut self, listed: &[String]) {
        let kept = |bot: &String| listed.contains(bot);
        self.heard.retain(|(answer, _)| kept(&answer.bot));
        self.owed.retain(|(bot, _)| kept(bot));
        self.told.retain(|bot, _| kept(bot));
    }

    pub(crate) fn forget_bot(&mut self, bot: &str) {
        self.heard.retain(|(answer, _)| answer.bot != bot);
        self.owed.retain(|(owed, _)| owed != bot);
        self.told.remove(bot);
    }

    pub(crate) fn forget(&mut self) {
        *self = Self::default();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Told {
    pub bot: String,
    pub name: String,
    pub failed: bool,
    pub first: String,
    pub ask: Option<u64>,
}

impl Told {
    pub(crate) fn new(answer: &Answer, name: &str) -> Self {
        let (bot, name, failed) = (answer.bot.clone(), name.to_string(), answer.failed);
        Self { bot, name, failed, first: first_of(answer), ask: None }
    }

    pub(crate) fn asking(bot: &str, name: &str, ask: &Ask) -> Self {
        Self { bot: bot.to_string(), name: name.to_string(), failed: false, first: ask.joined(), ask: Some(ask.id) }
    }

    pub(crate) fn headline(&self) -> String {
        if self.ask.is_some() {
            return title(&self.name);
        }
        let said = if self.failed { "could not answer" } else { "answered" };
        format!("{} {said}", self.name)
    }

    pub(crate) fn body(&self) -> Option<String> {
        Some(self.first.clone()).filter(|first| !first.is_empty())
    }

    pub(crate) fn text(&self) -> String {
        if self.ask.is_some() {
            return format!("{}: {}", self.headline(), self.first);
        }
        match (self.failed, self.body()) {
            (false, Some(first)) => format!("{}: {first}", self.name),
            (true, Some(first)) => format!("{} could not answer: {first}", self.name),
            (_, None) => self.headline(),
        }
    }
}

fn tidy(text: &str) -> String {
    let words = text.split_whitespace().map(|word| word.chars().filter(|c| !c.is_control()).collect::<String>());
    words.filter(|word| !word.is_empty()).collect::<Vec<_>>().join(" ")
}

fn block_text(block: &Block) -> String {
    match block {
        Block::Heading(_, inline) | Block::Paragraph(inline) | Block::Quote(inline) | Block::Item(_, _, inline) => {
            tidy(&inline.text)
        }
        Block::Code(code) => code.text().lines().map(tidy).find(|line| !line.is_empty()).unwrap_or_default(),
        Block::Table(rows) => rows
            .first()
            .map(|row| {
                row.iter().map(|cell| tidy(cell)).filter(|cell| !cell.is_empty()).collect::<Vec<_>>().join(" · ")
            })
            .unwrap_or_default(),
        Block::Rule => String::new(),
    }
}

fn cut(first: String) -> String {
    if first.chars().count() <= FIRST_LINE {
        return first;
    }
    let mut cut: String = first.chars().take(FIRST_LINE - 1).collect();
    cut.push('…');
    cut
}

pub(crate) fn first_line(source: &str) -> String {
    cut(markdown::outline(source).iter().map(block_text).find(|text| !text.is_empty()).unwrap_or_default())
}

fn plain_first_line(text: &str) -> String {
    cut(text.lines().map(tidy).find(|line| !line.is_empty()).unwrap_or_default())
}

fn first_of(answer: &Answer) -> String {
    if answer.failed { plain_first_line(&answer.text) } else { first_line(&answer.text) }
}

pub(crate) fn gone(name: &str) -> String {
    format!("{name} is gone")
}

#[cfg(test)]
mod tests;
