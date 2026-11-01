mod ssh;

use std::collections::BTreeSet;

use anyhow::anyhow;
use futures::StreamExt as _;
use futures::channel::mpsc::{UnboundedSender, unbounded};
use gpui::Context;

use super::{Browser, say};
use crate::cli;
use crate::model::link::loopback_port;
use crate::store::{Busy, Store};

pub(crate) use ssh::quit;

const PICKS: usize = 8;

pub(crate) enum Event {
    Ready(u64),
    Ended(u64, String),
    ReverseReady,
    ReverseLost(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum State {
    Starting,
    Ready,
    Retrying,
}

pub(crate) struct Tunnel {
    pub(crate) remote: u16,
    pub(crate) local: u16,
    pub(crate) state: State,
    process: Option<u64>,
    attempts: u32,
    job: Option<u64>,
}

impl Tunnel {
    #[cfg(test)]
    pub(crate) fn at(remote: u16, local: u16, state: State) -> Self {
        Self { remote, local, state, process: None, attempts: 0, job: None }
    }
}

struct Pending {
    remote: u16,
    url: String,
    workspace: Option<u64>,
}

pub(crate) struct Tunnels {
    list: Vec<Tunnel>,
    pending: Vec<Pending>,
    events: UnboundedSender<Event>,
}

impl Tunnels {
    pub(crate) fn new(cx: &mut Context<Browser>) -> Self {
        let (events, mut rx) = unbounded::<Event>();
        cx.spawn(async move |this, cx| {
            while let Some(event) = rx.next().await
                && this.update(cx, |this, cx| this.tunnel_event(event, cx)).is_ok()
            {}
        })
        .detach();
        Self { list: Vec::new(), pending: Vec::new(), events }
    }

    pub(crate) fn get(&self, remote: u16) -> Option<&Tunnel> {
        self.list.iter().find(|t| t.remote == remote)
    }

    fn entry(&mut self, remote: u16) -> &mut Tunnel {
        if let Some(at) = self.list.iter().position(|t| t.remote == remote) {
            return &mut self.list[at];
        }
        let job = None;
        self.list.push(Tunnel { remote, local: remote, state: State::Retrying, process: None, attempts: 0, job });
        let last = self.list.len() - 1;
        &mut self.list[last]
    }

    fn by_process(&mut self, id: u64) -> Option<&mut Tunnel> {
        self.list.iter_mut().find(|t| t.process == Some(id))
    }

    pub(crate) fn pending_in(&self, workspace: Option<u64>) -> bool {
        self.pending.iter().any(|p| p.workspace == workspace)
    }

    fn take_pending(&mut self, remote: u16) -> Vec<Pending> {
        let (taken, kept) = std::mem::take(&mut self.pending).into_iter().partition(|p| p.remote == remote);
        self.pending = kept;
        taken
    }

    pub(crate) fn reverse(&self, host: &str, browser: u16) -> Option<u16> {
        ssh::reverse(host, browser, &self.events)
    }
}

impl Drop for Tunnels {
    fn drop(&mut self) {
        self.list.iter().filter_map(|t| t.process).for_each(ssh::stop);
        ssh::drop_reverse_unless(cli::host().as_deref());
    }
}

pub(crate) fn rewrite(url: &str, local: u16) -> String {
    let Some((scheme, rest)) = url.split_once("://") else { return url.to_string() };
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = rest[..end].rsplit_once('@').map_or(&rest[..end], |(_, host)| host);
    let name = authority.rsplit_once(':').map_or(authority, |(host, _)| host);
    let host = if name.ends_with(".localhost") { name } else { "localhost" };
    format!("{scheme}://{host}:{local}{}", &rest[end..])
}

pub(crate) fn pick_local(
    remote: u16,
    previous: Option<u16>,
    free: impl Fn(u16) -> bool,
    any: impl Fn() -> Option<u16>,
) -> Option<u16> {
    let preferred = previous.into_iter().chain([remote]).find(|p| free(*p));
    preferred.or_else(|| (0..PICKS).find_map(|_| any().filter(|p| free(*p))))
}

pub(crate) fn wanted(listed: &[u16], pending: &[u16], tunnels: &[(u16, u16)], urls: &[&str]) -> BTreeSet<u16> {
    let used = urls.iter().filter_map(|url| loopback_port(url));
    let used: BTreeSet<u16> = used.collect();
    let kept = tunnels.iter().filter(|(_, local)| used.contains(local)).map(|(remote, _)| *remote);
    listed.iter().chain(pending).copied().chain(kept).collect()
}

fn failure(remote: u16, why: &str, was_ready: bool) -> String {
    let host = cli::host().unwrap_or_default();
    let what = if was_ready { "lost its tunnel" } else { "could not be tunnelled here" };
    format!("Port {remote} on {host} {what}: {why}. Trying again.")
}

fn end_job(job: Option<u64>, cx: &mut Context<Browser>) {
    if let Some(job) = job {
        Store::get(cx).update(cx, |s, cx| s.end_job(job, cx));
    }
}

impl Browser {
    fn tunnel_event(&mut self, event: Event, cx: &mut Context<Self>) {
        match event {
            Event::Ready(id) => self.tunnel_ready(id, cx),
            Event::Ended(id, why) => self.tunnel_ended(id, &why, cx),
            Event::ReverseReady => {}
            Event::ReverseLost(why) => {
                say(format!("Agents on {} cannot reach this browser: {why}", cli::host().unwrap_or_default()), cx);
            }
        }
        cx.notify();
    }

    pub(super) fn keep_tunnels(&mut self, cx: &mut Context<Self>) {
        let wanted = if cli::host().is_some() { self.wanted_ports() } else { BTreeSet::new() };
        let gone: Vec<u16> = self.dev.tunnels.list.iter().map(|t| t.remote).filter(|r| !wanted.contains(r)).collect();
        for remote in gone {
            self.stop_tunnel(remote, cx);
        }
        for remote in wanted {
            if self.dev.tunnels.get(remote).is_none() {
                self.start_tunnel(remote, cx);
            }
        }
    }

    fn wanted_ports(&self) -> BTreeSet<u16> {
        let listed: Vec<u16> = self.dev.ports.iter().flatten().map(|p| p.number).collect();
        let pending: Vec<u16> = self.dev.tunnels.pending.iter().map(|p| p.remote).collect();
        let tunnels: Vec<(u16, u16)> = self.dev.tunnels.list.iter().map(|t| (t.remote, t.local)).collect();
        let tabs = self.tabs.list.iter().map(|t| t.url.as_str());
        let urls: Vec<&str> = tabs.chain(self.restore.iter().map(|(url, _)| url.as_str())).collect();
        wanted(&listed, &pending, &tunnels, &urls)
    }

    fn start_tunnel(&mut self, remote: u16, cx: &mut Context<Self>) {
        let Some(host) = cli::host() else { return };
        let previous = self.dev.tunnels.get(remote).map(|t| t.local);
        let taken: Vec<u16> = self.dev.tunnels.list.iter().filter(|t| t.remote != remote).map(|t| t.local).collect();
        let local = pick_local(remote, previous, |p| !taken.contains(&p) && ssh::free(p), ssh::any_free);
        let events = self.dev.tunnels.events.clone();
        let started = local
            .ok_or_else(|| anyhow!("no free port on this Mac"))
            .and_then(|local| ssh::forward(&host, (local, remote), events).map(|id| (local, id)));
        match started {
            Ok((local, id)) => self.tunnel_started((remote, local), id, &host, cx),
            Err(e) => self.tunnel_failed(remote, &format!("{e:#}"), cx),
        }
    }

    fn tunnel_started(&mut self, (remote, local): (u16, u16), process: u64, host: &str, cx: &mut Context<Self>) {
        let busy = Busy::new(format!("tunnel:{remote}"), format!("Opening a tunnel to port {remote} on {host}…"));
        let job = Some(Store::get(cx).update(cx, |s, cx| s.start_job(busy, cx)));
        let t = self.dev.tunnels.entry(remote);
        (t.local, t.state, t.process, t.job) = (local, State::Starting, Some(process), job);
    }

    fn tunnel_ready(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(tunnel) = self.dev.tunnels.by_process(id) else { return };
        (tunnel.state, tunnel.attempts) = (State::Ready, 0);
        let (remote, local, job) = (tunnel.remote, tunnel.local, tunnel.job.take());
        end_job(job, cx);
        for open in self.dev.tunnels.take_pending(remote) {
            self.open_now(rewrite(&open.url, local), open.workspace, cx);
        }
    }

    fn tunnel_ended(&mut self, id: u64, why: &str, cx: &mut Context<Self>) {
        if let Some(remote) = self.dev.tunnels.by_process(id).map(|t| t.remote) {
            self.tunnel_failed(remote, why, cx);
        }
    }

    fn tunnel_failed(&mut self, remote: u16, why: &str, cx: &mut Context<Self>) {
        let tunnel = self.dev.tunnels.entry(remote);
        let was_ready = tunnel.state == State::Ready;
        (tunnel.state, tunnel.process) = (State::Retrying, None);
        let (attempts, job) = (tunnel.attempts, tunnel.job.take());
        tunnel.attempts += 1;
        end_job(job, cx);
        let dropped = self.dev.tunnels.take_pending(remote);
        if attempts == 0 || !dropped.is_empty() {
            say(failure(remote, why, was_ready), cx);
        }
        let wait = cli::backoff(attempts);
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(wait).await;
            let _ = this.update(cx, |this, cx| this.retry_tunnel(remote, cx));
        })
        .detach();
    }

    fn retry_tunnel(&mut self, remote: u16, cx: &mut Context<Self>) {
        if self.dev.tunnels.get(remote).is_some_and(|t| t.process.is_none()) {
            self.start_tunnel(remote, cx);
            cx.notify();
        }
    }

    fn stop_tunnel(&mut self, remote: u16, cx: &mut Context<Self>) {
        let tunnels = &mut self.dev.tunnels;
        let Some(at) = tunnels.list.iter().position(|t| t.remote == remote) else { return };
        let tunnel = tunnels.list.remove(at);
        tunnels.take_pending(remote);
        if let Some(process) = tunnel.process {
            ssh::stop(process);
        }
        end_job(tunnel.job, cx);
    }

    pub(super) fn browse_through(&mut self, remote: u16, url: String, workspace: Option<u64>, cx: &mut Context<Self>) {
        if self.dev.tunnels.get(remote).is_none_or(|t| t.process.is_none()) {
            self.start_tunnel(remote, cx);
        }
        match self.dev.tunnels.get(remote) {
            Some(t) if t.state == State::Ready => self.open_now(rewrite(&url, t.local), workspace, cx),
            Some(_) => self.dev.tunnels.pending.push(Pending { remote, url, workspace }),
            None => {}
        }
        cx.notify();
    }

    pub(super) fn tunnel_to(&mut self, remote: u16, cx: &mut Context<Self>) -> Option<u16> {
        if self.dev.tunnels.get(remote).is_none() {
            self.start_tunnel(remote, cx);
        }
        self.dev.tunnels.get(remote).filter(|t| t.state == State::Ready).map(|t| t.local)
    }

    pub(super) fn tunnelled(&self, url: String) -> String {
        let local = loopback_port(&url).and_then(|remote| self.dev.tunnels.get(remote)).map(|t| t.local);
        match local.filter(|_| cli::host().is_some()) {
            Some(local) => rewrite(&url, local),
            None => url,
        }
    }
}

#[cfg(test)]
mod tests;
