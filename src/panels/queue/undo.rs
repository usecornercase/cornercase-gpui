use std::time::Duration;

use serde::Deserialize;

use super::data::Entry;

pub(super) const UNDO_FOR: Duration = Duration::from_secs(6);

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Removed {
    entries: Vec<Entry>,
    undo: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Removal {
    pub(super) undo: u64,
    pub(super) text: String,
}

impl Removal {
    pub(super) fn from_answer(out: &str) -> Option<Self> {
        let removed: Removed = serde_json::from_str(out.trim()).ok()?;
        Some(Self { undo: removed.undo?, text: removed_text(&removed.entries) })
    }
}

fn routine_text(routine: &Entry, runs: usize) -> String {
    match runs {
        0 => format!("Removed “{}”", routine.label),
        1 => format!("Removed “{}” and its run", routine.label),
        many => format!("Removed “{}” and its {many} runs", routine.label),
    }
}

fn removed_text(entries: &[Entry]) -> String {
    if let Some(routine) = entries.iter().find(|e| e.is_routine()) {
        return routine_text(routine, entries.len() - 1);
    }
    match entries {
        [one] if one.routine.is_some() => format!("Removed a run of “{}”", one.label),
        [one] => format!("Removed “{}”", one.label),
        many if many.iter().any(|e| e.routine.is_some()) => format!("Removed {} finished entries", many.len()),
        many => format!("Removed {} started entries", many.len()),
    }
}

#[cfg(test)]
mod tests;
