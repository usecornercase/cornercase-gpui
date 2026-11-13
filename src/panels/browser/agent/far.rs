use crate::model::tunnels::{Gate, Toward};
use crate::store::ReverseState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Drive {
    Here,
    Own,
    Needs(String),
    Closed,
    Opening,
    Open(u16),
}

pub(crate) fn drive(toward: Option<&Toward>, gate: Gate, held: Option<ReverseState>, refused: Option<String>) -> Drive {
    let Some(toward) = toward else { return Drive::Here };
    match held {
        Some(ReverseState::Open { there }) => return Drive::Open(there),
        Some(ReverseState::Opening) => return Drive::Opening,
        None => {}
    }
    if *toward == Toward::Server && matches!(gate, Gate::Older(_)) {
        return Drive::Own;
    }
    refused.map_or(Drive::Closed, Drive::Needs)
}

pub(crate) fn own_reverse_stays(gate: Gate) -> bool {
    gate != Gate::Open
}

pub(crate) fn says(drive: &Drive, name: &str) -> String {
    match drive {
        Drive::Here | Drive::Own => {
            "Agents can drive this Chrome over the DevTools protocol, and you see their tabs here.".to_string()
        }
        Drive::Needs(_) => {
            format!("Agents on {name} cannot drive this Chrome: it runs on this {}.", crate::desktop::here())
        }
        Drive::Closed | Drive::Opening => {
            format!("Agents on {name} can drive this Chrome once you open it to {name}.")
        }
        Drive::Open(_) => format!("Agents on {name} can drive this Chrome, and you see their tabs here."),
    }
}

pub(crate) fn exposure(name: &str) -> String {
    format!("Open to {name}: every user and program there can drive it.")
}

pub(crate) fn own_exposure(host: &str) -> String {
    format!("Through this window's own ssh: every user and program on {host} can drive it while this window runs.")
}

pub(crate) fn open_label(name: &str) -> String {
    format!("Open this Chrome to {name}…")
}

pub(crate) fn opening_words(name: &str) -> String {
    format!("Opening this Chrome to {name}…")
}

pub(crate) fn close_label(name: &str) -> String {
    format!("Close it to {name}")
}

pub(crate) fn copied_words(name: &str) -> String {
    format!("Copied the DevTools address on {name}")
}

pub(crate) fn confirm_words(name: &str) -> (String, String, String) {
    let message = format!(
        "Agents on {name} will be able to drive this Chrome and use the logins its profile holds. So will every other user and program on {name}, through a port on its loopback, and anyone who can reach {name} if its ssh server sets GatewayPorts yes. It stays open until you close it, the link to {name} ends or this window quits."
    );
    (format!("Open this Chrome to {name}?"), message, format!("Open to {name}"))
}

#[cfg(test)]
mod tests;
