use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use gpui::{App, Context, Entity};
use serde_json::json;

use super::Browser;
use super::follow::Follow;
use super::picker::Pick;
use super::watch::Watch;
use crate::cli;
use crate::model::machines::seen::sighted;
use crate::model::tunnels::Gate;
use crate::model::{MachineSeen, Owner, PaneInfo, PaneThere, Session, Tab, ThereId, ThereWorkspace};
use crate::store::{self, Busy, Store, ToastKind, args};
use crate::ui::TextField;
use far::{Drive, drive};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Menu {
    Shots,
    Agents,
    Chooser,
}

#[derive(Default)]
pub(crate) struct Agents {
    pub(crate) chosen: HashMap<Owner, AgentKey>,
    pub(crate) pick: Option<Pick>,
    pub(crate) ask: Option<Entity<TextField>>,
    pub(crate) focus_ask: bool,
    pub(crate) menu: Option<Menu>,
    pub(crate) closed: Option<(Menu, Instant)>,
    pub(crate) watch: Watch,
    pub(crate) follow: Follow,
    pub(crate) shooting: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Choice {
    pub(crate) pane: AgentKey,
    pub(crate) label: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum AgentKey {
    Here(u64),
    There(ThereId),
}

impl AgentKey {
    pub(crate) fn element(self) -> String {
        match self {
            Self::Here(pane) => format!("here-{pane}"),
            Self::There(pane) => pane.key(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Agent {
    Here(u64),
    There(PaneThere),
}

impl Agent {
    pub(crate) fn key(&self) -> AgentKey {
        match self {
            Self::Here(pane) => AgentKey::Here(*pane),
            Self::There(there) => AgentKey::There(there.pane),
        }
    }
}

pub(crate) fn agent_pane(session: &Session) -> Option<u64> {
    let shown = session.shown();
    let tab = shown.tab.and_then(|id| session.tab(id)).and_then(|t| t.agent_pane()).map(|p| p.id);
    let (_, workspace) = session.workspace(shown.workspace?)?;
    tab.or_else(|| workspace.tabs.iter().find_map(|t| t.agent_pane()).map(|p| p.id))
}

pub(crate) fn endpoint(port: u16) -> String {
    format!("http://127.0.0.1:{port}")
}

fn tab_name<Id>(tab: &Tab<Id>) -> Option<String> {
    let name = tab.custom_name.clone().unwrap_or_else(|| tab.name.clone());
    (!name.trim().is_empty()).then_some(name)
}

pub(crate) fn agent_label<Id>(pane: &PaneInfo<Id>, tab: Option<&Tab<Id>>) -> String {
    let name = pane.agent.clone().unwrap_or_else(|| "agent".to_string());
    match pane.agent_title().or_else(|| tab.and_then(tab_name)) {
        Some(title) if !title.eq_ignore_ascii_case(&name) => format!("{name} · {title}"),
        _ => name,
    }
}

pub(crate) fn workspace_agents(session: &Session, workspace: u64) -> Vec<Choice> {
    let Some((_, ws)) = session.workspace(workspace) else { return Vec::new() };
    let label = |pane: &PaneInfo, tab: u64| agent_label(pane, session.tab(tab));
    ws.agent_rows()
        .iter()
        .map(|row| Choice { pane: AgentKey::Here(row.pane.id), label: label(row.pane, row.tab) })
        .collect()
}

pub(crate) fn pick_agent(session: &Session, remembered: Option<u64>) -> Option<u64> {
    let agents = workspace_agents(session, session.shown().workspace?);
    let listed = |pane: &u64| agents.iter().any(|a| a.pane == AgentKey::Here(*pane));
    remembered.filter(listed).or_else(|| agent_pane(session))
}

pub(crate) fn there_choices(ws: &ThereWorkspace) -> Vec<Choice> {
    let panes = ws.tabs.iter().flat_map(|tab| tab.panes.iter().map(move |pane| (tab, pane)));
    let agents = panes.filter(|(_, pane)| pane.agent.is_some());
    agents.map(|(tab, pane)| Choice { pane: AgentKey::There(pane.id), label: agent_label(pane, Some(tab)) }).collect()
}

pub(crate) fn pick_there(
    choices: &[Choice],
    remembered: Option<AgentKey>,
    tab: Option<&Tab<ThereId>>,
) -> Option<AgentKey> {
    let listed = |key: &AgentKey| choices.iter().any(|c| c.pane == *key);
    let shown = tab.and_then(|t| t.agent_pane()).map(|p| AgentKey::There(p.id));
    remembered.filter(listed).or(shown.filter(listed)).or_else(|| choices.first().map(|c| c.pane))
}

pub(crate) fn unready(session: &Session, pane: u64) -> Option<String> {
    unready_pane(session.pane(pane))
}

pub(crate) fn unready_pane<Id>(pane: Option<&PaneInfo<Id>>) -> Option<String> {
    let Some(info) = pane else { return Some("That agent is gone".to_string()) };
    let name = info.agent.as_deref().unwrap_or("The agent");
    if info.status.as_deref() == Some("waiting") {
        return Some(format!("{name} is waiting for an answer: answer it first"));
    }
    if info.dialog {
        return Some(format!("{name} shows a dialog: close it first"));
    }
    info.resuming.then(|| format!("{name} is resuming its conversation: try again once it is back"))
}

pub(crate) fn driving_note(url: &str) -> String {
    format!(
        "A Chrome you can drive runs at {url} (the DevTools protocol): connect with `chrome-devtools-mcp --browser-url={url}` or Playwright's `chromium.connectOverCDP(\"{url}\")`. I see its tabs in cornercase's Browser panel, so use it rather than launching another browser. "
    )
}

fn paste_args(pane: u64, file: &Path) -> Vec<String> {
    vec!["paste-file".into(), "--pane".into(), pane.to_string(), file.display().to_string()]
}

pub(crate) fn send_args(pane: u64, text: String) -> Vec<String> {
    let mut send = args(&["send", "--pane", &pane.to_string(), "--enter", "--"]);
    send.push(text);
    send
}

pub(crate) fn toast(text: impl Into<String>, kind: ToastKind, cx: &mut App) {
    let store = Store::get(cx);
    store.update(cx, |s, cx| s.toast(text, kind, cx));
}

fn submit_aside(pane: u64, text: String, cx: &mut App) {
    let busy = Busy::new(format!("browser-send:{pane}"), "Sending to the agent…");
    Store::get(cx).update(cx, |s, cx| s.run_aside(busy, send_args(pane, text), cx));
}

impl Browser {
    pub(super) fn hand_to_agent(pane: u64, text: &str, files: &[&Path], cx: &App) {
        let store = Store::get(cx);
        let store = store.read(cx);
        for file in files {
            store.cmd(paste_args(pane, file));
        }
        if !text.is_empty() {
            store.queue.input(pane, json!({ "paste": text }));
        }
    }

    pub(super) fn agents_menu_changed(&self, cx: &mut Context<Self>) {
        if self.agents.menu == Some(Menu::Agents) {
            cx.notify();
        }
    }

    pub(super) fn drive(&self, cx: &App) -> Drive {
        let host = cli::host();
        let store = Store::get(cx).read(cx);
        let toward = self.place.toward(host.as_deref());
        let Some(toward) = toward.as_ref() else { return drive(None, Gate::Open, None, None) };
        let held = store.reverse_hold(toward).map(|hold| hold.state);
        drive(Some(toward), store.reverse_gate_of(toward), held, store.reverse_refusal(toward))
    }

    pub(super) fn agent_endpoint(&self, drive: &Drive) -> Option<String> {
        match drive {
            Drive::Here => self.port().map(endpoint),
            Drive::Own => self.dev.tunnels.reverse(&cli::host()?, self.port()?).map(endpoint),
            Drive::Open(there) => Some(store::address_there(*there)),
            Drive::Needs(_) | Drive::Closed | Drive::Opening => None,
        }
    }

    fn machine_view<'a>(&self, session: &'a Session) -> Option<(&'a MachineSeen, &'a ThereWorkspace)> {
        sighted(&session.machines, self.place.there()?.aim()).ok()
    }

    pub(crate) fn target_agent(&self, cx: &App) -> Option<Agent> {
        let store = Store::get(cx).read(cx);
        let session = store.session.as_ref()?;
        let remembered = self.agents.chosen.get(&self.place).copied();
        match &self.place {
            Owner::Here(_) => {
                let chosen = remembered.and_then(|key| match key {
                    AgentKey::Here(pane) => Some(pane),
                    AgentKey::There(_) => None,
                });
                pick_agent(session, chosen).map(Agent::Here)
            }
            Owner::There(_) => {
                let (seen, ws) = self.machine_view(session)?;
                let tab = store.machine_place.as_ref().and_then(|place| ws.tab(place.tab));
                let key = pick_there(&there_choices(ws), remembered, tab)?;
                let AgentKey::There(pane) = key else { return None };
                Some(Agent::There(PaneThere { machine: seen.name.clone(), boot: seen.boot.clone(), pane }))
            }
            Owner::Nobody => None,
        }
    }

    pub(super) fn many_agents(&self, cx: &App) -> bool {
        self.agent_choices(cx).len() > 1
    }

    pub(super) fn agent_choices(&self, cx: &App) -> Vec<Choice> {
        let store = Store::get(cx).read(cx);
        let Some(session) = store.session.as_ref() else { return Vec::new() };
        match &self.place {
            Owner::Here(w) => workspace_agents(session, *w),
            Owner::There(_) => self.machine_view(session).map(|(_, ws)| there_choices(ws)).unwrap_or_default(),
            Owner::Nobody => Vec::new(),
        }
    }

    pub(super) fn agent_name(agent: &Agent, cx: &App) -> String {
        let store = Store::get(cx).read(cx);
        let session = store.session.as_ref();
        let gone = || "the agent".to_string();
        match agent {
            Agent::Here(pane) => {
                let info = session.and_then(|s| s.pane(*pane).map(|p| (p, s.tab_of_pane(*pane))));
                info.map_or_else(gone, |(p, tab)| agent_label(p, tab))
            }
            Agent::There(there) => {
                let seen = session.and_then(|s| s.machine(&there.machine));
                let info = seen.and_then(|seen| seen.pane(there.pane).map(|p| (p, seen.tab_of_pane(there.pane))));
                info.map_or_else(gone, |(p, tab)| agent_label(p, tab))
            }
        }
    }

    pub(super) fn no_agent_words(&self) -> String {
        self.place.machine().map_or_else(|| "No agent in this workspace".to_string(), no_agent_there)
    }

    pub(super) fn choose_agent(&mut self, key: AgentKey, cx: &mut Context<Self>) {
        if self.place.has_place() {
            self.agents.chosen.insert(self.place.clone(), key);
        }
        if self.agents.menu == Some(Menu::Chooser) {
            self.agents.menu = None;
        }
        cx.notify();
    }

    fn refused(pane: u64, cx: &mut App) -> bool {
        let store = Store::get(cx);
        let Some(why) = store.read(cx).session.as_ref().and_then(|s| unready(s, pane)) else { return false };
        toast(why, ToastKind::Error, cx);
        true
    }

    fn give_to_agent(pane: u64, text: &str, files: &[&Path], cx: &mut App) -> bool {
        if Self::refused(pane, cx) {
            return false;
        }
        Self::hand_to_agent(pane, text, files, cx);
        true
    }

    fn submit_to_agent(pane: u64, text: String, files: &[PathBuf], cx: &mut App) -> bool {
        if Self::refused(pane, cx) {
            return false;
        }
        let Some((last, rest)) = files.split_last() else {
            submit_aside(pane, text, cx);
            return true;
        };
        let store = Store::get(cx);
        for file in rest {
            store.read(cx).cmd(paste_args(pane, file));
        }
        store::queued(cx, paste_args(pane, last), move |pasted, cx| match pasted {
            Ok(_) => submit_aside(pane, text, cx),
            Err(e) => toast(e.to_string(), ToastKind::Error, cx),
        });
        true
    }
}

pub(crate) struct Hand {
    pub(crate) text: String,
    pub(crate) files: Vec<PathBuf>,
    pub(crate) submit: bool,
}

pub(crate) fn no_agent_there(machine: &str) -> String {
    format!("No agent in this workspace on {machine}")
}

impl Browser {
    pub(super) fn agent_name_on(agent: &Agent, cx: &App) -> String {
        let name = Self::agent_name(agent, cx);
        match agent {
            Agent::Here(_) => name,
            Agent::There(there) => format!("{name} on {}", there.machine),
        }
    }

    pub(super) fn hand(agent: &Agent, hand: Hand, said: String, cx: &mut App) -> bool {
        match agent {
            Agent::Here(pane) => Self::hand_here(*pane, hand, said, cx),
            Agent::There(there) => Self::hand_there(there, &hand, said, cx),
        }
    }

    fn hand_here(pane: u64, hand: Hand, said: String, cx: &mut App) -> bool {
        let sent = if hand.submit {
            Self::submit_to_agent(pane, hand.text, &hand.files, cx)
        } else {
            let files: Vec<&Path> = hand.files.iter().map(PathBuf::as_path).collect();
            Self::give_to_agent(pane, &hand.text, &files, cx)
        };
        if sent {
            toast(said, ToastKind::Info, cx);
        }
        sent
    }
}

pub(super) mod far;
mod there;

#[cfg(test)]
mod tests;
