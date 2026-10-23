use serde::Deserialize;
use serde_json::Value;

use crate::store::ToastKind;

#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "event", rename_all = "lowercase")]
pub(crate) enum Event {
    Notice(Notice),
    Clipboard {
        text: String,
        #[serde(default)]
        pane: Option<u64>,
    },
    Bell {
        #[serde(default = "one")]
        count: u64,
        #[serde(default)]
        tab: Option<u64>,
        #[serde(default)]
        pane: Option<u64>,
    },
    #[serde(other)]
    Other,
}

fn one() -> u64 {
    1
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub(crate) struct Notice {
    pub token: String,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub place: Option<String>,
    pub message: String,
    #[serde(default)]
    pub panes: Vec<u64>,
    #[serde(default)]
    pub sound: Option<String>,
}

impl Notice {
    pub(crate) fn toast_kind(&self) -> ToastKind {
        match self.status.as_deref() {
            Some("done") => ToastKind::Done,
            _ => ToastKind::NeedsYou,
        }
    }

    pub(crate) fn headline(&self) -> String {
        let agent = self.agent.as_deref().unwrap_or("an agent");
        match self.status.as_deref() {
            Some("done") => format!("{agent} finished"),
            Some("failed") => format!("{agent} failed"),
            _ => format!("{agent} needs you"),
        }
    }
}

pub(super) fn parse_event(line: &str) -> Option<Event> {
    serde_json::from_str(line.trim()).ok()
}

#[derive(Debug, Default, Deserialize, PartialEq)]
pub(crate) struct AppState {
    #[serde(default)]
    pub boot: Option<String>,
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub away: Vec<Away>,
    #[serde(default)]
    pub update: Option<Update>,
    #[serde(default)]
    pub keep_awake: Option<KeepAwake>,
}

#[derive(Debug, Default, Deserialize, PartialEq)]
pub(crate) struct Away {
    pub platform: String,
    #[serde(default)]
    pub ready: bool,
    #[serde(default, rename = "away")]
    pub on: bool,
}

#[derive(Debug, Default, Deserialize, PartialEq, Clone)]
pub(crate) struct Update {
    #[serde(default)]
    pub release: Option<Release>,
    #[serde(default)]
    pub checking: bool,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub install: Option<Install>,
    #[serde(default)]
    pub installing: bool,
    #[serde(default)]
    pub installed: bool,
    #[serde(default)]
    pub install_error: Option<String>,
}

#[derive(Debug, Default, Deserialize, PartialEq, Clone)]
pub(crate) struct Release {
    pub version: String,
    #[serde(default)]
    pub notes: Vec<Notes>,
}

#[derive(Debug, Default, Deserialize, PartialEq, Clone)]
pub(crate) struct Notes {
    pub version: String,
    #[serde(default)]
    pub text: String,
}

#[derive(Debug, Default, Deserialize, PartialEq, Clone)]
pub(crate) struct Install {
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub command: Option<String>,
}

#[derive(Debug, Default, Deserialize, PartialEq)]
pub(crate) struct KeepAwake {
    #[serde(default)]
    pub held: bool,
    #[serde(default)]
    pub by: Option<String>,
    #[serde(default)]
    pub unavailable: Option<String>,
}

impl AppState {
    pub(crate) fn from_value(value: &Value) -> Self {
        serde_json::from_value(value.clone()).unwrap_or_default()
    }

    pub(crate) fn release(&self) -> Option<&Release> {
        self.update.as_ref().and_then(|u| u.release.as_ref())
    }

    pub(crate) fn away_ready(&self) -> bool {
        self.away.iter().any(|a| a.ready)
    }

    pub(crate) fn away_on(&self) -> Vec<&str> {
        self.away.iter().filter(|a| a.on).map(|a| a.platform.as_str()).collect()
    }
}

pub(super) fn platform_name(platform: &str) -> String {
    match platform {
        "telegram" => "Telegram".into(),
        "discord" => "Discord".into(),
        other => other.to_string(),
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct Prefs {
    pub desktop: bool,
    pub sound_waiting: String,
    pub sound_done: String,
}

impl Default for Prefs {
    fn default() -> Self {
        Self { desktop: true, sound_waiting: String::new(), sound_done: String::new() }
    }
}

impl Prefs {
    pub(crate) fn from_config(config: &Value) -> Self {
        let mut prefs = Prefs::default();
        let Some(settings) = config.get("settings").and_then(Value::as_array) else { return prefs };
        for setting in settings {
            let value = setting.get("value").and_then(Value::as_str).unwrap_or_default().to_string();
            match setting.get("key").and_then(Value::as_str) {
                Some("desktop_notifications") => prefs.desktop = value != "off",
                Some("sound_waiting") => prefs.sound_waiting = value,
                Some("sound_done") => prefs.sound_done = value,
                _ => {}
            }
        }
        prefs
    }

    pub(crate) fn sound_file(&self, cue: &str, home: &str) -> String {
        let own = if cue == "finished" { &self.sound_done } else { &self.sound_waiting };
        let own = match own.strip_prefix("~/") {
            Some(rest) if !home.is_empty() => format!("{home}/{rest}"),
            _ => own.clone(),
        };
        if own.starts_with('/') && std::path::Path::new(&own).is_file() {
            return own;
        }
        system_sound(cue).to_string()
    }
}

pub(super) fn system_sound(cue: &str) -> &'static str {
    if cue == "finished" { "/System/Library/Sounds/Glass.aiff" } else { "/System/Library/Sounds/Ping.aiff" }
}

#[cfg(test)]
mod tests;
