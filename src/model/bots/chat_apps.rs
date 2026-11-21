use serde::Deserialize;
use serde_json::Value;

use crate::cli::{Feature, Lacks, newer_here};
use crate::platform::Os;

pub(crate) const AWAY: &str = "away";
pub(crate) const ALWAYS: &str = "always";
pub(crate) const OFF: &str = "off";
pub(crate) const CHOICES: [(&str, &str); 3] = [(AWAY, "While away"), (ALWAYS, "Always"), (OFF, "Off")];
pub(crate) const UNPAIRED: &str = "Pair Telegram or Discord in Settings › Remote";

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct ChatApp {
    pub platform: String,
    pub ready: bool,
    pub away: bool,
}

pub(crate) fn platforms(app: Option<&Value>) -> Vec<ChatApp> {
    let Some(Value::Array(rows)) = app.and_then(|app| app.get("away")) else { return Vec::new() };
    rows.iter().filter_map(|row| ChatApp::deserialize(row).ok()).collect()
}

pub(crate) fn known(value: &str) -> Option<String> {
    CHOICES.iter().any(|(id, _)| *id == value).then(|| value.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Row {
    Hidden,
    Lacks(String),
    Unpaired,
    Choices,
}

pub(crate) fn row(lacks: Option<Lacks<'_>>, platforms: &[ChatApp], os: Os) -> Row {
    match lacks {
        Some(Lacks::ThisComputer(has)) => {
            Row::Lacks(format!("Needs {}.", newer_here(Feature::BotChatApps.since(), has, os)))
        }
        Some(_) => Row::Hidden,
        None if platforms.iter().any(|p| p.ready) => Row::Choices,
        None => Row::Unpaired,
    }
}

pub(crate) fn sent(row: &Row, chosen: Option<&str>) -> Option<String> {
    chosen.filter(|_| *row == Row::Choices).map(str::to_string)
}

pub(crate) fn note(chosen: Option<&str>) -> Option<&'static str> {
    match chosen? {
        AWAY => Some("Its answers also reach your chat apps while you are away."),
        ALWAYS => Some("Its answers always reach your chat apps too."),
        OFF => {
            Some("Its answers never reach your chat apps. A message you write to it from one is still answered there.")
        }
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Reach {
    pub names: Vec<String>,
    pub always: bool,
}

fn platform_name(platform: &str) -> String {
    match platform {
        "telegram" => "Telegram".to_string(),
        "discord" => "Discord".to_string(),
        other => other.to_string(),
    }
}

pub(crate) fn reach(chat_apps: &str, platforms: &[ChatApp]) -> Option<Reach> {
    let always = chat_apps == ALWAYS;
    let names: Vec<String> = platforms
        .iter()
        .filter(|p| p.ready && (always || (chat_apps == AWAY && p.away)))
        .map(|p| platform_name(&p.platform))
        .collect();
    (!names.is_empty()).then_some(Reach { names, always })
}

pub(crate) fn reach_tip(reach: &Reach) -> String {
    let names = reach.names.join(" and ");
    if reach.always {
        format!("Its answers also go to {names}.")
    } else {
        format!("Its answers also go to {names} while you are away.")
    }
}

#[cfg(test)]
mod tests;
