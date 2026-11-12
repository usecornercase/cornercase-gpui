use std::time::Duration;

use futures::StreamExt as _;
use futures::channel::mpsc::unbounded;
use gpui::Context;

use super::{Busy, Store, ToastGo, ToastKind};
use crate::cli::{self, Ended};
use crate::model::tunnels::{Release, TunnelSeen, Way, held_line, way};

mod held;
mod memory;
mod rows;

pub(crate) use held::{Asked, Held};
use rows::Mark;
pub(crate) use rows::{Control, TunnelRow, Whose, asks_note, heading, none_note};

use held::{Fate, Hold, Settled, bring_back, fate, settle_now, up_toast, window_failure};

const SETTLE_AFTER: Duration = Duration::from_secs(3);

#[derive(Default)]
pub(crate) struct Tunnels {
    holds: Vec<Hold>,
    link: Option<String>,
    settling: Vec<(u16, String)>,
}

enum Heard {
    Line(String),
    Ended(Ended),
}

fn later(wait: Duration, then: impl FnOnce(&mut Store, &mut Context<Store>) + 'static, cx: &mut Context<Store>) {
    cx.spawn(async move |this, cx| {
        cx.background_executor().timer(wait).await;
        let _ = this.update(cx, then);
    })
    .detach();
}

fn job_key(port: u16) -> String {
    format!("tunnel:{port}")
}

fn closing_key(port: u16) -> String {
    format!("tunnel-close:{port}")
}

pub(crate) fn remote_url(port: u16) -> String {
    format!("http://localhost:{port}/")
}

impl Store {
    pub(crate) fn tunnel_way(&self) -> Way {
        let server = self.session.as_ref().map_or(Release::Unread, |s| Release::Read(s.release()));
        way(cli::host().as_deref(), self.heard_release(), server)
    }

    fn live_hold(&self, port: u16) -> Option<&Hold> {
        let ours = || self.tunnels.holds.iter().filter(move |hold| hold.port == port);
        ours().find(|hold| hold.state != Held::Ending).or_else(|| ours().next())
    }

    pub(crate) fn held_tunnel(&self, port: u16) -> Option<&Held> {
        self.live_hold(port).map(|hold| &hold.state)
    }

    pub(crate) fn tunnel_local(&self, port: u16) -> Option<u16> {
        self.live_hold(port).and_then(Hold::local)
    }

    pub(crate) fn tunnel_count(&self) -> usize {
        rows::count(&self.tunnels.holds)
    }

    fn listed_tunnels(&self) -> &[TunnelSeen] {
        self.session.as_ref().map_or(&[], |session| session.tunnels.as_slice())
    }

    pub(crate) fn tunnel_rows(&self) -> Vec<TunnelRow> {
        rows::menu_rows(self.listed_tunnels(), &self.tunnels.holds)
    }

    pub(crate) fn tunnel_control(&self, port: u16) -> Control {
        rows::row_control(&self.tunnel_way(), self.held_tunnel(port))
    }

    pub(crate) fn tunnel_mark(&self, port: u16) -> Option<Mark> {
        let there = rows::elsewhere(port, self.listed_tunnels(), &self.tunnels.holds);
        rows::mark(&self.tunnel_way(), self.held_tunnel(port), there)
    }

    pub(crate) fn hold_tunnel(&mut self, port: u16, asked: Asked, cx: &mut Context<Self>) {
        if self.tunnel_way() != Way::Cornercase
            || self.tunnels.holds.iter().any(|h| h.port == port && h.state != Held::Ending)
        {
            return;
        }
        self.start_hold(port, asked, None, cx);
    }

    fn start_hold(&mut self, port: u16, asked: Asked, attempts: Option<u32>, cx: &mut Context<Self>) {
        let (Some(host), Some(boot)) = (cli::host(), self.session.as_ref().map(|s| s.boot.clone())) else { return };
        let (lines, mut rx) = unbounded::<Heard>();
        let ends = lines.clone();
        let started = cli::hold(
            &cli::target(),
            port,
            move |line| {
                let _ = lines.unbounded_send(Heard::Line(line));
            },
            move |ended| {
                let _ = ends.unbounded_send(Heard::Ended(ended));
            },
        );
        let process = match started {
            Ok((process, None)) => process,
            Ok((process, Some(why))) => {
                cli::end_hold(process);
                return self.toast(window_failure(port, &why), ToastKind::Error, cx);
            }
            Err(e) => return self.toast(window_failure(port, &cli::reason(&e)), ToastKind::Error, cx),
        };
        let mut hold = Hold::opening(port, process, asked, &boot);
        hold.job =
            Some(self.start_job(Busy::new(job_key(port), format!("Opening a tunnel to port {port} on {host}…")), cx));
        if let Some(attempts) = attempts {
            hold.attempts = attempts;
            hold.was_up = true;
        }
        self.tunnels.holds.push(hold);
        if asked != Asked::Back && asked != Asked::Retry {
            self.remember_tunnel(&host, port, true, cx);
        }
        cx.spawn(async move |this, cx| {
            while let Some(heard) = rx.next().await
                && this.update(cx, |this, cx| this.hold_heard(process, heard, cx)).is_ok()
            {}
        })
        .detach();
        cx.notify();
    }

    fn remember_tunnel(&mut self, host: &str, port: u16, keep: bool, cx: &mut Context<Self>) {
        let saved =
            memory::save(
                |text| {
                    if keep { memory::remember(text, host, port) } else { memory::forget(text, host, port) }
                },
            );
        if let Err(e) = saved {
            self.toast(format!("Could not save the tunnels to bring back: {e:#}"), ToastKind::Error, cx);
        }
    }

    fn hold_heard(&mut self, process: u64, heard: Heard, cx: &mut Context<Self>) {
        match heard {
            Heard::Line(line) => {
                if let Some(held) = held_line(&line) {
                    self.hold_up(process, held.local, cx);
                }
            }
            Heard::Ended(ended) => self.hold_ended(process, &ended, cx),
        }
    }

    fn hold_up(&mut self, process: u64, local: u16, cx: &mut Context<Self>) {
        let Some(hold) = self.tunnels.holds.iter_mut().find(|h| h.process == process) else { return };
        if hold.state == Held::Ending {
            return;
        }
        hold.state = Held::Up { local };
        (hold.was_up, hold.attempts) = (true, 0);
        hold.last_local = Some(local);
        let (port, asked, job) = (hold.port, hold.asked, hold.job.take());
        if let Some(job) = job {
            self.end_job(job, cx);
        }
        let Some(host) = cli::host() else { return };
        if let Some((text, open)) = up_toast(asked, port, local, &host) {
            let action = open.then(|| ("Open".to_string(), ToastGo::Browse(remote_url(port))));
            self.toast_with(text, ToastKind::Info, action, cx);
        }
        cx.notify();
    }

    fn hold_ended(&mut self, process: u64, ended: &Ended, cx: &mut Context<Self>) {
        let Some(at) = self.tunnels.holds.iter().position(|h| h.process == process) else { return };
        let hold = self.tunnels.holds[at].clone();
        if let Some(job) = hold.job {
            self.end_job(job, cx);
        }
        match fate(&hold, ended.code, &ended.said) {
            Fate::Gone => drop(self.tunnels.holds.remove(at)),
            Fate::Failed(why) => {
                self.tunnels.holds.remove(at);
                self.toast(why, ToastKind::Error, cx);
            }
            Fate::Retry(why) => self.fall(at, why, cx),
            Fate::Settle => {
                self.tunnels.holds.remove(at);
                self.tunnels.settling.push((hold.port, hold.boot));
                later(SETTLE_AFTER, Self::settle_later, cx);
            }
        }
        cx.notify();
    }

    fn fall(&mut self, at: usize, why: String, cx: &mut Context<Self>) {
        let hold = &mut self.tunnels.holds[at];
        hold.state = Held::Down { why };
        (hold.job, hold.process) = (None, 0);
        hold.attempts += 1;
        let (port, attempts) = (hold.port, hold.attempts);
        later(cli::backoff(attempts - 1), move |this, cx| this.retry_hold(port, attempts, cx), cx);
    }

    fn retry_hold(&mut self, port: u16, attempts: u32, cx: &mut Context<Self>) {
        let waiting = |h: &Hold| h.port == port && h.attempts == attempts && matches!(h.state, Held::Down { .. });
        let Some(at) = self.tunnels.holds.iter().position(waiting) else { return };
        if self.tunnel_way() != Way::Cornercase || self.tunnels.link.is_none() {
            return;
        }
        self.tunnels.holds.remove(at);
        self.start_hold(port, Asked::Retry, Some(attempts), cx);
    }

    pub(crate) fn close_tunnel(&mut self, port: u16, forget: bool, cx: &mut Context<Self>) {
        let Some(host) = cli::host() else { return };
        let ours = self.tunnels.holds.iter().position(|h| h.port == port && h.state != Held::Ending);
        let quick = ours.is_some_and(|at| !matches!(self.tunnels.holds[at].state, Held::Up { .. }));
        if let Some(at) = ours {
            if quick {
                self.forget_in_memory(&host, port, cx);
                return self.drop_hold(at, cx);
            }
            self.tunnels.holds[at].state = Held::Ending;
        }
        let verb = if forget { "forget" } else { "close" };
        let busy = Busy::new(closing_key(port), format!("Closing the tunnel of port {port} on {host}…"));
        let args = vec!["tunnel".to_string(), verb.to_string(), port.to_string()];
        let work = move || cli::run(&args);
        self.run_then(busy, work, move |this, result, cx| this.closed((port, forget), result, cx), cx);
        cx.notify();
    }

    fn forget_in_memory(&mut self, host: &str, port: u16, cx: &mut Context<Self>) {
        self.remember_tunnel(host, port, false, cx);
    }

    fn drop_hold(&mut self, at: usize, cx: &mut Context<Self>) {
        let hold = self.tunnels.holds.remove(at);
        cli::end_hold(hold.process);
        if let Some(job) = hold.job {
            self.end_job(job, cx);
        }
        cx.notify();
    }

    fn closed(&mut self, (port, forget): (u16, bool), result: anyhow::Result<String>, cx: &mut Context<Self>) {
        let host = cli::host().unwrap_or_default();
        let verb = if forget { "forget" } else { "close" };
        match result {
            Ok(_) => {
                self.forget_in_memory(&host, port, cx);
                let past = if forget { "Forgot" } else { "Closed" };
                self.toast(format!("{past} the tunnel of port {port} on {host}"), ToastKind::Info, cx);
            }
            Err(e) => {
                let text = format!("Could not {verb} the tunnel of port {port} on {host}: {}", cli::reason(&e));
                self.toast(text, ToastKind::Error, cx);
                self.restore_ending(port);
            }
        }
        cx.notify();
    }

    fn restore_ending(&mut self, port: u16) {
        for hold in self.tunnels.holds.iter_mut().filter(|h| h.port == port && h.state == Held::Ending) {
            if let (Some(local), true) = (hold.last_local, hold.process != 0) {
                hold.state = Held::Up { local };
            }
        }
    }

    pub(crate) fn follow_tunnels(&mut self, cx: &mut Context<Self>) {
        if self.tunnel_way() != Way::Cornercase {
            if !self.tunnels.holds.is_empty() {
                self.end_tunnels_here(cx);
            }
            self.tunnels.link = None;
            return;
        }
        let (Some(host), Some(boot)) = (cli::host(), self.session.as_ref().map(|s| s.boot.clone())) else { return };
        let relink = self.tunnels.link.as_ref() != Some(&boot);
        self.tunnels.link = Some(boot);
        self.settle_tunnels(&host, cx);
        if relink {
            self.link_tunnels(&host, cx);
        }
    }

    fn settle_later(&mut self, cx: &mut Context<Self>) {
        if let Some(host) = cli::host() {
            self.settle_tunnels(&host, cx);
        }
    }

    fn settle_tunnels(&mut self, host: &str, cx: &mut Context<Self>) {
        let Some(now) = self.session.as_ref().map(|s| s.boot.clone()) else { return };
        let linked = self.tunnels.link.is_some();
        for (port, started) in std::mem::take(&mut self.tunnels.settling) {
            match settle_now(linked, &started, &now) {
                None => self.tunnels.settling.push((port, started)),
                Some(Settled::Closed) => self.forget_in_memory(host, port, cx),
                Some(Settled::Back) => self.hold_tunnel(port, Asked::Back, cx),
            }
        }
    }

    fn link_tunnels(&mut self, host: &str, cx: &mut Context<Self>) {
        let held: Vec<u16> = self.tunnels.holds.iter().map(|h| h.port).collect();
        for port in bring_back(&memory::remembered(host), &held) {
            self.hold_tunnel(port, Asked::Back, cx);
        }
        let down: Vec<(u16, u32)> = self
            .tunnels
            .holds
            .iter()
            .filter(|h| matches!(h.state, Held::Down { .. }))
            .map(|h| (h.port, h.attempts))
            .collect();
        for (port, attempts) in down {
            self.retry_hold(port, attempts, cx);
        }
    }

    pub(crate) fn drop_down_tunnel(&mut self, port: u16) {
        self.tunnels.holds.retain(|hold| !(hold.port == port && matches!(hold.state, Held::Down { .. })));
    }

    pub(crate) fn tunnels_unlinked(&mut self) {
        self.tunnels.link = None;
    }

    fn end_tunnels_here(&mut self, cx: &mut Context<Self>) {
        cli::end_holds();
        for hold in std::mem::take(&mut self.tunnels.holds) {
            if let Some(job) = hold.job {
                self.end_job(job, cx);
            }
        }
        cx.notify();
    }
}
