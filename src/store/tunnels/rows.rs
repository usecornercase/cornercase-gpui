use std::collections::{BTreeMap, BTreeSet};

use super::held::{Held, Hold};
use super::reverse::{ReverseHold, ReverseState};
use crate::model::MachineSeen;
use crate::model::machines::listed;
use crate::model::tunnels::{Gate, Older, Toward, TunnelSeen, orders_note};
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

fn others_row(toward: &Toward, seen: &TunnelSeen) -> TunnelRow {
    let whose = if seen.state == "closed" {
        Whose::ClosedElsewhere
    } else if seen.kind == "window" {
        Whose::Window
    } else {
        Whose::Program
    };
    TunnelRow { toward: toward.clone(), port: seen.port, whose }
}

fn of_place<'a>(held: &'a [Hold], toward: &Toward) -> Vec<&'a Hold> {
    held.iter().filter(|hold| hold.toward == *toward).collect()
}

pub(crate) fn place_rows(
    toward: &Toward,
    listed: &[TunnelSeen],
    held: &[Hold],
    reverse: Option<&ReverseHold>,
) -> Vec<TunnelRow> {
    let ours = of_place(held, toward);
    let mine = ours.iter().copied().filter_map(ours_row);
    let others = listed.iter().filter(|seen| seen.forward() && !is_ours(seen, &ours)).map(|s| others_row(toward, s));
    let mut rows: Vec<TunnelRow> = mine.chain(others).collect();
    rows.sort_by_key(|row| (row.port, !row.ours()));
    rows.extend(reverse.map(|hold| reverse_row(toward, hold)));
    rows
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TunnelSection {
    pub(crate) name: String,
    pub(crate) rows: Vec<TunnelRow>,
    pub(crate) last_sent: Option<String>,
}

impl TunnelSection {
    pub(crate) fn rests(&self, row: &TunnelRow) -> bool {
        self.last_sent.is_some() && !row.ours()
    }
}

pub(crate) fn last_sent_note(seen: &MachineSeen) -> String {
    format!("{}: the tunnels other windows or programs hold to it are as it last sent them", seen.link_note())
}

pub(crate) fn pruned(ended: &mut BTreeSet<(String, u16, u16)>, machines: &[MachineSeen]) {
    let lists = |seen: &MachineSeen, port: u16, local: u16| {
        seen.tunnels.iter().any(|t| t.forward() && t.kind == "hold" && t.port == port && t.local == Some(local))
    };
    ended.retain(|(name, port, local)| {
        machines.iter().find(|seen| seen.name == *name).is_none_or(|seen| !seen.linked() || lists(seen, *port, *local))
    });
}

fn last_sent(seen: Option<&MachineSeen>, name: &str, ended: &BTreeSet<(String, u16, u16)>) -> Vec<TunnelSeen> {
    let Some(seen) = seen else { return Vec::new() };
    let gone = |tunnel: &TunnelSeen| {
        let ours = |local: u16| ended.contains(&(name.to_string(), tunnel.port, local));
        !seen.linked() && tunnel.forward() && tunnel.kind == "hold" && tunnel.local.is_some_and(ours)
    };
    seen.tunnels.iter().filter(|tunnel| !gone(tunnel)).cloned().collect()
}

fn section(
    name: &str,
    (holds, reverses): (&[Hold], &BTreeMap<Toward, ReverseHold>),
    (seen, ended): (Option<&MachineSeen>, &BTreeSet<(String, u16, u16)>),
) -> Option<TunnelSection> {
    let toward = Toward::Machine(name.to_string());
    let listed = last_sent(seen, name, ended);
    let rows = place_rows(&toward, &listed, holds, reverses.get(&toward));
    if rows.is_empty() {
        return None;
    }
    let others = rows.iter().any(|row| !row.ours());
    let last_sent = seen.filter(|seen| !seen.linked() && others).map(last_sent_note);
    Some(TunnelSection { name: name.to_string(), rows, last_sent })
}

pub(crate) fn machine_sections(
    holds: &[Hold],
    reverses: &BTreeMap<Toward, ReverseHold>,
    machines: &[MachineSeen],
    ended: &BTreeSet<(String, u16, u16)>,
) -> Vec<TunnelSection> {
    let towards = holds.iter().map(|hold| &hold.toward).chain(reverses.keys());
    let held = towards.filter_map(|toward| toward.machine().map(str::to_string));
    let names: BTreeSet<String> = held.chain(machines.iter().map(|seen| seen.name.clone())).collect();
    let seen = |name: &str| machines.iter().find(|seen| seen.name == name);
    names.iter().filter_map(|name| section(name, (holds, reverses), (seen(name), ended))).collect()
}

pub(crate) fn elsewhere(toward: &Toward, port: u16, listed: &[TunnelSeen], held: &[Hold]) -> bool {
    let ours = of_place(held, toward);
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
