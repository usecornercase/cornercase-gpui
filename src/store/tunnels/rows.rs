use super::held::{Held, Hold};
use crate::model::tunnels::{ASKS_ISSUE, TunnelSeen, Way};

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
    Window,
    Program,
    ClosedElsewhere,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TunnelRow {
    pub(crate) port: u16,
    pub(crate) whose: Whose,
}

impl TunnelRow {
    pub(crate) fn ours(&self) -> bool {
        matches!(self.whose, Whose::Up { .. } | Whose::Opening | Whose::Down { .. })
    }

    pub(crate) fn text(&self) -> String {
        let port = self.port;
        match &self.whose {
            Whose::Up { local } => format!("⇄ {port} → localhost:{local} · up"),
            Whose::Opening => format!("⇄ {port} · opening"),
            Whose::Down { .. } => format!("⇄ {port} · down"),
            Whose::Window => format!("⇄ {port} · in another window"),
            Whose::Program => format!("⇄ {port} · in another program"),
            Whose::ClosedElsewhere => format!("⇄ {port} · closed in another window"),
        }
    }
}

pub(crate) fn count(ours: &[Hold]) -> usize {
    ours.iter().filter(|hold| hold.state != Held::Ending).count()
}

fn ours_row(hold: &Hold) -> Option<TunnelRow> {
    let whose = match &hold.state {
        Held::Opening => Whose::Opening,
        Held::Up { local } => Whose::Up { local: *local },
        Held::Down { why } => Whose::Down { why: why.clone() },
        Held::Ending => return None,
    };
    Some(TunnelRow { port: hold.port, whose })
}

fn is_ours(seen: &TunnelSeen, ours: &[Hold]) -> bool {
    seen.kind == "hold"
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
    TunnelRow { port: seen.port, whose }
}

pub(crate) fn menu_rows(listed: &[TunnelSeen], ours: &[Hold]) -> Vec<TunnelRow> {
    let mine = ours.iter().filter_map(ours_row);
    let others = listed.iter().filter(|seen| !is_ours(seen, ours)).map(others_row);
    let mut rows: Vec<TunnelRow> = mine.chain(others).collect();
    rows.sort_by_key(|row| (row.port, !row.ours()));
    rows
}

pub(crate) fn elsewhere(port: u16, listed: &[TunnelSeen], ours: &[Hold]) -> bool {
    listed.iter().any(|seen| seen.port == port && seen.state != "closed" && !is_ours(seen, ours))
}

pub(crate) fn row_control(way: &Way, held: Option<&Held>) -> Control {
    if *way != Way::Cornercase {
        return Control::Hidden;
    }
    match held {
        None => Control::Tunnel,
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

pub(crate) fn mark(way: &Way, held: Option<&Held>, elsewhere: bool) -> Option<Mark> {
    if *way != Way::Cornercase {
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

pub(crate) fn asks_note(host: &str) -> String {
    format!("A program on {host} cannot open a tunnel here yet: needs {ASKS_ISSUE}")
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
