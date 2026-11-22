use super::entries::Turn;
use crate::model::bots::Bot;
use crate::panels::flows::model::{age, plural};
use crate::panels::queue;

pub(super) const HELD: &str = "Held";
pub(super) const DROPPED_STAMP: &str = "Dropped";
pub(super) const EARLIER: &str = "Earlier messages are left out: these are the newest 1,000.";
pub(super) const NEW_MESSAGES: &str = "New messages";
pub(super) const GIVE: &str = "Give…";

pub(crate) fn time_label(at: i64, now: i64) -> String {
    let (Some(then), Some(today)) = (queue::local(at), queue::local(now)) else { return String::new() };
    let same_day = queue::days(then.year, then.month, then.day) == queue::days(today.year, today.month, today.day);
    if same_day { then.clock() } else { queue::when_label(then, today) }
}

pub(crate) fn list_time(at: i64, now: i64) -> String {
    const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    let (Some(then), Some(today)) = (queue::local(at), queue::local(now)) else { return String::new() };
    match queue::days(today.year, today.month, today.day) - queue::days(then.year, then.month, then.day) {
        0 => then.clock(),
        1 => "Yesterday".to_string(),
        2..=6 => WEEKDAYS.get(then.weekday as usize).copied().unwrap_or_default().to_string(),
        _ => then.date(),
    }
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

const WHAT_A_BOT_IS: &str = "A bot is an agent of your own. It belongs to no project and keeps a memory.";

pub(crate) fn empty_lines(routines: bool) -> [&'static str; 2] {
    let when = if routines {
        "It answers here when you write to it, and on a schedule it sets itself. Try one of these:"
    } else {
        "It runs only when you write to it, and answers here. Try one of these:"
    };
    [WHAT_A_BOT_IS, when]
}

pub(crate) const EXAMPLES: [&str; 4] = [
    "Which of my open pull requests are blocked, and why?",
    "What did my agents get done today, in every project?",
    "Which issues labelled bug came in today, in every project?",
    "Every weekday at 9, tell me which of my PRs are blocked.",
];

pub(crate) fn examples_shown(routines: bool) -> usize {
    if routines { EXAMPLES.len() } else { EXAMPLES.len() - 1 }
}

pub(crate) const SEND: &str = "Send";
pub(crate) const SENDING: &str = "Sending…";
pub(crate) const SETTING_UP: &str = "Setting up…";
pub(crate) const STOP: &str = "Stop";
pub(crate) const STOPPING: &str = "Stopping…";
pub(crate) const RETRY: &str = "Retry";
pub(crate) const NEW_CONVERSATION: &str = "New conversation";
pub(crate) const STARTING: &str = "Starting…";
pub(crate) const RUN_AGAIN: &str = "Run again";
pub(crate) const ANSWERING: &str = "Answering…";
pub(crate) const ROUTINES: &str = "Routines";

pub(crate) fn routines_tip(name: &str) -> String {
    format!("What {name} does on a schedule")
}

pub(crate) fn row_detail(bot: &Bot, busy: bool, now: i64) -> String {
    match bot.next.filter(|_| !busy) {
        Some(next) => format!("next {}", time_label(next, now)),
        None => bot.detail(),
    }
}

pub(crate) fn placeholder(name: &str) -> String {
    format!("Message {name}…")
}

pub(crate) fn details_tip(name: &str) -> String {
    format!("Details of {name}")
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

pub(super) const SHOW: &str = "Show";
pub(super) const CLOSED: &str = "closed";
pub(super) const GO_TO_AGENT: &str = "Go to agent";
pub(super) const SHOW_WHOLE: &str = "Show the whole message";
pub(super) const FOLD: &str = "Fold the message";

pub(super) fn there(machine: &str) -> String {
    format!("on {machine}")
}
