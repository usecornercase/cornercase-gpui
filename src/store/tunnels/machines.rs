use std::collections::BTreeSet;

use gpui::Context;

use super::Store;
use super::links::{Link, followed, machine_link, split_stale};
use crate::cli;
use crate::model::tunnels::Toward;
use crate::store::machine_read::reach;

impl Store {
    pub(super) fn machines_followed(&self) -> BTreeSet<String> {
        let listed = self.machines.iter().map(|machine| machine.name.clone());
        let seen = self.session.iter().flat_map(|session| session.machines.iter().map(|seen| seen.name.clone()));
        let linked = self.tunnels.machines.keys().cloned();
        let held = self.tunnels.holds.iter().map(|hold| &hold.toward);
        let settling = self.tunnels.settling.iter().map(|settling| &settling.toward);
        let taking = self.tunnels.takers.keys();
        followed(listed.chain(seen).chain(linked), held.chain(settling).chain(taking))
    }

    pub(super) fn link_of_machine(&self, name: &str) -> Link {
        let listed = self.machines.iter().find(|machine| machine.name == name);
        let seen = self.session.as_ref().and_then(|session| session.machine(name));
        let reached = seen.map(|seen| reach(&cli::target(), &self.machines, seen));
        let known = self.machines_known();
        machine_link(listed, known, seen, reached.as_ref(), &self.machine_tunnel_gate(name))
    }

    pub(crate) fn follow_machine_tunnels(&mut self, cx: &mut Context<Self>) {
        for name in self.machines_followed() {
            self.follow_machine(&name, cx);
        }
        self.settle_tunnels(cx);
        self.follow_takers(cx);
    }

    fn follow_machine(&mut self, name: &str, cx: &mut Context<Self>) {
        let toward = Toward::Machine(name.to_string());
        let link = self.link_of_machine(name);
        match &link {
            Link::Up { boot, destination } => {
                if self.tunnels.machines.get(name) != Some(boot) {
                    self.tunnels.machines.insert(name.to_string(), boot.clone());
                    self.link_tunnels(&toward, destination, cx);
                }
            }
            Link::Waiting(_) => drop(self.tunnels.machines.remove(name)),
            Link::Lost => {
                self.tunnels.machines.remove(name);
                self.tunnels.settling.retain(|settling| settling.toward != toward);
            }
        }
        let (ended, kept) = split_stale(std::mem::take(&mut self.tunnels.holds), &toward, &link);
        self.tunnels.holds = kept;
        self.end_holds_now(ended, cx);
    }
}
