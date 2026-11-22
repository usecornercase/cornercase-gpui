use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{BotKey, FACE_COLOURS, Home, face_colour};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Faces {
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    here: BTreeMap<String, u8>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    machines: BTreeMap<String, BTreeMap<String, u8>>,
}

pub(crate) fn face_of_name(name: &str) -> u8 {
    face_colour(&name.trim().to_lowercase())
}

impl Faces {
    pub(crate) fn parse(text: &str) -> Faces {
        serde_json::from_str(text).unwrap_or_default()
    }

    pub(crate) fn text(&self) -> String {
        let mut text = serde_json::to_string_pretty(self).unwrap_or_default();
        text.push('\n');
        text
    }

    fn kept(&self, key: &BotKey) -> Option<u8> {
        let home = match &key.home {
            Home::Here => &self.here,
            Home::Machine(machine) => self.machines.get(machine)?,
        };
        home.get(&key.id).copied().filter(|colour| FACE_COLOURS.contains(colour))
    }

    pub(crate) fn of(&self, key: &BotKey) -> u8 {
        self.kept(key).unwrap_or_else(|| face_colour(&key.id))
    }

    pub(crate) fn set(&mut self, key: &BotKey, colour: u8) -> bool {
        if !FACE_COLOURS.contains(&colour) || self.kept(key) == Some(colour) {
            return false;
        }
        let home = match &key.home {
            Home::Here => &mut self.here,
            Home::Machine(machine) => self.machines.entry(machine.clone()).or_default(),
        };
        home.insert(key.id.clone(), colour);
        true
    }

    pub(crate) fn forget(&mut self, key: &BotKey) -> bool {
        let home = match &key.home {
            Home::Here => Some(&mut self.here),
            Home::Machine(machine) => self.machines.get_mut(machine),
        };
        home.and_then(|home| home.remove(&key.id)).is_some()
    }
}

#[cfg(test)]
mod tests;
