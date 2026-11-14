use std::time::Instant;

use gpui::{App, Context, Entity, IntoElement as _};

use super::Browser;
use super::ports::web_ports;
use crate::model::{MachineSeen, Owner, WorkspaceThere};
use crate::panels::PanelKind;
use crate::panels::on_machine::{self, Missed};
use crate::panels::ports::{Listed, sample_there};
use crate::store::{MachineLink, Reach, Store};

pub(super) fn on_screen(cx: &App) -> (bool, Owner) {
    let store = Store::get(cx).read(cx);
    (store.dock == Some(PanelKind::Browser), store.shown_owner())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Fate {
    Stays,
    Moves(WorkspaceThere),
    Ends,
}

pub(crate) fn fate(owner: &WorkspaceThere, seen: &[MachineSeen], link: &MachineLink) -> Fate {
    if !link.stays_for(owner.destination()) {
        return Fate::Ends;
    }
    match owner.followed(seen) {
        None => Fate::Ends,
        Some(now) if now.aim() != owner.aim() => Fate::Moves(now),
        Some(_) => Fate::Stays,
    }
}

pub(crate) fn refused_why(reach: Reach) -> Option<String> {
    match reach {
        Reach::Refused(why) | Reach::Waiting(why) => Some(why),
        Reach::Up(_) | Reach::Down(_) | Reach::Older(_) => None,
    }
}

impl Browser {
    pub(super) fn measure_there(&mut self, cx: &mut Context<Self>) {
        let Some(read) = Store::get(cx).read(cx).machine_read() else { return };
        let Some(target) = read.reach.target().cloned() else {
            self.dev.retry.forget_while_older(&read.reach);
            cx.notify();
            return;
        };
        if !self.dev.retry.due(Instant::now()) || !self.dev.watch_begin() {
            return;
        }
        let generation = self.dev.generation;
        sample_there(cx, (&read, target, false), move |this, answer, cx| this.sampled_there(generation, answer, cx));
    }

    fn sampled_there(
        &mut self,
        generation: u64,
        answer: Result<Vec<crate::panels::ports::data::Port<crate::model::ThereId>>, Missed>,
        cx: &mut Context<Self>,
    ) {
        self.dev.watch_end();
        if generation != self.dev.generation {
            crate::panels::poll::Polled::measure(self, cx);
            return;
        }
        match answer {
            Ok(ports) => {
                self.dev.retry.heard();
                (self.dev.listed, self.dev.error) = (Listed::There(web_ports(ports)), None);
            }
            Err(missed) => self.dev.retry.missed(missed, Instant::now()),
        }
        self.auto_open(cx);
        cx.notify();
    }

    pub(super) fn machine_strip(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let read = Store::get(cx).read(cx).machine_read().filter(|_| self.place.machine().is_some())?;
        let offer = Store::get(cx).read(cx).update_offer(read.reach.older());
        Some(on_machine::strip(&read, &self.dev.retry, &self.counting, (false, &offer)).into_any_element())
    }

    pub(super) fn refused_place(&self, cx: &Context<Self>) -> Option<String> {
        let name = self.place.machine()?;
        refused_why(Store::get(cx).read(cx).reach_of(name)?)
    }

    fn machine_owners(&self) -> Vec<WorkspaceThere> {
        let mut found: Vec<WorkspaceThere> = Vec::new();
        let tabs = self.tabs.list.iter().map(|t| &t.owner);
        let restore = self.restore.iter().map(|r| &r.owner);
        let held = self.dev.held_opens.iter().map(super::held::HeldOpen::owner);
        let seen = self.dev.seen.iter().map(|(owner, _)| owner);
        for owner in tabs.chain(restore).chain(held).chain(seen) {
            if let Some(ws) = owner.there().filter(|ws| !found.contains(ws)) {
                found.push(ws.clone());
            }
        }
        found
    }

    pub(super) fn follow_owners(&mut self, store: &Entity<Store>, cx: &mut Context<Self>) {
        let owners = self.machine_owners();
        let s = store.read(cx);
        let Some(session) = s.session.as_ref().filter(|_| !owners.is_empty()) else { return };
        let mut changes = Vec::new();
        for owner in owners {
            match fate(&owner, &session.machines, &s.machine_link(owner.machine())) {
                Fate::Stays => {}
                other => changes.push((owner, other)),
            }
        }
        for (owner, fate) in changes {
            match fate {
                Fate::Ends => self.end_owner(&owner),
                Fate::Moves(now) => self.move_owner(&owner, &now),
                Fate::Stays => {}
            }
        }
        cx.notify();
    }

    fn end_owner(&mut self, ended: &WorkspaceThere) {
        let owner = Owner::There(ended.clone());
        let targets: Vec<String> =
            self.tabs.list.iter().filter(|t| t.owner == owner).map(|t| t.target.clone()).collect();
        for target in targets {
            self.close_tab(&target);
        }
        self.restore.retain(|r| r.owner != owner);
        self.dev.held_opens.retain(|o| !o.owned_by(&owner));
        self.dev.seen.retain(|(seen, _)| *seen != owner);
        self.agents.chosen.remove(&owner);
    }

    fn move_owner(&mut self, old: &WorkspaceThere, now: &WorkspaceThere) {
        let (from, to) = (Owner::There(old.clone()), Owner::There(now.clone()));
        self.tabs.rekey(&from, &to);
        self.restore.iter_mut().filter(|r| r.owner == from).for_each(|r| r.owner = to.clone());
        for open in self.dev.held_opens.iter_mut().filter(|o| o.owned_by(&from)) {
            *open.owner_mut() = to.clone();
        }
        let moved: Vec<u16> = self.dev.seen.iter().filter(|(o, _)| *o == from).map(|(_, port)| *port).collect();
        self.dev.seen.retain(|(seen, _)| *seen != from);
        self.dev.seen.extend(moved.into_iter().map(|port| (to.clone(), port)));
        self.agents.chosen.remove(&from);
    }
}

#[cfg(test)]
mod tests;
