use std::collections::HashSet;
use std::time::Duration;

use gpui::{Context, Entity};
use serde_json::Value;

use super::Browser;
use super::held::{HeldOpen, Route, route};
use super::tabs::{Restore, Tab, Through};
use super::tunnels::Tunnels;
use crate::cli;
use crate::model::Owner;
use crate::model::link::{loopback_port, rewrite};
use crate::model::tunnels::MachineOpen;
use crate::model::tunnels::Way;
use crate::panels::PanelKind;
use crate::panels::on_machine::Retry;
use crate::panels::poll::{Polled, Watch, parse};
use crate::panels::ports::Listed;
use crate::panels::ports::data::{List, Port, list_command};
use crate::store::{self, Store};

pub(crate) struct DevServers {
    pub(crate) listed: Listed,
    pub(crate) error: Option<String>,
    pub(crate) retry: Retry,
    pub(crate) tunnels: Tunnels,
    pub(super) held_opens: Vec<HeldOpen>,
    pub(super) scope: Option<(Owner, String)>,
    pub(super) generation: u64,
    pub(super) seen: HashSet<(Owner, u16)>,
    watch: Watch,
}

impl DevServers {
    pub(super) fn watch_begin(&mut self) -> bool {
        self.watch.begin()
    }

    pub(super) fn watch_end(&mut self) {
        self.watch.end();
    }

    pub(crate) fn new(cx: &mut Context<Browser>) -> Self {
        let tunnels = Tunnels::new(cx);
        let watch = Watch::start(cx);
        Self {
            listed: Listed::None,
            error: None,
            retry: Retry::default(),
            tunnels,
            held_opens: Vec::new(),
            scope: None,
            generation: 0,
            seen: HashSet::new(),
            watch,
        }
    }

    pub(super) fn forget_server(&mut self) {
        self.reset(None);
        self.seen.clear();
    }

    fn reset(&mut self, scope: Option<(Owner, String)>) {
        self.scope = scope;
        (self.listed, self.error, self.retry) = (Listed::None, None, Retry::default());
        self.generation += 1;
    }

    pub(super) fn follow(&mut self, place: &Owner) -> bool {
        let scope = scope_of(place);
        let moved = scope != self.scope;
        if moved {
            self.reset(scope);
        }
        moved
    }

    pub(crate) fn own_numbers(&self) -> Vec<u16> {
        here_numbers(&self.listed)
    }

    pub(crate) fn listed_numbers(&self) -> Vec<u16> {
        match &self.listed {
            Listed::None => Vec::new(),
            Listed::Here(ports) => ports.iter().map(|p| p.number).collect(),
            Listed::There(ports) => ports.iter().map(|p| p.number).collect(),
        }
    }
}

pub(crate) fn here_numbers(listed: &Listed) -> Vec<u16> {
    match listed {
        Listed::Here(ports) => ports.iter().map(|p| p.number).collect(),
        Listed::None | Listed::There(_) => Vec::new(),
    }
}

pub(crate) fn scope_of(place: &Owner) -> Option<(Owner, String)> {
    match place {
        Owner::Nobody => None,
        Owner::Here(_) => Some((place.clone(), String::new())),
        Owner::There(ws) => Some((place.clone(), ws.destination().to_string())),
    }
}

pub(crate) fn web_ports<Id>(ports: Vec<Port<Id>>) -> Vec<Port<Id>> {
    let mut web: Vec<Port<Id>> = ports.into_iter().filter(|p| p.url.is_some()).collect();
    web.sort_by_key(|p| p.number);
    web.dedup_by_key(|p| p.number);
    web
}

pub(crate) fn appeared<Id>(ports: &[Port<Id>], seen: &mut HashSet<(Owner, u16)>, owner: &Owner) -> Option<String> {
    let first = ports.iter().find(|p| !seen.contains(&(owner.clone(), p.number))).and_then(|p| p.url.clone());
    seen.extend(ports.iter().map(|p| (owner.clone(), p.number)));
    first
}

pub(crate) fn reused_tab(tabs: &[Tab], owner: &Owner, url: &str, through: Option<Through>) -> Option<(String, bool)> {
    let mine = || tabs.iter().filter(|t| t.owner == *owner);
    if let Some(tab) = mine().find(|t| same_page(&t.url, url)) {
        return Some((tab.target.clone(), true));
    }
    let new = through?;
    let old_hold = |t: &&Tab| {
        let held = t.through.filter(|h| h.remote == new.remote && loopback_port(&t.url) == Some(h.local));
        held.is_some_and(|h| h.local != new.local && same_page(&rewrite(&t.url, new.local), url))
    };
    mine().find(old_hold).map(|tab| (tab.target.clone(), false))
}

pub(crate) fn origin(url: &str) -> &str {
    let start = url.find("://").map_or(0, |at| at + 3);
    let end = url[start..].find(['/', '?', '#']).map_or(url.len(), |at| start + at);
    &url[..end]
}

pub(crate) fn same_page(shown: &str, asked: &str) -> bool {
    match &asked[origin(asked).len()..] {
        "" | "/" => origin(shown) == origin(asked),
        _ => shown == asked,
    }
}

impl Polled for Browser {
    const KIND: PanelKind = PanelKind::Browser;
    const EVERY: Duration = Duration::from_secs(2);

    fn watch(&mut self) -> &mut Watch {
        &mut self.dev.watch
    }

    fn measure(&mut self, cx: &mut Context<Self>) {
        let (shown, place) = super::there::on_screen(cx);
        self.dev.follow(&place);
        if !shown {
            return;
        }
        match place {
            Owner::Here(workspace) => self.measure_here(workspace, cx),
            Owner::There(_) => self.measure_there(cx),
            Owner::Nobody => {}
        }
    }
}

impl Browser {
    fn measure_here(&mut self, workspace: u64, cx: &mut Context<Self>) {
        if !self.dev.watch.begin() {
            return;
        }
        let command = list_command(false, Some(workspace));
        let generation = self.dev.generation;
        store::run_json(cx, command, move |this, result, cx| this.sampled(generation, result, cx));
    }

    pub(super) fn dev_changed(&mut self, store: &Entity<Store>, cx: &mut Context<Self>) {
        if self.dev.follow(&self.place) {
            self.measure(cx);
        }
        let asked = store.read(cx).browse.is_some();
        if let Some((url, owner)) = asked.then(|| store.update(cx, |s, _| s.browse.take())).flatten() {
            self.browse(url, owner, cx);
        }
    }

    fn sampled(&mut self, generation: u64, result: anyhow::Result<Value>, cx: &mut Context<Self>) {
        self.dev.watch.end();
        if generation != self.dev.generation {
            self.measure(cx);
            return;
        }
        match parse::<List>(result) {
            Ok(list) => (self.dev.listed, self.dev.error) = (Listed::Here(web_ports(list.ports)), None),
            Err(e) => self.dev.error = Some(e),
        }
        let own = matches!(Store::get(cx).read(cx).tunnel_way(), Way::Own(_));
        self.keep_tunnels(own, cx);
        self.auto_open(cx);
        cx.notify();
    }

    pub(super) fn auto_open(&mut self, cx: &mut Context<Self>) {
        let owner = self.place.clone();
        if let Owner::There(ws) = &owner
            && Store::get(cx).read(cx).machine_open(ws.machine()) != MachineOpen::Open
        {
            return;
        }
        let url = match &self.dev.listed {
            Listed::None => return,
            Listed::Here(ports) => appeared(ports, &mut self.dev.seen, &owner),
            Listed::There(ports) => appeared(ports, &mut self.dev.seen, &owner),
        };
        let opening = self.dev.tunnels.pending_in(&owner)
            || self.dev.held_opens.iter().any(|open| open.owned_by(&owner))
            || self.restore.iter().any(|r| r.owner == owner);
        let busy = opening || self.tabs.active(&owner).is_some();
        if let Some(url) = url.filter(|_| !busy) {
            self.browse(url, owner, cx);
        }
    }

    pub(super) fn browse(&mut self, url: String, owner: Owner, cx: &mut Context<Self>) {
        let toward = owner.toward(cli::host().as_deref());
        let store = Store::get(cx).read(cx);
        let port = loopback_port(&url);
        let open = owner.machine().map(|name| store.machine_open_at(name, port));
        match route(&url, toward.as_ref(), store.tunnel_way(), open.as_ref()) {
            Route::Direct => self.open_now(url, owner, None, cx),
            Route::Own(remote) => self.browse_through(remote, url, owner, cx),
            Route::Held(_, remote) => self.browse_held((remote, url), (owner, false), cx),
            Route::Refused(why) => Store::get(cx).update(cx, |s, cx| s.not_opened(&why, cx)),
        }
    }

    pub(super) fn open_now(&mut self, url: String, owner: Owner, through: Option<Through>, cx: &mut Context<Self>) {
        let shown = reused_tab(&self.tabs.list, &owner, &url, through);
        if let Some((target, direct)) = shown {
            if let Some(tab) = self.tabs.get_mut(&target).filter(|_| direct) {
                tab.through = through.or(tab.through);
            }
            self.tabs.activate(owner, &target);
            self.recast(cx);
        } else if self.cdp().is_some() {
            self.open_tab(&url, owner, through, cx);
        } else {
            self.restore.push(Restore { url, owner, through });
        }
    }
}

#[cfg(test)]
mod tests;
