use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;

use crate::cli::Version;

pub(crate) const NO_MODE: &str = "default";
pub(crate) const NO_MODE_SINCE: Version = Version::new(0, 13, 28);

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Choice {
    pub value: String,
    pub dangerous: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Modes {
    pub chosen: String,
    pub choices: Vec<Choice>,
    pub server: Option<Version>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(from = "Listed")]
pub(crate) struct Agents {
    pub default: String,
    pub list: Vec<Choice>,
    pub modes: HashMap<String, Modes>,
}

#[derive(Deserialize)]
struct Listed {
    settings: Vec<Entry>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Entry {
    key: String,
    value: Value,
    choices: Vec<Choice>,
}

impl Entry {
    fn chosen(&self) -> String {
        self.value.as_str().unwrap_or_default().to_string()
    }

    fn offered(self) -> Vec<Choice> {
        self.choices.into_iter().filter(|choice| !choice.value.is_empty()).collect()
    }
}

impl From<Listed> for Agents {
    fn from(listed: Listed) -> Self {
        let mut agents = Agents::default();
        for entry in listed.settings {
            let key = entry.key.clone();
            if key == "agent" {
                agents.default = entry.chosen();
                agents.list = entry.offered().into_iter().filter(|choice| choice.value != "auto").collect();
            } else if let Some(agent) = key.strip_prefix("agents.").and_then(|rest| rest.strip_suffix(".mode")) {
                let chosen = entry.chosen();
                agents.modes.insert(agent.to_string(), Modes { chosen, choices: entry.offered(), server: None });
            }
        }
        agents
    }
}

impl Agents {
    pub(crate) fn from_config(config: &Value) -> Self {
        Listed::deserialize(config).map(Agents::from).unwrap_or_default()
    }

    pub(crate) fn initial(&self) -> String {
        if self.list.iter().any(|agent| agent.value == self.default) {
            return self.default.clone();
        }
        self.list.first().map_or_else(|| "claude".into(), |agent| agent.value.clone())
    }

    pub(crate) fn modes_of(&self, agent: &str) -> Option<&Modes> {
        self.modes.get(agent).filter(|modes| !modes.choices.is_empty())
    }
}

fn default_rests(chosen: &str, server: Option<Version>) -> bool {
    chosen != NO_MODE && !NO_MODE_SINCE.reached_by(server)
}

fn default_waits(chosen: &str, machine: Option<&str>, server: Option<Version>) -> String {
    let (place, runs) = match machine {
        Some(machine) => (format!(" on {machine} while its settings"), format!("{machine}'s server runs")),
        None => (" while settings".to_string(), "the server runs".to_string()),
    };
    let release = server.map(|server| format!(" ({runs} {server})")).unwrap_or_default();
    format!("default needs cornercase {NO_MODE_SINCE} or newer{place} choose {chosen}{release}")
}

impl Modes {
    pub(crate) fn on_server(self, server: Option<Version>) -> Self {
        Self { server, ..self }
    }

    fn offers(&self, value: &str) -> bool {
        self.choices.iter().any(|choice| choice.value == value)
    }

    pub(crate) fn shown<'a>(&'a self, picked: Option<&'a str>) -> &'a str {
        picked.filter(|value| self.offers(value) && !self.rests(value)).unwrap_or(&self.chosen)
    }

    pub(crate) fn rests(&self, value: &str) -> bool {
        value == NO_MODE && default_rests(&self.chosen, self.server)
    }

    pub(crate) fn sent(&self, picked: Option<&str>) -> Option<String> {
        let value = picked.filter(|value| self.offers(value) && !self.rests(value))?;
        (value != self.chosen).then(|| value.to_string())
    }

    pub(crate) fn waits(&self) -> bool {
        self.choices.iter().any(|choice| self.rests(&choice.value))
    }

    pub(crate) fn waiting(&self, machine: Option<&str>) -> Option<String> {
        self.waits().then(|| default_waits(&self.chosen, machine, self.server))
    }
}

#[cfg(test)]
mod tests;
