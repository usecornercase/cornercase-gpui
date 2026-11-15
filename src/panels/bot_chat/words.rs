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
    "It runs only when you write to it, and answers here. Try one of these:",
];

pub(crate) const EXAMPLES: [&str; 3] = [
    "Which of my open pull requests are blocked, and why?",
    "What did my agents get done today, in every project?",
    "Which issues labelled bug came in today, in every project?",
];

pub(crate) const SEND: &str = "Send";
pub(crate) const SENDING: &str = "Sending…";
pub(crate) const STOP: &str = "Stop";
pub(crate) const STOPPING: &str = "Stopping…";
pub(crate) const RETRY: &str = "Retry";
pub(crate) const NEW_CONVERSATION: &str = "New conversation";
pub(crate) const STARTING: &str = "Starting…";

pub(crate) fn placeholder(name: &str) -> String {
    format!("Message {name}…")
}

pub(crate) fn menu_tip(name: &str) -> String {
    format!("More for {name}")
}

pub(crate) fn answering(name: &str) -> String {
    format!("{name} is answering: stop it first")
}

pub(crate) fn new_conversation_title(name: &str) -> String {
    format!("Start a new conversation with {name}?")
}

pub(crate) fn new_conversation_message(name: &str) -> String {
    format!(
        "{name} forgets this conversation, and your next message starts a new one. Its chat stays, and so do what it is for and its memory (bot.md and memory.md), which the new conversation reads."
    )
}

pub(super) fn read_failed(name: &str, why: &str) -> String {
    format!("Could not read the chat of {name}: {why}")
}
