use std::collections::HashMap;
use std::time::Duration;

use gpui::{Context, Entity};
use serde_json::json;

use super::Browser;
use super::tabs::Tabs;
use crate::model::Session;
use crate::store::Store;

const SETTLE: Duration = Duration::from_millis(1500);
const FLASH: Duration = Duration::from_millis(2500);

#[derive(Default)]
pub(crate) struct Watch {
    pub(crate) off: bool,
    pub(crate) flash: Option<(u64, String)>,
    working: HashMap<u64, u64>,
    pending: HashMap<u64, u64>,
    serial: u64,
}

impl Watch {
    fn next(&mut self) -> u64 {
        self.serial += 1;
        self.serial
    }
}

pub(crate) fn working_agents(session: &Session) -> HashMap<u64, u64> {
    let placed = session
        .projects
        .iter()
        .flat_map(|p| &p.workspaces)
        .flat_map(|w| w.tabs.iter().flat_map(|t| &t.panes).map(move |pane| (w.id, pane)));
    placed
        .filter(|(_, pane)| pane.agent.is_some() && pane.status.as_deref() == Some("working"))
        .map(|(workspace, pane)| (pane.id, workspace))
        .collect()
}

pub(crate) fn finished(before: &HashMap<u64, u64>, now: &HashMap<u64, u64>, session: &Session) -> Vec<(u64, u64)> {
    let mut done: Vec<(u64, u64)> = before
        .iter()
        .filter(|(pane, _)| !now.contains_key(pane) && session.pane(**pane).is_some())
        .map(|(pane, workspace)| (*workspace, *pane))
        .collect();
    done.sort_unstable();
    done
}

pub(crate) fn web(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

pub(crate) fn reloadable(tabs: &Tabs, workspace: u64) -> Vec<String> {
    let mine = tabs.list.iter().filter(|t| t.workspace == Some(workspace) && web(&t.url));
    mine.filter_map(|t| t.session.clone()).collect()
}

impl Browser {
    pub(super) fn agents_changed(&mut self, store: &Entity<Store>, cx: &mut Context<Self>) {
        let Some(session) = store.read(cx).session.as_ref() else { return };
        let now = working_agents(session);
        let done = finished(&self.agents.watch.working, &now, session);
        self.agents.watch.working = now;
        if self.agents.watch.off {
            return;
        }
        for (workspace, pane) in done {
            self.reload_soon(workspace, pane, cx);
        }
    }

    fn reload_soon(&mut self, workspace: u64, pane: u64, cx: &mut Context<Self>) {
        let serial = self.agents.watch.next();
        self.agents.watch.pending.insert(workspace, serial);
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SETTLE).await;
            let _ = this.update(cx, |this, cx| this.reload_after(workspace, (pane, serial), cx));
        })
        .detach();
    }

    fn reload_after(&mut self, workspace: u64, (pane, serial): (u64, u64), cx: &mut Context<Self>) {
        let watch = &mut self.agents.watch;
        if watch.pending.get(&workspace) != Some(&serial) {
            return;
        }
        watch.pending.remove(&workspace);
        if watch.off || watch.working.contains_key(&pane) || self.picking() {
            return;
        }
        let sessions = reloadable(&self.tabs, workspace);
        for session in &sessions {
            self.send(Some(session), "Page.reload", json!({}));
        }
        if !sessions.is_empty() {
            let store = Store::get(cx);
            let name = store.read(cx).session.as_ref().and_then(|s| s.pane(pane)).and_then(|p| p.agent.clone());
            self.flash(format!("Reloaded: {} finished", name.as_deref().unwrap_or("an agent")), cx);
        }
    }

    fn flash(&mut self, label: String, cx: &mut Context<Self>) {
        let serial = self.agents.watch.next();
        self.agents.watch.flash = Some((serial, label));
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(FLASH).await;
            let _ = this.update(cx, |this, cx| this.unflash(serial, cx));
        })
        .detach();
    }

    fn unflash(&mut self, serial: u64, cx: &mut Context<Self>) {
        if self.agents.watch.flash.as_ref().is_some_and(|(s, _)| *s == serial) {
            self.agents.watch.flash = None;
            cx.notify();
        }
    }
}

#[cfg(test)]
mod tests;
