use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use futures::StreamExt as _;
use futures::channel::mpsc::unbounded;
use gpui::Context;

use super::browse::{listed_gate, toast_go};
use super::{Busy, Store, ToastKind};
use crate::cli::{self, Ended, Holding, Version};
use crate::model::tunnels::{
    Gate, Release, Toward, TunnelSeen, Way, gate_note, held_line, local_kept, local_to_ask, machine_gate, way,
};

mod closing;
mod held;
mod links;
mod machines;
mod memory;
mod orders;
mod reverse;
mod rows;
mod taker;

pub(crate) use held::{Asked, Held};
pub(crate) use links::MachineLink;
pub(crate) use reverse::{ReverseHold, ReverseState, address_there, copied_words};
use rows::Mark;
pub(crate) use rows::{Control, Tail, TunnelRow, Whose, heading, none_note, others_note, section_tail};

use held::{
    Again, Fate, Hold, Reached, Settled, again_of, bring_back, came_back, fate, forgets_failed, live, local_before,
    ports_kept, remembers, settle_now, up_toast, window_failure,
};

const SETTLE_AFTER: Duration = Duration::from_secs(3);

#[derive(Default)]
pub(crate) struct Tunnels {
    holds: Vec<Hold>,
    reverses: BTreeMap<Toward, ReverseHold>,
    link: Option<String>,
    machines: BTreeMap<String, String>,
    settling: Vec<Settling>,
    takers: BTreeMap<Toward, taker::Taker>,
    next_taker: u64,
}

struct Settling {
    toward: Toward,
    port: u16,
    boot: String,
    destination: String,
    since: Instant,
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

fn job_key(toward: &Toward, port: u16) -> String {
    format!("tunnel:{}:{port}", toward.key())
}

pub(crate) fn remote_url(port: u16) -> String {
    format!("http://localhost:{port}/")
}

type HeardBy = fn(&mut Store, u64, Heard, &mut Context<Store>);

fn listen_to_hold(
    process: u64,
    mut rx: futures::channel::mpsc::UnboundedReceiver<Heard>,
    on: HeardBy,
    cx: &mut Context<Store>,
) {
    cx.spawn(async move |this, cx| {
        while let Some(heard) = rx.next().await
            && this.update(cx, |this, cx| on(this, process, heard, cx)).is_ok()
        {}
    })
    .detach();
}

impl Store {
    pub(crate) fn tunnel_way(&self) -> Way {
        let server = self.session.as_ref().map_or(Release::Unread, |s| Release::Read(s.release()));
        way(cli::host().as_deref(), self.heard_release(), server)
    }

    pub(crate) fn machine_tunnel_gate(&self, name: &str) -> Gate {
        let server = crate::model::machines::run_on::server_release(Some(name), self.session.as_ref(), &self.machines);
        listed_gate(self.machines_known(), machine_gate(self.heard_release(), server))
    }

    fn reached(&self, toward: &Toward) -> Result<Reached, String> {
        let session = self.session.as_ref().ok_or_else(|| "not connected".to_string())?;
        let toward = toward.clone();
        match &toward {
            Toward::Server => {
                let host = cli::host().ok_or_else(|| "not on another server".to_string())?;
                let boot = session.boot.clone();
                Ok(Reached { toward, target: cli::target(), boot, label: host.clone(), destination: host })
            }
            Toward::Machine(name) => {
                let seen = session.machine(name).ok_or_else(|| format!("{name} is not shown here"))?;
                let target = links::tunnel_target(self.reach_seen(seen))?;
                let listed = self.machines.iter().find(|machine| machine.name == *name);
                let destination = listed.map(|machine| machine.destination.clone()).unwrap_or_default();
                Ok(Reached { target, boot: seen.boot.clone(), label: name.clone(), destination, toward })
            }
        }
    }

    pub(crate) fn can_tunnel(&self, toward: &Toward) -> bool {
        match toward {
            Toward::Server => self.tunnel_way() == Way::Cornercase,
            Toward::Machine(name) => self.machine_tunnel_gate(name) == Gate::Open && self.reached(toward).is_ok(),
        }
    }

    pub(crate) fn tunnel_refusal(&self, toward: &Toward) -> String {
        let Toward::Machine(name) = toward else { return String::new() };
        match self.machine_tunnel_gate(name) {
            Gate::Older(older) => gate_note(&older, name),
            Gate::Waiting => format!("this {}'s cornercase release is not read yet", crate::desktop::here()),
            Gate::Open => self.reached(toward).err().unwrap_or_default(),
        }
    }

    pub(crate) fn held_tunnel(&self, toward: &Toward, port: u16) -> Option<&Held> {
        live(&self.tunnels.holds, toward, port).map(|hold| &hold.state)
    }

    pub(crate) fn tunnel_local(&self, toward: &Toward, port: u16) -> Option<u16> {
        live(&self.tunnels.holds, toward, port).and_then(Hold::local)
    }

    pub(crate) fn tunnel_count(&self) -> usize {
        rows::count(&self.tunnels.holds, self.tunnels.reverses.len())
    }

    fn release_gate(&self, toward: &Toward, make: fn(Release, Option<Version>) -> Gate) -> Gate {
        match toward {
            Toward::Server => {
                self.session.as_ref().map_or(Gate::Waiting, |session| make(self.heard_release(), session.release()))
            }
            Toward::Machine(name) => {
                let server =
                    crate::model::machines::run_on::server_release(Some(name), self.session.as_ref(), &self.machines);
                make(self.heard_release(), server)
            }
        }
    }

    fn listed_tunnels(&self) -> &[TunnelSeen] {
        self.session.as_ref().map_or(&[], |session| session.tunnels.as_slice())
    }

    pub(crate) fn tunnel_rows(&self) -> Vec<TunnelRow> {
        rows::menu_rows(self.listed_tunnels(), &self.tunnels.holds, self.tunnels.reverses.get(&Toward::Server))
    }

    pub(crate) fn machine_tunnel_sections(&self) -> Vec<(String, Vec<TunnelRow>)> {
        rows::machine_sections(&self.tunnels.holds, &self.tunnels.reverses)
    }

    pub(crate) fn tunnel_control(&self, toward: &Toward, port: u16) -> Control {
        rows::row_control(self.can_tunnel(toward), self.held_tunnel(toward, port))
    }

    pub(crate) fn tunnel_mark(&self, toward: &Toward, port: u16) -> Option<Mark> {
        let there = *toward == Toward::Server && rows::elsewhere(port, self.listed_tunnels(), &self.tunnels.holds);
        rows::mark(self.can_tunnel(toward), self.held_tunnel(toward, port), there)
    }

    pub(crate) fn hold_tunnel(&mut self, toward: &Toward, port: u16, asked: Asked, cx: &mut Context<Self>) {
        if live(&self.tunnels.holds, toward, port).is_some() {
            return;
        }
        if !self.can_tunnel(toward) {
            if asked == Asked::Click && *toward != Toward::Server {
                let why = self.tunnel_refusal(toward);
                self.not_sent(&why, cx);
            }
            return;
        }
        let _ = self.start_hold(toward, port, (asked, None), cx);
    }

    fn start_hold(
        &mut self,
        toward: &Toward,
        port: u16,
        (asked, again): (Asked, Option<Again>),
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let reached = self.reached(toward)?;
        let remembered = memory::remembered_local(&reached.destination, port);
        let before = local_before(again, remembered);
        let local = local_to_ask(self.heard_release(), port, before);
        let spawned = Self::spawn_hold(&reached, Holding::Forward { port, local });
        let (process, rx) = spawned.inspect_err(|why| {
            self.toast(window_failure(&reached.toward, port, why), ToastKind::Error, cx);
        })?;
        let mut hold = Hold::opening(&reached, port, process, asked);
        hold.had_local = local_kept(self.heard_release(), before);
        let busy = Busy::new(job_key(toward, port), format!("Opening a tunnel to port {port} on {}…", reached.label));
        hold.job = Some(self.start_job(busy, cx));
        if let Some(Again { attempts, .. }) = again {
            hold.attempts = attempts;
            hold.was_up = true;
        }
        hold.added = remembers(asked) && !memory::remembered(&reached.destination).contains(&port);
        self.tunnels.holds.push(hold);
        if remembers(asked) {
            self.remember_tunnel(&reached.destination, port, true, cx);
        }
        listen_to_hold(process, rx, Self::hold_heard, cx);
        cx.notify();
        Ok(())
    }

    fn spawn_hold(
        reached: &Reached,
        holding: Holding,
    ) -> Result<(u64, futures::channel::mpsc::UnboundedReceiver<Heard>), String> {
        let (lines, rx) = unbounded::<Heard>();
        let ends = lines.clone();
        let started = cli::hold(
            &reached.target,
            holding,
            move |line| {
                let _ = lines.unbounded_send(Heard::Line(line));
            },
            move |ended| {
                let _ = ends.unbounded_send(Heard::Ended(ended));
            },
        );
        let why = match started {
            Ok((process, None)) => return Ok((process, rx)),
            Ok((process, Some(why))) => {
                cli::end_hold(process);
                why
            }
            Err(e) => cli::reason(&e),
        };
        Err(why)
    }

    fn remember_tunnel(&mut self, destination: &str, port: u16, keep: bool, cx: &mut Context<Self>) {
        let saved = memory::save(|text| {
            if keep { memory::remember(text, destination, port) } else { memory::forget(text, destination, port) }
        });
        if let Err(e) = saved {
            self.toast(format!("Could not save the tunnels to bring back: {e:#}"), ToastKind::Error, cx);
        }
    }

    fn hold_heard(&mut self, process: u64, heard: Heard, cx: &mut Context<Self>) {
        match heard {
            Heard::Line(line) => {
                if let Some(held) = held_line(&line).filter(|held| !held.reverse) {
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
        let (toward, label) = (hold.toward.clone(), hold.label().to_string());
        let (destination, wanted) = (hold.destination.clone(), hold.had_local);
        if let Some(job) = job {
            self.end_job(job, cx);
        }
        self.note_local(&destination, port, local, cx);
        let said = up_toast(asked, port, local, &label)
            .or_else(|| came_back(asked, (port, local), wanted, &label).map(|text| (text, true)));
        if let Some((text, open)) = said {
            let go = toast_go(&toward, port);
            let action = open.then(|| ("Open".to_string(), go));
            self.toast_with(text, ToastKind::Info, action, cx);
        }
        cx.notify();
    }

    fn note_local(&mut self, destination: &str, port: u16, local: u16, cx: &mut Context<Self>) {
        let saved = memory::save_noted(|text| memory::note_local(text, destination, port, local));
        if let Err(e) = saved {
            self.toast(format!("Could not save the tunnels to bring back: {e:#}"), ToastKind::Error, cx);
        }
    }

    fn hold_ended(&mut self, process: u64, ended: &Ended, cx: &mut Context<Self>) {
        let Some(at) = self.tunnels.holds.iter().position(|h| h.process == process) else { return };
        let hold = self.tunnels.holds[at].clone();
        if let Some(job) = hold.job {
            self.end_job(job, cx);
        }
        let fate = fate(&hold, ended.code, &ended.said);
        if forgets_failed(&fate, hold.added) {
            self.remember_tunnel(&hold.destination, hold.port, false, cx);
        }
        let failing = matches!(fate, Fate::Failed(_) | Fate::Retry(_));
        match fate {
            Fate::Gone => drop(self.tunnels.holds.remove(at)),
            Fate::Failed(why) => {
                self.tunnels.holds.remove(at);
                self.toast(why, ToastKind::Error, cx);
            }
            Fate::Retry(why) => self.fall(at, why, cx),
            Fate::Settle => {
                self.tunnels.holds.remove(at);
                let (toward, port, boot) = (hold.toward.clone(), hold.port, hold.boot.clone());
                let destination = hold.destination.clone();
                let since = Instant::now();
                self.tunnels.settling.push(Settling { toward, port, boot, destination, since });
                later(SETTLE_AFTER, Self::settle_later, cx);
            }
        }
        if let (true, Toward::Machine(name)) = (failing, &hold.toward) {
            self.there_failed(name, cx);
        }
        cx.notify();
    }

    fn fall(&mut self, at: usize, why: String, cx: &mut Context<Self>) {
        let hold = &mut self.tunnels.holds[at];
        hold.state = Held::Down { why };
        (hold.job, hold.process) = (None, 0);
        hold.attempts += 1;
        let (toward, port, attempts) = (hold.toward.clone(), hold.port, hold.attempts);
        later(cli::backoff(attempts - 1), move |this, cx| this.retry_hold(&toward, port, attempts, cx), cx);
    }

    fn linked(&self, toward: &Toward) -> bool {
        match toward {
            Toward::Server => self.tunnels.link.is_some(),
            Toward::Machine(name) => self.tunnels.machines.contains_key(name),
        }
    }

    fn retry_hold(&mut self, toward: &Toward, port: u16, attempts: u32, cx: &mut Context<Self>) {
        let waiting = |h: &Hold| {
            h.port == port && h.toward == *toward && h.attempts == attempts && matches!(h.state, Held::Down { .. })
        };
        let Some(at) = self.tunnels.holds.iter().position(waiting) else { return };
        if !self.can_tunnel(toward) || !self.linked(toward) {
            return;
        }
        let again = again_of(&self.tunnels.holds[at]);
        self.tunnels.holds.remove(at);
        let _ = self.start_hold(toward, port, (Asked::Retry, Some(again)), cx);
    }

    pub(crate) fn follow_tunnels(&mut self, cx: &mut Context<Self>) {
        self.follow_own_tunnels(cx);
        self.follow_machine_tunnels(cx);
        self.follow_server_taker(cx);
    }

    fn follow_own_tunnels(&mut self, cx: &mut Context<Self>) {
        let way = self.tunnel_way();
        let reverse_ends = way != Way::Cornercase || self.reverse_gate_of(&Toward::Server).known_older();
        self.follow_own_reverse(reverse_ends, cx);
        if way != Way::Cornercase {
            self.end_own_holds(&way, cx);
            self.tunnels.link = None;
            return;
        }
        let Some(boot) = self.session.as_ref().map(|s| s.boot.clone()) else { return };
        let relink = self.tunnels.link.as_ref() != Some(&boot);
        self.tunnels.link = Some(boot);
        self.settle_tunnels(cx);
        if let (true, Some(host)) = (relink, cli::host()) {
            self.link_tunnels(&Toward::Server, &host, cx);
        }
    }

    fn settle_later(&mut self, cx: &mut Context<Self>) {
        self.settle_tunnels(cx);
    }

    fn boot_of(&self, toward: &Toward) -> Option<String> {
        let session = self.session.as_ref()?;
        match toward {
            Toward::Server => Some(session.boot.clone()),
            Toward::Machine(name) => session.machine(name).map(|seen| seen.boot.clone()),
        }
    }

    pub(super) fn settle_tunnels(&mut self, cx: &mut Context<Self>) {
        for Settling { toward, port, boot: started, destination, since } in std::mem::take(&mut self.tunnels.settling) {
            let now = self.boot_of(&toward);
            let live = live(&self.tunnels.holds, &toward, port).is_some_and(|hold| hold.state != Held::Ending);
            let waited = since.elapsed() >= SETTLE_AFTER;
            match settle_now((self.linked(&toward), live, waited), &started, now.as_deref()) {
                None => self.tunnels.settling.push(Settling { toward, port, boot: started, destination, since }),
                Some(Settled::Superseded) => {}
                Some(Settled::Closed) => self.remember_tunnel(&destination, port, false, cx),
                Some(Settled::Back) => self.hold_tunnel(&toward, port, Asked::Back, cx),
            }
        }
    }

    fn link_tunnels(&mut self, toward: &Toward, destination: &str, cx: &mut Context<Self>) {
        let settling = self.tunnels.settling.iter().filter(|s| s.toward == *toward).map(|s| s.port);
        let held = ports_kept(&self.tunnels.holds, toward, settling);
        for port in bring_back(&memory::remembered(destination), &held) {
            self.hold_tunnel(toward, port, Asked::Back, cx);
        }
        let down: Vec<(u16, u32)> = self
            .tunnels
            .holds
            .iter()
            .filter(|h| h.toward == *toward && matches!(h.state, Held::Down { .. }))
            .map(|h| (h.port, h.attempts))
            .collect();
        for (port, attempts) in down {
            self.retry_hold(toward, port, attempts, cx);
        }
        self.orders_linked(toward, cx);
    }

    pub(crate) fn try_tunnel_again(&mut self, toward: &Toward, port: u16, cx: &mut Context<Self>) {
        if !self.can_tunnel(toward) {
            if *toward != Toward::Server {
                let why = self.tunnel_refusal(toward);
                self.not_sent(&why, cx);
            }
            return;
        }
        self.drop_down_tunnel(toward, port);
        self.hold_tunnel(toward, port, Asked::Click, cx);
    }

    pub(crate) fn drop_down_tunnel(&mut self, toward: &Toward, port: u16) {
        self.tunnels
            .holds
            .retain(|hold| !(hold.port == port && hold.toward == *toward && matches!(hold.state, Held::Down { .. })));
    }

    pub(crate) fn tunnels_unlinked(&mut self) {
        self.tunnels.link = None;
        self.tunnels.machines.clear();
    }

    fn end_own_holds(&mut self, way: &Way, cx: &mut Context<Self>) {
        let (ended, kept) = links::split_ended(std::mem::take(&mut self.tunnels.holds), way);
        self.tunnels.holds = kept;
        self.end_holds_now(ended, cx);
    }

    fn end_holds_now(&mut self, ended: Vec<Hold>, cx: &mut Context<Self>) {
        if ended.is_empty() {
            return;
        }
        for hold in ended {
            if hold.process != 0 {
                cli::end_hold(hold.process);
            }
            if let Some(job) = hold.job {
                self.end_job(job, cx);
            }
        }
        cx.notify();
    }
}
