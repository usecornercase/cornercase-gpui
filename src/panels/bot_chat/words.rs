use super::entries::Turn;
use crate::panels::flows::model::{age, plural};
use crate::panels::queue;

pub(super) const HELD: &str = "Held";
pub(super) const EARLIER: &str = "Earlier messages are left out: these are the newest 1,000.";
pub(super) const NEW_MESSAGES: &str = "New messages";

pub(crate) fn time_label(at: i64, now: i64) -> String {
    let (Some(then), Some(today)) = (queue::local(at), queue::local(now)) else { return String::new() };
    let same_day = queue::days(then.year, then.month, then.day) == queue::days(today.year, today.month, today.day);
    if same_day { then.clock() } else { queue::when_label(then, today) }
}

pub(crate) fn turn_label(turn: Turn, now: i64) -> String {
    let waited = age((now - turn.since).max(0));
    let parts = [
        Some("Thinking".to_string()),
        Some(waited).filter(|waited| waited != "now"),
        Some(turn.steps).filter(|steps| *steps > 0).map(|steps| plural(steps, "step")),
    ];
    parts.into_iter().flatten().collect::<Vec<_>>().join(" · ")
}

pub(crate) fn folded_label(hidden: usize) -> String {
    format!("+{}", plural(hidden, "step"))
}

pub(super) const EMPTY_LINES: [&str; 2] = [
    "A bot is an agent of your own. It belongs to no project and keeps a memory.",
    "It runs only when you write to it, and answers here.",
];

pub(super) fn footer(name: &str, id: &str) -> String {
    format!("Write to {name} in cornercase's own window, or with cornercase bot say {id}.")
}
