use std::collections::HashSet;
use std::time::Duration;

use gpui::{App, Context, Entity};
use serde_json::Value;

use super::Browser;
use super::tunnels::{Tunnels, loopback_port};
use crate::cli;
use crate::panels::PanelKind;
use crate::panels::poll::{Polled, Watch, parse};
use crate::panels::ports::data::{List, Port, list_command};
use crate::store::{self, Store};

pub(crate) struct DevServers {
    pub(crate) ports: Option<Vec<Port>>,
    pub(crate) error: Option<String>,
    pub(crate) tunnels: Tunnels,
    workspace: Option<u64>,
    seen: HashSet<(u64, u16)>,
    watch: Watch,
}

impl DevServers {
    pub(crate) fn new(cx: &mut Context<Browser>) -> Self {
        let tunnels = Tunnels::new(cx);
        let watch = Watch::start(cx);
        Self { ports: None, error: None, tunnels, workspace: None, seen: HashSet::new(), watch }
    }

    fn follow(&mut self, workspace: Option<u64>) -> bool {
        let moved = workspace != self.workspace;
        if moved {
            (self.workspace, self.ports, self.error) = (workspace, None, None);
        }
        moved
    }
}

pub(crate) fn web_ports(ports: Vec<Port>) -> Vec<Port> {
    let mut web: Vec<Port> = ports.into_iter().filter(|p| p.url.is_some()).collect();
    web.sort_by_key(|p| p.number);
    web.dedup_by_key(|p| p.number);
    web
}

pub(crate) fn appeared(ports: &[Port], seen: &mut HashSet<(u64, u16)>, workspace: u64) -> Option<String> {
    let first = ports.iter().find(|p| !seen.contains(&(workspace, p.number))).and_then(|p| p.url.clone());
    seen.extend(ports.iter().map(|p| (workspace, p.number)));
    first
}

pub(crate) fn origin(url: &str) -> &str {
    let start = url.find("://").map_or(0, |at| at + 3);
    let end = url[start..].find(['/', '?', '#']).map_or(url.len(), |at| start + at);
    &url[..end]
}

fn on_screen(cx: &App) -> (bool, Option<u64>) {
    let store = Store::get(cx).read(cx);
    (store.dock == Some(PanelKind::Browser), store.session.as_ref().and_then(|s| s.shown().workspace))
}

impl Polled for Browser {
    const KIND: PanelKind = PanelKind::Browser;
    const EVERY: Duration = Duration::from_secs(2);

    fn watch(&mut self) -> &mut Watch {
        &mut self.dev.watch
    }

    fn measure(&mut self, cx: &mut Context<Self>) {
        let (shown, workspace) = on_screen(cx);
        self.dev.follow(workspace);
        let Some(workspace) = workspace.filter(|_| shown) else { return };
        if !self.dev.watch.begin() {
            return;
        }
        let command = list_command(false, Some(workspace));
        store::run_json(cx, command, move |this, result, cx| this.sampled(workspace, result, cx));
    }
}

impl Browser {
    pub(super) fn dev_changed(&mut self, store: &Entity<Store>, cx: &mut Context<Self>) {
        if self.dev.follow(on_screen(cx).1) {
            self.measure(cx);
        }
        let asked = store.read(cx).browse.is_some();
        if let Some((url, workspace)) = asked.then(|| store.update(cx, |s, _| s.browse.take())).flatten() {
            self.browse(url, workspace.or(self.workspace), cx);
        }
    }

    fn sampled(&mut self, workspace: u64, result: anyhow::Result<Value>, cx: &mut Context<Self>) {
        self.dev.watch.end();
        if self.dev.workspace != Some(workspace) {
            self.measure(cx);
            return;
        }
        match parse::<List>(result) {
            Ok(list) => (self.dev.ports, self.dev.error) = (Some(web_ports(list.ports)), None),
            Err(e) => self.dev.error = Some(e),
        }
        self.keep_tunnels(cx);
        self.auto_open(workspace, cx);
        cx.notify();
    }

    fn auto_open(&mut self, workspace: u64, cx: &mut Context<Self>) {
        let Some(ports) = &self.dev.ports else { return };
        let url = appeared(ports, &mut self.dev.seen, workspace);
        let opening =
            self.dev.tunnels.pending_in(Some(workspace)) || self.restore.iter().any(|r| r.1 == Some(workspace));
        let busy = opening || self.tabs.active(Some(workspace)).is_some();
        if let Some(url) = url.filter(|_| !busy) {
            self.browse(url, Some(workspace), cx);
        }
    }

    pub(super) fn browse(&mut self, url: String, workspace: Option<u64>, cx: &mut Context<Self>) {
        match loopback_port(&url).filter(|_| cli::host().is_some()) {
            Some(remote) => self.browse_through(remote, url, workspace, cx),
            None => self.open_now(url, workspace, cx),
        }
    }

    pub(super) fn open_now(&mut self, url: String, workspace: Option<u64>, cx: &mut Context<Self>) {
        let shown = self.tabs.list.iter().find(|t| t.workspace == workspace && origin(&t.url) == origin(&url));
        if let Some(target) = shown.map(|t| t.target.clone()) {
            self.tabs.activate(workspace, &target);
            self.recast(cx);
        } else if self.cdp().is_some() {
            self.open_tab(&url, workspace, cx);
        } else {
            self.restore.push((url, workspace));
        }
    }
}

#[cfg(test)]
mod tests;
