use serde::Deserialize as _;
use serde_json::Value;

use super::{BotEvent, Give};

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum Home {
    #[default]
    Here,
    Machine(String),
}

impl Home {
    pub(crate) fn machine(&self) -> Option<&str> {
        match self {
            Self::Here => None,
            Self::Machine(name) => Some(name),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct BotKey {
    pub home: Home,
    pub id: String,
}

impl BotKey {
    pub(crate) fn here(id: &str) -> Self {
        Self { home: Home::Here, id: id.to_string() }
    }

    #[cfg(test)]
    pub(crate) fn on(machine: &str, id: &str) -> Self {
        Self { home: Home::Machine(machine.to_string()), id: id.to_string() }
    }

    pub(crate) fn tag(&self) -> String {
        match &self.home {
            Home::Here => self.id.clone(),
            Home::Machine(machine) => format!("{machine}/{}", self.id),
        }
    }
}

pub(crate) fn label(name: &str, home: &Home) -> String {
    match home.machine() {
        None => name.to_string(),
        Some(machine) => format!("{name} on {machine}"),
    }
}

pub(crate) fn giving_at(home: &Home, offer: Give) -> Give {
    match home.machine() {
        None => offer,
        Some(machine) => {
            Give::Here(format!("Give it on {machine}: this window answers only this computer's bots for now."))
        }
    }
}

pub(crate) fn event_of(line: &str) -> Option<BotEvent> {
    let value: Value = serde_json::from_str(line.trim()).ok()?;
    if value.get("event").and_then(Value::as_str) != Some("bot") {
        return None;
    }
    BotEvent::deserialize(&value).ok()
}

#[cfg(test)]
mod tests;
