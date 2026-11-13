use std::collections::HashMap;
use std::time::Duration;

use gpui::{Context, Entity};
use serde_json::json;

use super::Browser;
use super::tabs::Tabs;
use crate::model::machines::Machine;
use crate::model::{Owner, PaneThere, Session, WorkspaceThere};
use crate::store::Store;

const SETTLE: Duration = Duration::from_millis(1500);
const FLASH: Duration = Duration::from_millis(2500);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Working {
    Here(u64),
    There(PaneThere),
}

#[derive(Default)]
pub(crate) struct Watch {
    pub(crate) off: bool,
    pub(crate) flash: Option<(u64, String)>,
    working: HashMap<Working, Owner>,
    pending: HashMap<Owner, u64>,
    serial: u64,
}

impl Watch {
    fn next(&mut self) -> u64 {
        self.serial += 1;
        self.serial
    }
}

fn working_here(session: &Session) -> impl Iterator<Item = (Working, Owner)> + '_ {
    let placed = session
        .projects
        .iter()
        .flat_map(|p| &p.workspaces)
        .flat_map(|w| w.tabs.iter().flat_map(|t| &t.panes).map(move |pane| (w.id, pane)));
    let busy = placed.filter(|(_, pane)| pane.agent.is_some() && pane.status.as_deref() == Some("working"));
    busy.map(|(workspace, pane)| (Working::Here(pane.id), Owner::Here(workspace)))
}

fn working_there<'a>(session: &'a Session, machines: &'a [Machine]) -> impl Iterator<Item = (Working, Owner)> + 'a {
    let linked = session.machines.iter().filter(|seen| seen.linked());
    linked.flat_map(move |seen| {
        let destination = machines.iter().find(|m| m.name == seen.name).map_or("", |m| m.destination.as_str());
        seen.workspaces().flat_map(move |ws| {
            let panes = ws.tabs.iter().flat_map(|tab| &tab.panes);
            let busy = panes.filter(|pane| pane.agent.is_some() && pane.status.as_deref() == Some("working"));
            busy.map(move |pane| {
                let working =
                    Working::There(PaneThere { machine: seen.name.clone(), boot: seen.boot.clone(), pane: pane.id });
                (working, Owner::There(WorkspaceThere::at(seen, ws, destination)))
            })
        })
    })
}

pub(crate) fn working_agents(session: &Session, machines: &[Machine]) -> HashMap<Working, Owner> {
    working_here(session).chain(working_there(session, machines)).collect()
}

fn still_listed(session: &Session, working: &Working) -> bool {
    match working {
        Working::Here(pane) => session.pane(*pane).is_some(),
        Working::There(there) => there.sighted(&session.machines).is_ok(),
    }
}

fn program_of(session: &Session, working: &Working) -> Option<String> {
    match working {
        Working::Here(pane) => session.pane(*pane)?.agent.clone(),
        Working::There(there) => session.machine(&there.machine)?.pane(there.pane)?.agent.clone(),
    }
}

pub(crate) fn finished(
    before: &HashMap<Working, Owner>,
    now: &HashMap<Working, Owner>,
    session: &Session,
) -> Vec<(Working, Owner)> {
    let gone = before.iter().filter(|(working, _)| !now.contains_key(*working) && still_listed(session, working));
    let mut done: Vec<(Working, Owner)> = gone.map(|(working, owner)| (working.clone(), owner.clone())).collect();
    done.sort_by_key(|(working, _)| format!("{working:?}"));
    done
}

pub(crate) fn web(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

pub(crate) fn reloadable(tabs: &Tabs, owner: &Owner) -> Vec<String> {
    let mine = tabs.list.iter().filter(|t| t.owner == *owner && web(&t.url));
    mine.filter_map(|t| t.session.clone()).collect()
}

impl Browser {
    pub(super) fn agents_changed(&mut self, store: &Entity<Store>, cx: &mut Context<Self>) {
        let s = store.read(cx);
        let Some(session) = s.session.as_ref() else { return };
        let now = working_agents(session, &s.machines);
        let done = finished(&self.agents.watch.working, &now, session);
        self.agents.watch.working = now;
        if self.agents.watch.off {
            return;
        }
        for (working, owner) in done {
            self.reload_soon(owner, working, cx);
        }
    }

    fn reload_soon(&mut self, owner: Owner, working: Working, cx: &mut Context<Self>) {
        let serial = self.agents.watch.next();
        self.agents.watch.pending.insert(owner.clone(), serial);
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SETTLE).await;
            let _ = this.update(cx, |this, cx| this.reload_after(&owner, (working, serial), cx));
        })
        .detach();
    }

    fn reload_after(&mut self, owner: &Owner, (working, serial): (Working, u64), cx: &mut Context<Self>) {
        let watch = &mut self.agents.watch;
        if watch.pending.get(owner) != Some(&serial) {
            return;
        }
        watch.pending.remove(owner);
        if watch.off || watch.working.contains_key(&working) || self.picking() {
            return;
        }
        let sessions = reloadable(&self.tabs, owner);
        for session in &sessions {
            self.send(Some(session), "Page.reload", json!({}));
        }
        if !sessions.is_empty() {
            let store = Store::get(cx);
            let name = store.read(cx).session.as_ref().and_then(|s| program_of(s, &working));
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
