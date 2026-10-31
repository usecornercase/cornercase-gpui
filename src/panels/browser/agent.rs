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
use crate::model::{PaneInfo, Session, Tab};
use crate::store::{self, Busy, Store, ToastKind, args};
use crate::ui::TextField;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Menu {
    Shots,
    Agents,
    Chooser,
}

#[derive(Default)]
pub(crate) struct Agents {
    pub(crate) chosen: HashMap<u64, u64>,
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
    pub(crate) pane: u64,
    pub(crate) label: String,
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

fn tab_name(tab: &Tab) -> Option<String> {
    let name = tab.custom_name.clone().unwrap_or_else(|| tab.name.clone());
    (!name.trim().is_empty()).then_some(name)
}

pub(crate) fn agent_label(pane: &PaneInfo, tab: Option<&Tab>) -> String {
    let name = pane.agent.clone().unwrap_or_else(|| "agent".to_string());
    match pane.agent_title().or_else(|| tab.and_then(tab_name)) {
        Some(title) if !title.eq_ignore_ascii_case(&name) => format!("{name} · {title}"),
        _ => name,
    }
}

pub(crate) fn workspace_agents(session: &Session, workspace: u64) -> Vec<Choice> {
    let Some((_, ws)) = session.workspace(workspace) else { return Vec::new() };
    let label = |pane: &PaneInfo, tab: u64| agent_label(pane, session.tab(tab));
    ws.agent_rows().iter().map(|row| Choice { pane: row.pane.id, label: label(row.pane, row.tab) }).collect()
}

pub(crate) fn pick_agent(session: &Session, remembered: Option<u64>) -> Option<u64> {
    let agents = workspace_agents(session, session.shown().workspace?);
    remembered.filter(|pane| agents.iter().any(|a| a.pane == *pane)).or_else(|| agent_pane(session))
}

pub(crate) fn unready(session: &Session, pane: u64) -> Option<String> {
    let Some(info) = session.pane(pane) else { return Some("That agent is gone".to_string()) };
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

    pub(super) fn agent_endpoint(&self) -> Option<String> {
        let port = self.port()?;
        match cli::host() {
            None => Some(endpoint(port)),
            Some(host) => self.dev.tunnels.reverse(&host, port).map(endpoint),
        }
    }

    pub(crate) fn target_agent(&self, cx: &App) -> Option<u64> {
        let store = Store::get(cx);
        let session = store.read(cx).session.as_ref()?;
        pick_agent(session, self.workspace.and_then(|w| self.agents.chosen.get(&w).copied()))
    }

    pub(super) fn agent_choices(&self, cx: &App) -> Vec<Choice> {
        let store = Store::get(cx);
        let session = store.read(cx).session.as_ref();
        session.zip(self.workspace).map(|(s, w)| workspace_agents(s, w)).unwrap_or_default()
    }

    pub(super) fn agent_name(pane: u64, cx: &App) -> String {
        let store = Store::get(cx);
        let session = store.read(cx).session.as_ref();
        let info = session.and_then(|s| s.pane(pane).map(|p| (p, s.tab_of_pane(pane))));
        info.map_or_else(|| "the agent".to_string(), |(p, tab)| agent_label(p, tab))
    }

    pub(super) fn choose_agent(&mut self, pane: u64, cx: &mut Context<Self>) {
        if let Some(workspace) = self.workspace {
            self.agents.chosen.insert(workspace, pane);
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

    pub(super) fn give_to_agent(pane: u64, text: &str, files: &[&Path], cx: &mut App) -> bool {
        if Self::refused(pane, cx) {
            return false;
        }
        Self::hand_to_agent(pane, text, files, cx);
        true
    }

    pub(super) fn submit_to_agent(pane: u64, text: String, files: &[PathBuf], cx: &mut App) -> bool {
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

#[cfg(test)]
mod tests;
