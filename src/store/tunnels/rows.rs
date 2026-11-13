use std::collections::BTreeMap;

use super::held::{Held, Hold};
use super::reverse::{ReverseHold, ReverseState};
use crate::model::machines::listed;
use crate::model::tunnels::{ELSEWHERE_ISSUE, Gate, Older, Toward, TunnelSeen, orders_note};
use crate::store::{OlderNote, UpdateOffer, UpdateThere};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Control {
    Hidden,
    Tunnel,
    Opening,
    Close,
    Closing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Whose {
    Up { local: u16 },
    Opening,
    Down { why: String },
    Reverse { there: Option<u16> },
    Window,
    Program,
    ClosedElsewhere,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TunnelRow {
    pub(crate) toward: Toward,
    pub(crate) port: u16,
    pub(crate) whose: Whose,
}

impl TunnelRow {
    pub(crate) fn ours(&self) -> bool {
        matches!(self.whose, Whose::Up { .. } | Whose::Opening | Whose::Down { .. } | Whose::Reverse { .. })
    }

    pub(crate) fn text(&self) -> String {
        let port = self.port;
        match &self.whose {
            Whose::Up { local } => format!("⇄ {port} → localhost:{local} · up"),
            Whose::Opening => format!("⇄ {port} · opening"),
            Whose::Down { .. } => format!("⇄ {port} · down"),
            Whose::Reverse { there: Some(there) } => format!("⇄ {there} ← this Chrome · open"),
            Whose::Reverse { there: None } => "⇄ this Chrome · opening".to_string(),
            Whose::Window => format!("⇄ {port} · in another window"),
            Whose::Program => format!("⇄ {port} · in another program"),
            Whose::ClosedElsewhere => format!("⇄ {port} · closed in another window"),
        }
    }
}

pub(crate) fn count(ours: &[Hold], reverses: usize) -> usize {
    ours.iter().filter(|hold| hold.state != Held::Ending).count() + reverses
}

fn reverse_row(toward: &Toward, hold: &ReverseHold) -> TunnelRow {
    let there = match hold.state {
        ReverseState::Opening => None,
        ReverseState::Open { there } => Some(there),
    };
    TunnelRow { toward: toward.clone(), port: there.unwrap_or(0), whose: Whose::Reverse { there } }
}

fn ours_row(hold: &Hold) -> Option<TunnelRow> {
    let whose = match &hold.state {
        Held::Opening => Whose::Opening,
        Held::Up { local } => Whose::Up { local: *local },
        Held::Down { why } => Whose::Down { why: why.clone() },
        Held::Ending => return None,
    };
    Some(TunnelRow { toward: hold.toward.clone(), port: hold.port, whose })
}

fn is_ours(seen: &TunnelSeen, ours: &[&Hold]) -> bool {
    seen.forward()
        && seen.kind == "hold"
        && ours.iter().any(|hold| hold.port == seen.port && hold.local().is_none_or(|local| Some(local) == seen.local))
}

fn others_row(seen: &TunnelSeen) -> TunnelRow {
    let whose = if seen.state == "closed" {
        Whose::ClosedElsewhere
    } else if seen.kind == "window" {
        Whose::Window
    } else {
        Whose::Program
    };
    TunnelRow { toward: Toward::Server, port: seen.port, whose }
}

fn on_server(held: &[Hold]) -> Vec<&Hold> {
    held.iter().filter(|hold| hold.toward == Toward::Server).collect()
}

pub(crate) fn menu_rows(listed: &[TunnelSeen], held: &[Hold], reverse: Option<&ReverseHold>) -> Vec<TunnelRow> {
    let ours = on_server(held);
    let mine = ours.iter().copied().filter_map(ours_row);
    let others = listed.iter().filter(|seen| seen.forward() && !is_ours(seen, &ours)).map(others_row);
    let mut rows: Vec<TunnelRow> = mine.chain(others).collect();
    rows.sort_by_key(|row| (row.port, !row.ours()));
    rows.extend(reverse.map(|hold| reverse_row(&Toward::Server, hold)));
    rows
}

pub(crate) fn machine_sections(
    holds: &[Hold],
    reverses: &BTreeMap<Toward, ReverseHold>,
) -> Vec<(String, Vec<TunnelRow>)> {
    let mut by_machine: BTreeMap<String, Vec<TunnelRow>> = BTreeMap::new();
    for row in holds.iter().filter_map(ours_row) {
        if let Toward::Machine(name) = &row.toward {
            by_machine.entry(name.clone()).or_default().push(row);
        }
    }
    for rows in by_machine.values_mut() {
        rows.sort_by_key(|row| row.port);
    }
    for (toward, hold) in reverses {
        if let Toward::Machine(name) = toward {
            by_machine.entry(name.clone()).or_default().push(reverse_row(toward, hold));
        }
    }
    by_machine.into_iter().collect()
}

pub(crate) fn elsewhere(port: u16, listed: &[TunnelSeen], held: &[Hold]) -> bool {
    let ours = on_server(held);
    listed.iter().any(|seen| seen.forward() && seen.port == port && seen.state != "closed" && !is_ours(seen, &ours))
}

pub(crate) fn row_control(can_tunnel: bool, held: Option<&Held>) -> Control {
    match held {
        None if can_tunnel => Control::Tunnel,
        None => Control::Hidden,
        Some(Held::Opening) => Control::Opening,
        Some(Held::Up { .. } | Held::Down { .. }) => Control::Close,
        Some(Held::Ending) => Control::Closing,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Mark {
    pub(crate) text: String,
    pub(crate) tip: Option<String>,
    pub(crate) waiting: bool,
}

pub(crate) fn mark(shows: bool, held: Option<&Held>, elsewhere: bool) -> Option<Mark> {
    if !shows && held.is_none() {
        return None;
    }
    let plain = |text: &str| Some(Mark { text: text.to_string(), tip: None, waiting: false });
    match held {
        Some(Held::Up { local }) => plain(&format!("⇄ :{local}")),
        Some(Held::Opening) => plain("⇄ …"),
        Some(Held::Down { why }) => {
            Some(Mark { text: "⇄ down".to_string(), tip: (!why.is_empty()).then(|| why.clone()), waiting: true })
        }
        None if elsewhere => Some(Mark {
            text: "⇄ elsewhere".to_string(),
            tip: Some("In another window or program: cornercase tunnels lists it".to_string()),
            waiting: false,
        }),
        Some(Held::Ending) | None => None,
    }
}

pub(crate) fn others_note(machines: &str) -> String {
    format!("Tunnels other windows or programs hold to {machines} are not listed here: needs {ELSEWHERE_ISSUE}")
}

pub(crate) fn orders_notes(gates: &[(String, Gate)]) -> Vec<String> {
    let this = |gate: &Gate| match gate {
        Gate::Older(older @ Older::ThisComputer(_)) => Some(*older),
        _ => None,
    };
    if let Some(older) = gates.iter().find_map(|(_, gate)| this(gate)) {
        let names: Vec<String> =
            gates.iter().filter(|(_, gate)| this(gate).is_some()).map(|(n, _)| n.clone()).collect();
        return vec![orders_note(&older, &listed(&names))];
    }
    let theirs = |(name, gate): &(String, Gate)| match gate {
        Gate::Older(older @ Older::Server(_)) => Some(orders_note(older, name)),
        _ => None,
    };
    gates.iter().filter_map(theirs).collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Tail {
    Note(String),
    Update(UpdateThere),
    Updating(String),
}

pub(crate) fn section_tail(note: Option<&OlderNote>, offer: &UpdateOffer) -> Vec<Tail> {
    let Some(note) = note else { return Vec::new() };
    let mut tail = vec![Tail::Note(note.tunnels.clone())];
    match offer {
        UpdateOffer::Hidden => {}
        UpdateOffer::Ask(update) => tail.push(Tail::Update(update.clone())),
        UpdateOffer::Running(label) => tail.push(Tail::Updating(label.clone())),
    }
    tail
}

const HEADING_HOST: usize = 28;

pub(crate) fn heading(host: &str) -> String {
    let cut: String = host.chars().take(HEADING_HOST).collect();
    let ellipsis = if host.chars().count() > HEADING_HOST { "…" } else { "" };
    format!("Tunnels to {cut}{ellipsis}")
}

pub(crate) fn none_note(keys: Option<&str>) -> String {
    let ports = keys.map_or_else(|| "Ports".to_string(), |keys| format!("Ports ({keys})"));
    format!("None yet: Tunnel on a port in {ports}, or opening it in the Browser, makes one.")
}

#[cfg(test)]
mod tests;
