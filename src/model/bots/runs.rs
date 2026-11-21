use std::collections::HashMap;

use crate::panels::queue::Entry;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Run {
    bot: String,
    label: String,
    answers: bool,
    again: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Runs {
    by_entry: HashMap<u64, Run>,
    scheduled: HashMap<String, Vec<String>>,
}

fn again_of(entry: &Entry, entries: &[Entry]) -> Option<u64> {
    match entry.routine {
        Some(routine) => entries.iter().any(|found| found.id == routine).then_some(routine),
        None => matches!(entry.state.as_str(), "failed" | "missed").then_some(entry.id),
    }
}

impl Runs {
    pub(crate) fn of(entries: &[Entry]) -> Self {
        let mut runs = Self::default();
        for entry in entries {
            let Some(bot) = entry.place.bot.as_deref().filter(|bot| !bot.is_empty()) else { continue };
            let run = Run {
                bot: bot.to_string(),
                label: entry.label.clone(),
                answers: entry.state == "starting",
                again: again_of(entry, entries),
            };
            runs.by_entry.insert(entry.id, run);
            if entry.routine.is_none() {
                runs.scheduled.entry(bot.to_string()).or_default().push(entry.label.clone());
            }
        }
        runs
    }

    pub(crate) fn answers(&self, run: u64) -> bool {
        self.by_entry.get(&run).is_some_and(|found| found.answers)
    }

    pub(crate) fn answering(&self, bot: &str) -> bool {
        self.by_entry.values().any(|run| run.bot == bot && run.answers)
    }

    pub(crate) fn again(&self, run: u64) -> Option<u64> {
        self.by_entry.get(&run).and_then(|found| found.again)
    }

    pub(crate) fn label(&self, run: u64) -> Option<&str> {
        self.by_entry.get(&run).map(|found| found.label.as_str())
    }

    pub(crate) fn scheduled(&self, bot: &str) -> &[String] {
        self.scheduled.get(bot).map_or(&[], Vec::as_slice)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Anchor {
    pub bot: String,
    pub run: u64,
    pub serial: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Aim {
    Wait,
    Go(usize),
    Miss,
}

pub(crate) fn aim(rows: &[Option<u64>], anchor: Option<&Anchor>, aimed: u64, read: bool) -> Aim {
    let Some(anchor) = anchor.filter(|anchor| anchor.serial > aimed) else { return Aim::Wait };
    match rows.iter().position(|row| *row == Some(anchor.run)) {
        Some(ix) => Aim::Go(ix),
        None if read => Aim::Miss,
        None => Aim::Wait,
    }
}

#[cfg(test)]
mod tests;
