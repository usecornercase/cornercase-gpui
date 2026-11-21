use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use gpui::Context;
use serde_json::Value;

use super::super::{Reach, Store, ToastKind, args, follow_on};
use crate::cli::{self, Stream, Target};
use crate::model::MachineSeen;
use crate::model::bots::{self, Bot, BotKey, Bots, Chat, Home, Told, label};
use crate::model::place::Moved;

#[derive(Default)]
pub(in crate::store) struct BotsThere {
    links: BTreeMap<String, Link>,
    revision: u64,
    starts: u64,
}

struct Link {
    bots: Bots,
    target: Option<Target>,
    list: Option<Stream>,
    events: Option<Stream>,
    start: u64,
    heard: u64,
    failures: u32,
    next: Option<Instant>,
}

impl Link {
    fn new(machine: &str, active: bool) -> Self {
        Self {
            bots: Bots::at(Home::Machine(machine.to_string()), active),
            target: None,
            list: None,
            events: None,
            start: 0,
            heard: 0,
            failures: 0,
            next: None,
        }
    }

    fn failed(&self) -> bool {
        self.failures > 0 && self.list.is_none()
    }

    fn due(&self, now: Instant) -> bool {
        self.next.is_none_or(|next| now >= next)
    }

    fn stop(&mut self) -> bool {
        let running = self.target.is_some() || self.list.is_some() || self.events.is_some();
        (self.target, self.list, self.events, self.start, self.heard) = (None, None, None, 0, 0);
        running
    }
}

impl BotsThere {
    pub(in crate::store) fn revision(&self) -> u64 {
        self.revision + self.links.values().map(|link| link.bots.revision()).sum::<u64>()
    }

    pub(in crate::store) fn set_active(&mut self, active: bool) {
        for link in self.links.values_mut() {
            link.bots.active = active;
        }
    }

    pub(in crate::store) fn forget_chats(&mut self) {
        for link in self.links.values_mut() {
            link.bots.chat = None;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Follow {
    Start(Target),
    Keep,
    Wait,
    Stop,
    Drop,
}

pub(crate) fn follow_of(reach: Option<&Reach>, running: Option<&Target>, due: bool) -> Follow {
    match reach {
        None => Follow::Drop,
        Some(Reach::Up(target)) if running == Some(target) => Follow::Keep,
        Some(Reach::Up(target)) if due => Follow::Start(target.clone()),
        Some(Reach::Up(_)) => Follow::Wait,
        Some(_) => Follow::Stop,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Events {
    Start,
    Stop,
    Leave,
}

pub(crate) fn events_wanted(listed: bool, following: bool) -> Events {
    match (listed, following) {
        (true, false) => Events::Start,
        (false, true) => Events::Stop,
        _ => Events::Leave,
    }
}

pub(crate) fn note_of(machine: &str, reach: Option<&Reach>, failed: bool) -> Option<String> {
    match reach? {
        Reach::Up(_) => failed.then(|| format!("{machine} did not answer · reconnecting…")),
        other => other.why_not().map(str::to_string),
    }
}

fn later(wait: Duration, cx: &mut Context<Store>) {
    cx.spawn(async move |this, cx| {
        cx.background_executor().timer(wait).await;
        let _ = this.update(cx, Store::follow_machine_bots);
    })
    .detach();
}

impl Store {
    pub(crate) fn bots_at(&self, home: &Home) -> Option<&Bots> {
        match home {
            Home::Here => Some(&self.bots),
            Home::Machine(machine) => self.there.bots.links.get(machine).map(|link| &link.bots),
        }
    }

    pub(in crate::store) fn bots_at_mut(&mut self, home: &Home) -> Option<&mut Bots> {
        match home {
            Home::Here => Some(&mut self.bots),
            Home::Machine(machine) => self.there.bots.links.get_mut(machine).map(|link| &mut link.bots),
        }
    }

    pub(crate) fn bot(&self, key: &BotKey) -> Option<&Bot> {
        self.bots_at(&key.home)?.find(&key.id)
    }

    fn links_in_order(&self) -> Vec<(&String, &Link)> {
        let machines = self.session.as_ref().map_or(&[][..], |session| session.machines.as_slice());
        let at = |name: &str| machines.iter().position(|seen| seen.name == name).unwrap_or(usize::MAX);
        let mut links: Vec<(&String, &Link)> = self.there.bots.links.iter().collect();
        links.sort_by_key(|(name, _)| at(name));
        links
    }

    fn every_bots(&self) -> impl Iterator<Item = &Bots> {
        std::iter::once(&self.bots).chain(self.there.bots.links.values().map(|link| &link.bots))
    }

    pub(crate) fn any_bot_listed(&self) -> bool {
        self.every_bots().any(|bots| !bots.list.is_empty())
    }

    pub(crate) fn all_bots(&self) -> Vec<&Bot> {
        let there = self.links_in_order().into_iter().flat_map(|(_, link)| link.bots.list.iter());
        self.bots.list.iter().chain(there).collect()
    }

    pub(crate) fn bots_on_machines(&self) -> Vec<(&MachineSeen, &[Bot], Option<String>)> {
        let Some(session) = self.session.as_ref() else { return Vec::new() };
        self.links_in_order()
            .into_iter()
            .filter_map(|(name, link)| {
                let seen = session.machine(name)?;
                Some((seen, link.bots.list.as_slice(), self.machine_bots_note(name)))
            })
            .collect()
    }

    pub(crate) fn shown_chat(&self) -> Option<(&Bot, &Chat)> {
        let key = self.shown_bot()?;
        let bots = self.bots_at(&key.home)?;
        let chat = bots.chat.as_ref().filter(|chat| chat.bot == key.id)?;
        Some((bots.find(&key.id)?, chat))
    }

    pub(crate) fn bot_label(&self, key: &BotKey) -> String {
        self.bot(key).map_or_else(|| label(&key.id, &key.home), Bot::label)
    }

    pub(crate) fn draft(&self, key: &BotKey) -> &str {
        self.bots_at(&key.home).map_or("", |bots| bots.drafts.text(&key.id))
    }

    pub(crate) fn drafts_revision(&self) -> u64 {
        self.every_bots().map(|bots| bots.drafts.revision()).sum()
    }

    pub(crate) fn bot_stopping(&self, key: &BotKey) -> bool {
        self.bots_at(&key.home).is_some_and(|bots| bots.stopping(&key.id))
    }

    pub(crate) fn any_bot_thinking(&self) -> bool {
        self.every_bots().any(Bots::thinking)
    }

    pub(crate) fn bots_unread(&self) -> usize {
        self.every_bots().map(Bots::unread).sum()
    }

    pub(crate) fn take_told(&mut self) -> Vec<Told> {
        std::mem::take(&mut self.bots.to_tell)
    }

    pub(crate) fn machine_bots_note(&self, machine: &str) -> Option<String> {
        let reach = self.session.as_ref()?.machine(machine).map(|seen| self.reach_seen(seen));
        let failed = self.there.bots.links.get(machine).is_some_and(Link::failed);
        note_of(machine, reach.as_ref(), failed)
    }

    pub(in crate::store) fn follow_machine_bots(&mut self, cx: &mut Context<Self>) {
        let Some(session) = self.session.as_ref() else { return };
        let mut names: Vec<String> = session.machines.iter().map(|seen| seen.name.clone()).collect();
        names.extend(self.there.bots.links.keys().filter(|name| !names.contains(name)).cloned().collect::<Vec<_>>());
        let now = Instant::now();
        for name in names {
            self.follow_bots_of(&name, now, cx);
        }
    }

    fn follow_bots_of(&mut self, machine: &str, now: Instant, cx: &mut Context<Self>) {
        let reach =
            self.session.as_ref().and_then(|session| session.machine(machine)).map(|seen| self.reach_seen(seen));
        let link = self.there.bots.links.get(machine);
        let due = link.is_none_or(|link| link.due(now));
        match follow_of(reach.as_ref(), link.and_then(|link| link.target.as_ref()), due) {
            Follow::Start(target) => self.start_bots_there(machine, &target, cx),
            Follow::Keep | Follow::Wait => {}
            Follow::Stop => self.stop_bots_there(machine, cx),
            Follow::Drop => self.drop_bots_there(machine, cx),
        }
    }

    fn start_bots_there(&mut self, machine: &str, target: &Target, cx: &mut Context<Self>) {
        self.there.bots.starts += 1;
        let (start, active) = (self.there.bots.starts, self.bots.active);
        let link = self.there.bots.links.entry(machine.to_string()).or_insert_with(|| Link::new(machine, active));
        link.stop();
        (link.start, link.target) = (start, Some(target.clone()));
        let (heard, ended) = (machine.to_string(), machine.to_string());
        let list = follow_on(
            cx,
            target,
            args(&["status", "--app", "--follow"]),
            move |this: &mut Self, line, cx| this.bots_line_there(&heard, start, &line, cx),
            move |this: &mut Self, _, cx| this.bots_ended_there(&ended, start, cx),
        );
        if let Some(link) = self.there.bots.links.get_mut(machine) {
            link.list = list;
        }
        self.there.bots.revision += 1;
        cx.notify();
    }

    fn running(&mut self, machine: &str, start: u64) -> Option<&mut Link> {
        self.there.bots.links.get_mut(machine).filter(|link| link.start == start && start > 0)
    }

    fn bots_line_there(&mut self, machine: &str, start: u64, line: &str, cx: &mut Context<Self>) {
        let Ok(value) = serde_json::from_str::<Value>(line) else { return };
        let Some(followed) = self.running(machine, start) else { return };
        let came_back = followed.failures > 0;
        (followed.failures, followed.next) = (0, None);
        if came_back {
            self.there.bots.revision += 1;
        }
        self.take_bots(&Home::Machine(machine.to_string()), &value, cx);
        self.follow_events_there(machine, cx);
        cx.notify();
    }

    fn follow_events_there(&mut self, machine: &str, cx: &mut Context<Self>) {
        let Some(link) = self.there.bots.links.get_mut(machine) else { return };
        match events_wanted(!link.bots.list.is_empty(), link.events.is_some()) {
            Events::Leave => {}
            Events::Stop => (link.events, link.heard) = (None, 0),
            Events::Start => self.follow_bot_events(machine, cx),
        }
    }

    fn follow_bot_events(&mut self, machine: &str, cx: &mut Context<Self>) {
        self.there.bots.starts += 1;
        let heard = self.there.bots.starts;
        let Some(link) = self.there.bots.links.get_mut(machine) else { return };
        let Some(target) = link.target.clone() else { return };
        let start = link.start;
        link.heard = heard;
        let (on_line, on_end) = (machine.to_string(), machine.to_string());
        let events = follow_on(
            cx,
            &target,
            args(&["events", "--json"]),
            move |this: &mut Self, line, cx| this.bot_event_there(&on_line, start, &line, cx),
            move |this: &mut Self, _, cx| this.bot_events_ended(&on_end, (start, heard), cx),
        );
        if let Some(link) = self.running(machine, start) {
            link.events = events;
        }
        self.events_started(&Home::Machine(machine.to_string()), cx);
    }

    fn bot_events_ended(&mut self, machine: &str, (start, heard): (u64, u64), cx: &mut Context<Self>) {
        if self.running(machine, start).is_some_and(|link| link.heard == heard) {
            self.bots_ended_there(machine, start, cx);
        }
    }

    fn bot_event_there(&mut self, machine: &str, start: u64, line: &str, cx: &mut Context<Self>) {
        if self.running(machine, start).is_none() {
            return;
        }
        if let Some(event) = bots::event_of(line) {
            self.bot_heard(&Home::Machine(machine.to_string()), &event, cx);
        }
    }

    fn bots_ended_there(&mut self, machine: &str, start: u64, cx: &mut Context<Self>) {
        let Some(link) = self.running(machine, start) else { return };
        link.stop();
        let wait = cli::backoff(link.failures);
        link.failures += 1;
        link.next = Some(Instant::now() + wait);
        self.there.bots.revision += 1;
        self.there_failed(machine, cx);
        later(wait, cx);
        cx.notify();
    }

    fn stop_bots_there(&mut self, machine: &str, cx: &mut Context<Self>) {
        let Some(link) = self.there.bots.links.get_mut(machine) else { return };
        let stopped = link.stop();
        (link.failures, link.next) = (0, None);
        if stopped {
            self.there.bots.revision += 1;
            cx.notify();
        }
    }

    fn drop_bots_there(&mut self, machine: &str, cx: &mut Context<Self>) {
        let keys: Vec<BotKey> = self
            .there
            .bots
            .links
            .get(machine)
            .map_or_else(Vec::new, |link| link.bots.list.iter().map(Bot::key).collect());
        for key in &keys {
            self.close_dialog_naming(key, cx);
        }
        let Some(link) = self.there.bots.links.remove(machine) else { return };
        let home = Home::Machine(machine.to_string());
        for bot in &link.bots.list {
            self.drop_bot_toasts(&bot.key(), cx);
        }
        let shown = self.shown_bot().is_some_and(|key| key.home == home);
        self.place(Moved::Listed(home, Vec::new()));
        if shown {
            self.toast(format!("{machine} is no longer listed"), ToastKind::Info, cx);
        }
        self.there.bots.revision += 1;
        cx.notify();
    }
}

#[cfg(test)]
mod tests;
