use std::time::{Duration, Instant};

use super::held::{Held, Hold};
use crate::cli::{self, Stream, Target};
use crate::model::machines::hide_destination;
use crate::model::tunnels::{Gate, Older, Order, Toward};

pub(super) struct Taker {
    pub(super) destination: String,
    pub(super) label: String,
    pub(super) target: Target,
    pub(super) stream: Option<Stream>,
    pub(super) attempts: u32,
    pub(super) up_since: Option<Instant>,
    pub(super) told: bool,
    pub(super) generation: u64,
}

impl Taker {
    pub(super) fn phase(&self) -> Phase {
        if self.stream.is_some() { Phase::Running } else { Phase::Waiting { generation: self.generation } }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Phase {
    Running,
    Waiting { generation: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Cause {
    Follow,
    Linked,
    Due { generation: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Next {
    Start,
    Stop,
    Nothing,
}

pub(crate) const MOST_ASKED: usize = 32;

pub(crate) fn next(phase: Option<Phase>, cause: Cause, (can, stays): (bool, bool)) -> Next {
    let start = if can { Next::Start } else { Next::Nothing };
    match (phase, cause) {
        (Some(_), _) if !stays => Next::Stop,
        (None, Cause::Follow | Cause::Linked) | (Some(Phase::Waiting { .. }), Cause::Linked) => start,
        (Some(Phase::Waiting { generation: waiting }), Cause::Due { generation }) if waiting == generation => {
            if can {
                Next::Start
            } else {
                Next::Stop
            }
        }
        _ => Next::Nothing,
    }
}

pub(crate) fn wait_after(attempts: u32) -> Duration {
    cli::backoff(attempts.saturating_sub(1))
}

pub(crate) fn orders_stay(link_stays: bool, gate: Gate) -> bool {
    link_stays && !matches!(gate, Gate::Older(Older::ThisComputer(_) | Older::Server(Some(_))))
}

pub(crate) fn may_take(gate: Gate, linked: bool, shared: bool) -> bool {
    gate == Gate::Open && linked && !shared
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Answer {
    Hold,
    Again,
    Ignore,
    Refuse(String),
}

pub(crate) fn answer(held: Option<&Held>, live: usize, cannot: Option<String>, (port, label): (u16, &str)) -> Answer {
    let here = crate::desktop::here();
    match held {
        Some(Held::Opening | Held::Up { .. }) => return Answer::Ignore,
        Some(Held::Ending) => {
            let words = format!("is closing the tunnel of port {port}: ask again once it is closed");
            return Answer::Refuse(format!("the window on this {here} {words}"));
        }
        Some(Held::Down { .. }) | None => {}
    }
    if let Some(why) = cannot {
        return Answer::Refuse(format!("the window on this {here} cannot hold it now: {why}"));
    }
    if matches!(held, Some(Held::Down { .. })) {
        return Answer::Again;
    }
    if live >= MOST_ASKED {
        return Answer::Refuse(format!(
            "the window on this {here} already holds {MOST_ASKED} tunnels to {label}: close one first"
        ));
    }
    Answer::Hold
}

pub(crate) fn split_closed(holds: Vec<Hold>, destination: &str, port: u16) -> (Vec<Hold>, Vec<Hold>) {
    holds
        .into_iter()
        .partition(|hold| hold.destination == destination && hold.port == port && hold.state != Held::Ending)
}

const STABLE_AFTER: Duration = Duration::from_secs(30);

pub(crate) fn after_end(attempts: u32, told: bool, up: Option<Duration>) -> (u32, bool) {
    if up.is_some_and(|up| up >= STABLE_AFTER) { (1, false) } else { (attempts + 1, told) }
}

pub(crate) fn untold(toward: &Toward, (label, destination): (&str, &str), port: u16, reason: &str) -> String {
    let reason = match toward {
        Toward::Server => reason.to_string(),
        Toward::Machine(_) => hide_destination(reason, label, destination),
    };
    format!("Could not tell {label} that port {port} is not tunnelled here: {reason}")
}

const TELL_AFTER: u32 = 5;

pub(crate) fn tells(attempts: u32, told: bool) -> bool {
    attempts >= TELL_AFTER && !told
}

pub(crate) fn keeps(order: Order) -> bool {
    matches!(order, Order::Open(_))
}

pub(crate) fn refusal_words(why: &str) -> String {
    let here = crate::desktop::here();
    if why.trim().is_empty() { format!("the window on this {here} cannot hold it") } else { why.to_string() }
}

pub(crate) fn unheard(toward: &Toward, (label, destination): (&str, &str), said: &str) -> String {
    let line = cli::reason_of(said);
    let line = if line.is_empty() { "the connection ended".to_string() } else { line };
    let line = match toward {
        Toward::Server => line,
        Toward::Machine(_) => hide_destination(&line, label, destination),
    };
    format!("A program on {label} cannot ask this window for a tunnel: {line}")
}

#[cfg(test)]
mod tests;
