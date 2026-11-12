use gpui::{App, Context, Entity};
use serde_json::json;

use super::Browser;
use crate::cli;
use crate::model::link::{loopback_port, rewrite};
use crate::model::tunnels::Way;
use crate::store::Store;
use crate::store::{Asked, Held};

pub(crate) struct HeldOpen {
    remote: u16,
    url: String,
    workspace: Option<u64>,
    navigate: bool,
    asked: bool,
}

impl HeldOpen {
    pub(super) fn in_workspace(&self, workspace: Option<u64>) -> bool {
        self.workspace == workspace
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Step {
    Open(u16),
    Wait,
    Drop,
}

pub(crate) fn step(held: Option<&Held>, asked: bool) -> Step {
    match held {
        Some(Held::Up { local }) => Step::Open(*local),
        Some(Held::Opening | Held::Down { .. }) => Step::Wait,
        None | Some(Held::Ending) => {
            if asked {
                Step::Drop
            } else {
                Step::Wait
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Passage {
    Through(u16),
    Waiting,
    Not,
}

pub(crate) fn passage(way: &Way, held: Option<&Held>) -> Passage {
    match (way, held) {
        (Way::Cornercase, Some(Held::Up { local })) => Passage::Through(*local),
        (Way::Cornercase, Some(Held::Opening)) | (Way::Waiting, _) => Passage::Waiting,
        _ => Passage::Not,
    }
}

impl Browser {
    pub(super) fn browse_held(
        &mut self,
        (remote, url): (u16, String),
        (workspace, navigate): (Option<u64>, bool),
        cx: &mut Context<Self>,
    ) {
        self.dev.held_opens.push(HeldOpen { remote, url, workspace, navigate, asked: false });
        self.held_changed(cx);
    }

    pub(super) fn held_changed(&mut self, cx: &mut Context<Self>) {
        if self.dev.held_opens.is_empty() {
            return;
        }
        let store = Store::get(cx);
        let way = store.read(cx).tunnel_way();
        match way {
            Way::Waiting => {}
            Way::Here => self.hand_over(cx, |this, open, cx| this.open_now(open.url, open.workspace, cx)),
            Way::Own(_) => {
                self.hand_over(cx, |this, open, cx| this.browse_through(open.remote, open.url, open.workspace, cx));
            }
            Way::Cornercase => self.follow_holds(&store, cx),
        }
    }

    fn hand_over(&mut self, cx: &mut Context<Self>, open: impl Fn(&mut Self, HeldOpen, &mut Context<Self>)) {
        for held in std::mem::take(&mut self.dev.held_opens) {
            open(self, held, cx);
        }
    }

    fn follow_holds(&mut self, store: &Entity<Store>, cx: &mut Context<Self>) {
        let mut waiting = Vec::new();
        for mut open in std::mem::take(&mut self.dev.held_opens) {
            if !open.asked {
                store.update(cx, |s, cx| s.hold_tunnel(open.remote, Asked::Open, cx));
                open.asked = true;
            }
            let held = store.read(cx).held_tunnel(open.remote).cloned();
            match step(held.as_ref(), open.asked) {
                Step::Open(local) => self.open_held(&open, local, cx),
                Step::Wait => waiting.push(open),
                Step::Drop => {}
            }
        }
        waiting.append(&mut self.dev.held_opens);
        self.dev.held_opens = waiting;
        cx.notify();
    }

    fn open_held(&mut self, open: &HeldOpen, local: u16, cx: &mut Context<Self>) {
        let url = rewrite(&open.url, local);
        match self.active_session().filter(|_| open.navigate) {
            Some(session) => self.send(Some(&session), "Page.navigate", json!({ "url": url })),
            None => self.open_now(url, open.workspace, cx),
        }
    }

    pub(super) fn through_cornercase(&self, url: &str, cx: &Context<Self>) -> Option<u16> {
        let remote = loopback_port(url).filter(|_| cli::host().is_some())?;
        let store = Store::get(cx).read(cx);
        let listed = self.dev.ports.iter().flatten().any(|p| p.number == remote);
        let known = listed || store.held_tunnel(remote).is_some();
        (store.tunnel_way() == Way::Cornercase && known).then_some(remote)
    }

    pub(super) fn passage_to(&mut self, remote: u16, cx: &mut Context<Self>) -> Passage {
        let way = Store::get(cx).read(cx).tunnel_way();
        if matches!(way, Way::Own(_)) {
            return self.tunnel_to(remote, cx).map_or(Passage::Waiting, Passage::Through);
        }
        passage(&way, Store::get(cx).read(cx).held_tunnel(remote))
    }

    pub(super) fn left_server_now(&mut self, was_remote: bool, cx: &mut Context<Self>) {
        for target in tabs_of_a_server(self.tabs.list.iter().map(|t| (t.target.as_str(), t.workspace)), was_remote) {
            self.close_tab(&target);
        }
        if was_remote {
            self.restore.clear();
        }
        super::tunnels::quit();
        self.dev.tunnels.clear(cx);
        self.dev.held_opens.clear();
        self.dev.forget_server();
        cx.notify();
    }

    fn follow_store(&mut self, store: &Entity<Store>, cx: &mut Context<Self>) {
        self.watching = cx.observe(store, |this, store, cx| this.store_changed(&store, cx));
        self.store_changed(store, cx);
    }
}

pub(crate) fn follow_store(store: &Entity<Store>, cx: &mut App) {
    if let Some(browser) = super::running(cx) {
        let store = store.clone();
        browser.update(cx, move |browser, cx| browser.follow_store(&store, cx));
    }
}

pub(crate) fn left_server(was_remote: bool, cx: &mut App) {
    if let Some(browser) = super::running(cx) {
        browser.update(cx, move |browser, cx| browser.left_server_now(was_remote, cx));
    }
}

pub(crate) fn tabs_of_a_server<'a>(
    tabs: impl Iterator<Item = (&'a str, Option<u64>)>,
    was_remote: bool,
) -> Vec<String> {
    tabs.filter(|(_, workspace)| was_remote && workspace.is_some()).map(|(target, _)| target.to_string()).collect()
}

#[cfg(test)]
mod tests;
