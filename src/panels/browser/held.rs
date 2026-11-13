use gpui::{App, Context, Entity};
use serde_json::json;

use super::Browser;
use super::tabs::Through;
use crate::model::Owner;
use crate::model::link::{loopback_port, rewrite};
use crate::model::tunnels::MachineOpen;
use crate::model::tunnels::{Toward, Way};
use crate::store::Store;
use crate::store::{Asked, Held};
use crate::{cli, desktop};

pub(crate) struct HeldOpen {
    remote: u16,
    url: String,
    owner: Owner,
    toward: Toward,
    navigate: bool,
    session: Option<String>,
    asked: bool,
}

impl HeldOpen {
    #[cfg(test)]
    pub(crate) fn waiting(remote: u16, session: Option<&str>) -> Self {
        Self {
            remote,
            url: format!("http://localhost:{remote}"),
            owner: Owner::Nobody,
            toward: Toward::Server,
            navigate: true,
            session: session.map(str::to_string),
            asked: false,
        }
    }

    pub(super) fn owned_by(&self, owner: &Owner) -> bool {
        self.owner == *owner
    }

    pub(super) fn owner_mut(&mut self) -> &mut Owner {
        &mut self.owner
    }

    pub(super) fn owner(&self) -> &Owner {
        &self.owner
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Route {
    Direct,
    Held(Toward, u16),
    Own(u16),
    Refused(String),
}

pub(crate) fn route(url: &str, toward: Option<&Toward>, way: Way, machine: Option<&MachineOpen>) -> Route {
    let (Some(remote), Some(toward)) = (loopback_port(url), toward) else { return Route::Direct };
    match toward {
        Toward::Machine(_) => match machine {
            Some(MachineOpen::Closed(why)) => Route::Refused(why.clone()),
            Some(MachineOpen::Open | MachineOpen::Waiting) => Route::Held(toward.clone(), remote),
            None => Route::Refused("this machine is not read yet".to_string()),
        },
        Toward::Server => match way {
            Way::Own(_) => Route::Own(remote),
            Way::Cornercase | Way::Waiting => Route::Held(Toward::Server, remote),
            Way::Here => Route::Direct,
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Dispatch {
    Wait,
    Direct,
    Own,
    Hold,
}

pub(crate) fn dispatch(toward: &Toward, way: Way) -> Dispatch {
    match (toward, way) {
        (Toward::Machine(_), _) | (Toward::Server, Way::Cornercase) => Dispatch::Hold,
        (Toward::Server, Way::Waiting) => Dispatch::Wait,
        (Toward::Server, Way::Here) => Dispatch::Direct,
        (Toward::Server, Way::Own(_)) => Dispatch::Own,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Step {
    Open(u16),
    Wait,
    Drop,
}

pub(crate) fn asks_now(asked: bool, held: Option<&Held>) -> bool {
    !asked && held != Some(&Held::Ending)
}

pub(crate) fn superseded(opens: &mut Vec<HeldOpen>, session: &str) {
    opens.retain(|open| open.session.as_deref() != Some(session));
}

pub(crate) fn superseded_typed(opens: &mut Vec<HeldOpen>, owner: &Owner) {
    opens.retain(|open| !(open.navigate && open.session.is_none() && open.owner == *owner));
}

pub(crate) fn step(held: Option<&Held>, asked: bool) -> Step {
    match held {
        Some(Held::Up { local }) => Step::Open(*local),
        Some(Held::Opening | Held::Down { .. }) => Step::Wait,
        Some(Held::Ending) | None => {
            if asked {
                Step::Drop
            } else {
                Step::Wait
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Passage {
    Through(u16),
    Waiting,
    Not,
    Closed(String),
}

pub(crate) fn passage(way: &Way, held: Option<&Held>) -> Passage {
    match (way, held) {
        (Way::Cornercase, Some(Held::Up { local })) => Passage::Through(*local),
        (Way::Cornercase, Some(Held::Opening)) | (Way::Waiting, _) => Passage::Waiting,
        _ => Passage::Not,
    }
}

pub(crate) fn repointed(url: &str, through: Through, now: Option<u16>) -> Option<(String, Through)> {
    let local = now.filter(|local| *local != through.local)?;
    (loopback_port(url) == Some(through.local)).then(|| (rewrite(url, local), Through { local, ..through }))
}

pub(crate) fn inherited(opener: Option<Through>, url: &str) -> Option<Through> {
    let blank = url.is_empty() || url == "about:blank";
    opener.filter(|through| blank || loopback_port(url) == Some(through.local))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Typed {
    Through(u16),
    Here,
    HereSaid(u16),
}

pub(crate) fn typed_route(port: Option<u16>, toward: Option<&Toward>, known: (bool, bool)) -> Typed {
    let (known, own_local) = known;
    match (port, toward) {
        (Some(_), Some(_)) if own_local => Typed::Here,
        (Some(port), Some(_)) if known => Typed::Through(port),
        (Some(port), Some(_)) => Typed::HereSaid(port),
        _ => Typed::Here,
    }
}

pub(crate) fn said_here(port: u16, whose: &str) -> String {
    format!(
        "Opened this {}'s localhost:{port}: {whose} lists no web port {port}, so it was not tunnelled",
        desktop::here()
    )
}

pub(crate) fn machine_passage(open: &MachineOpen, held: Option<&Held>) -> Passage {
    match open {
        MachineOpen::Open | MachineOpen::Waiting => passage(&Way::Cornercase, held),
        MachineOpen::Closed(why) => Passage::Closed(why.clone()),
    }
}

impl Browser {
    pub(super) fn browse_held(
        &mut self,
        (remote, url): (u16, String),
        (owner, navigate): (Owner, bool),
        cx: &mut Context<Self>,
    ) {
        let toward = owner.toward(cli::host().as_deref()).unwrap_or(Toward::Server);
        let session = self.active_session().filter(|_| navigate);
        if let Some(session) = &session {
            superseded(&mut self.dev.held_opens, session);
        }
        let same = |o: &HeldOpen| {
            (o.remote, &o.url, &o.owner, o.navigate, &o.session) == (remote, &url, &owner, navigate, &session)
        };
        if !self.dev.held_opens.iter().any(same) {
            let open = HeldOpen { remote, url, owner, toward, navigate, session, asked: false };
            self.dev.held_opens.push(open);
        }
        self.held_changed(cx);
    }

    pub(super) fn held_changed(&mut self, cx: &mut Context<Self>) {
        if self.dev.held_opens.is_empty() {
            return;
        }
        let store = Store::get(cx);
        let way = store.read(cx).tunnel_way();
        let mut holds = Vec::new();
        for open in std::mem::take(&mut self.dev.held_opens) {
            match dispatch(&open.toward, way) {
                Dispatch::Wait => self.dev.held_opens.push(open),
                Dispatch::Direct => self.open_now(open.url, open.owner, None, cx),
                Dispatch::Own => self.browse_through(open.remote, open.url, open.owner, cx),
                Dispatch::Hold => holds.push(open),
            }
        }
        self.follow_holds(holds, &store, cx);
    }

    fn follow_holds(&mut self, opens: Vec<HeldOpen>, store: &Entity<Store>, cx: &mut Context<Self>) {
        let mut waiting = Vec::new();
        for mut open in opens {
            match Self::machine_gate(&open, store, cx) {
                Some(Step::Wait) => {
                    waiting.push(open);
                    continue;
                }
                Some(_) => continue,
                None => {}
            }
            let toward = open.toward.clone();
            let before = store.read(cx).held_tunnel(&toward, open.remote).cloned();
            if asks_now(open.asked, before.as_ref()) {
                store.update(cx, |s, cx| s.hold_tunnel(&toward, open.remote, Asked::Open, cx));
                open.asked = true;
            }
            let held = store.read(cx).held_tunnel(&toward, open.remote).cloned();
            match step(held.as_ref(), open.asked) {
                Step::Open(local) => self.open_held(&open, local, cx),
                Step::Wait => waiting.push(open),
                Step::Drop => {}
            }
        }
        self.dev.held_opens.append(&mut waiting);
        cx.notify();
    }

    fn machine_gate(open: &HeldOpen, store: &Entity<Store>, cx: &mut Context<Self>) -> Option<Step> {
        let Toward::Machine(name) = &open.toward else { return None };
        match store.read(cx).machine_open(name) {
            MachineOpen::Open => None,
            MachineOpen::Waiting => Some(Step::Wait),
            MachineOpen::Closed(why) => {
                store.update(cx, |s, cx| s.not_opened(&why, cx));
                Some(Step::Drop)
            }
        }
    }

    fn open_held(&mut self, open: &HeldOpen, local: u16, cx: &mut Context<Self>) {
        let url = rewrite(&open.url, local);
        let through = Some(Through { remote: open.remote, local });
        let session = open.session.as_ref().filter(|session| self.tabs.by_session(session).is_some());
        match session {
            Some(session) => {
                if let Some(tab) = self.tabs.by_session_mut(Some(session)) {
                    tab.through = through;
                }
                self.send(Some(session), "Page.navigate", json!({ "url": url }));
            }
            None => self.open_now(url, open.owner.clone(), through, cx),
        }
    }

    pub(super) fn passage_to(&mut self, toward: &Toward, remote: u16, cx: &mut Context<Self>) -> Passage {
        let store = Store::get(cx).read(cx);
        if let Toward::Machine(name) = toward {
            return machine_passage(&store.machine_open(name), store.held_tunnel(toward, remote));
        }
        let way = store.tunnel_way();
        if matches!(way, Way::Own(_)) {
            return self.tunnel_to(remote, cx).map_or(Passage::Waiting, Passage::Through);
        }
        passage(&way, Store::get(cx).read(cx).held_tunnel(&Toward::Server, remote))
    }

    pub(super) fn supersede_waiting(&mut self) {
        match self.active_session() {
            Some(session) => superseded(&mut self.dev.held_opens, &session),
            None => superseded_typed(&mut self.dev.held_opens, &self.place),
        }
    }

    pub(super) fn left_server_now(&mut self, cx: &mut Context<Self>) {
        for target in tabs_of_a_server(self.tabs.list.iter().map(|t| (t.target.as_str(), &t.owner))) {
            self.close_tab(&target);
        }
        self.restore.retain(|r| !r.owner.has_place());
        let claimed: Vec<String> =
            self.claims.iter().filter(|(_, c)| c.owner.has_place()).map(|(t, _)| t.clone()).collect();
        for target in claimed {
            self.claims.remove(&target);
            self.close_tab(&target);
        }
        self.server_generation += 1;
        self.agents.chosen.retain(|owner, _| !owner.has_place());
        super::tunnels::quit();
        self.dev.tunnels.clear(cx);
        self.dev.held_opens.clear();
        self.dev.forget_server();
        cx.notify();
    }

    pub(super) fn repoint_tabs(&mut self, store: &Entity<Store>, cx: &mut Context<Self>) {
        let host = cli::host();
        let s = store.read(cx);
        let mut moves = Vec::new();
        for tab in &self.tabs.list {
            let (Some(through), Some(session), Some(toward)) =
                (tab.through, &tab.session, tab.owner.toward(host.as_deref()))
            else {
                continue;
            };
            if let Some((url, now)) = repointed(&tab.url, through, s.tunnel_local(&toward, through.remote)) {
                moves.push((tab.target.clone(), session.clone(), url, now));
            }
        }
        for (target, session, url, through) in moves {
            if let Some(tab) = self.tabs.get_mut(&target) {
                tab.through = Some(through);
            }
            self.send(Some(&session), "Page.navigate", json!({ "url": url }));
        }
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

pub(crate) fn left_server(cx: &mut App) {
    if let Some(browser) = super::running(cx) {
        browser.update(cx, Browser::left_server_now);
    }
}

pub(crate) fn tabs_of_a_server<'a>(tabs: impl Iterator<Item = (&'a str, &'a Owner)>) -> Vec<String> {
    tabs.filter(|(_, owner)| owner.has_place()).map(|(target, _)| target.to_string()).collect()
}

#[cfg(test)]
mod tests;
