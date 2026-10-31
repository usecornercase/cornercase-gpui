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

fn removed_text(entries: &[Entry]) -> String {
    match entries {
        [one] => format!("Removed “{}”", one.label),
        many => format!("Removed {} started entries", many.len()),
    }
}

#[cfg(test)]
mod tests;
