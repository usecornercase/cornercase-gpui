use anyhow::bail;
use serde_json::Value;

use super::{Bot, Home, access_words, set};
use crate::cli::{Feature, Lacks, newer_here};
use crate::model::{MachinePlace, MachineSeen, PaneInfo, Session, Tab};
use crate::panels::flows::model::plural;
use crate::panels::queue::{Entry, repeat_words};
use crate::platform::Os;
use crate::platform::paths::{Style, home_relative};

const THE_AGENTS_OWN: &str = "the agent's own";
const UNREADABLE: &str = "secrets cannot be read";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tone {
    Plain,
    Danger,
    Waiting,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CardRow {
    pub label: &'static str,
    pub value: String,
    pub tone: Tone,
}

fn row(label: &'static str, value: String, tone: Tone) -> CardRow {
    CardRow { label, value, tone }
}

pub(crate) fn secrets_words(count: usize, unreadable: bool) -> Option<(String, bool)> {
    if unreadable {
        return Some((UNREADABLE.to_string(), true));
    }
    (count > 0).then(|| (plural(count, "secret"), false))
}

fn access_row(access: &str) -> Option<CardRow> {
    match access_words(access) {
        Some((words, full)) => Some(row("Access", words.to_string(), if full { Tone::Danger } else { Tone::Plain })),
        None => (!access.is_empty()).then(|| row("Access", access.to_string(), Tone::Plain)),
    }
}

pub(crate) fn card_rows(bot: &Bot, made: Option<String>) -> Vec<CardRow> {
    let plain = |label, value: Option<String>| value.map(|value| row(label, value, Tone::Plain));
    let model = set(bot.model.as_ref()).unwrap_or_else(|| THE_AGENTS_OWN.to_string());
    let secrets = secrets_words(bot.secrets, bot.secrets_unreadable)
        .map(|(words, unreadable)| row("Secrets", words, if unreadable { Tone::Danger } else { Tone::Plain }));
    let asks = bot.asks.iter().map(|ask| row("Asks", format!("for a secret: {}", ask.joined()), Tone::Waiting));
    let mut rows: Vec<CardRow> = [
        plain("Agent", Some(bot.agent.clone()).filter(|agent| !agent.is_empty())),
        Some(row("Model", model, Tone::Plain)),
        plain("Effort", set(bot.effort.as_ref())),
        plain("Account", set(bot.account.as_ref())),
        access_row(&bot.access),
        secrets,
    ]
    .into_iter()
    .flatten()
    .collect();
    rows.extend(asks);
    rows.extend(plain("Made", made));
    rows
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ListRow {
    pub id: u64,
    pub name: String,
    pub detail: String,
}

pub(crate) fn routine_rows(entries: &[Entry], bot: &str, next: impl Fn(i64) -> String) -> Vec<ListRow> {
    entries
        .iter()
        .filter(|entry| entry.repeat.is_some() && entry.place.bot.as_deref() == Some(bot))
        .map(|entry| {
            let first = if entry.paused {
                "paused".to_string()
            } else {
                format!("next {}", next(entry.next.unwrap_or(entry.at)))
            };
            let detail = format!("{first} · ↻ {}", repeat_words(entry));
            ListRow { id: entry.id, name: entry.label.clone(), detail }
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ShowAt {
    Pane(u64),
    There(Box<MachinePlace>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StartedRow {
    pub row: ListRow,
    pub status: Option<String>,
    pub stop: String,
    pub show: ShowAt,
}

fn place(project: &str, workspaces: usize, workspace: &str) -> String {
    if workspaces > 1 { format!("{project} › {workspace}") } else { project.to_string() }
}

fn tagged<'a, Id>(tabs: &'a [Tab<Id>], bot: &'a str) -> impl Iterator<Item = (&'a Tab<Id>, &'a PaneInfo<Id>)> {
    tabs.iter()
        .flat_map(|tab| tab.panes.iter().map(move |pane| (tab, pane)))
        .filter(move |(_, pane)| pane.agent.is_some() && pane.bot.as_deref() == Some(bot))
}

fn started_row<Id>(
    (tab, pane): (&Tab<Id>, &PaneInfo<Id>),
    place: &str,
    (id, stop, show): (u64, String, ShowAt),
) -> StartedRow {
    let name = pane.agent_title().unwrap_or_else(|| tab.name.clone());
    let agent = pane.agent.clone().unwrap_or_default();
    let row = ListRow { id, name, detail: format!("{agent} · {place}") };
    StartedRow { row, status: pane.status.clone(), stop, show }
}

pub(crate) fn started_rows(session: &Session, bot: &str) -> Vec<StartedRow> {
    let mut rows = Vec::new();
    for project in &session.projects {
        for workspace in &project.workspaces {
            let place = place(&project.name, project.workspaces.len(), &workspace.name);
            rows.extend(tagged(&workspace.tabs, bot).map(|(tab, pane)| {
                started_row((tab, pane), &place, (pane.id, pane.id.to_string(), ShowAt::Pane(pane.id)))
            }));
        }
    }
    rows
}

pub(crate) fn started_rows_there(seen: &MachineSeen, bot: &str) -> Vec<StartedRow> {
    let workspaces = seen.projects.iter().flat_map(|project| project.workspaces.iter().map(move |ws| (project, ws)));
    let panes = workspaces.flat_map(|(project, ws)| tagged(&ws.tabs, bot).map(move |found| (project, ws, found)));
    panes
        .enumerate()
        .map(|(at, (project, ws, (tab, pane)))| {
            let place = place(&project.name, project.workspaces.len(), &ws.name);
            let show = ShowAt::There(Box::new(MachinePlace::at(seen, ws, Some(tab.id))));
            started_row((tab, pane), &place, (u64::try_from(at).unwrap_or(u64::MAX), pane.id.key(), show))
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Controls {
    pub routines: bool,
    pub edit: bool,
    pub note: Option<String>,
}

pub(crate) fn controls(home: &Home, routines_offered: bool) -> Controls {
    match home.machine() {
        None => Controls { routines: routines_offered, edit: true, note: None },
        Some(machine) => Controls {
            routines: false,
            edit: false,
            note: Some(format!(
                "Change it, its secrets and its routines on {machine}: this window only reads a machine's bot for now."
            )),
        },
    }
}

pub(crate) fn folder_parts(shown: &str) -> (&str, &str) {
    let style = Style::of_path(shown);
    let whole = shown.trim_end_matches(|c| style.is_separator(c));
    whole.split_at(style.name_start(whole))
}

pub(crate) fn folder_shown(path: &str, home: &Home, own_home: &str) -> String {
    match home {
        Home::Here => home_relative(path, own_home),
        Home::Machine(_) => path.to_string(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AboutGate {
    Hidden,
    Lacks(String),
    Read,
}

pub(crate) fn about_gate(lacks: Option<Lacks<'_>>, os: Os) -> AboutGate {
    match lacks {
        None => AboutGate::Read,
        Some(Lacks::ThisComputer(has)) => {
            AboutGate::Lacks(format!("Needs {}.", newer_here(Feature::BotAboutRead.since(), has, os)))
        }
        Some(Lacks::Server(_) | Lacks::Machine(..)) => AboutGate::Hidden,
    }
}

pub(crate) fn about_args(bot: &str) -> Vec<String> {
    ["bot", "about", "--json", "--", bot].map(str::to_string).to_vec()
}

pub(crate) fn about_of(out: &str) -> anyhow::Result<String> {
    let found: Value = serde_json::from_str(out)?;
    match found.get("about") {
        Some(Value::String(about)) => Ok(about.clone()),
        _ => bail!("cornercase did not say what the bot is for"),
    }
}

pub(crate) fn about_failed(why: &str) -> String {
    format!("Could not read what it is for: {why}")
}

#[cfg(test)]
mod tests;
