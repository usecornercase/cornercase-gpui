use std::collections::BTreeSet;

use gpui::Context;

use super::Store;
use super::held::unsent;
use super::links::{Linking, MachineLink, followed, linking, split_stale};
use super::rows;
use crate::model::tunnels::Toward;

impl Store {
    pub(super) fn machines_followed(&self) -> BTreeSet<String> {
        let listed = self.machines.iter().map(|machine| machine.name.clone());
        let seen = self.session.iter().flat_map(|session| session.machines.iter().map(|seen| seen.name.clone()));
        let linked = self.tunnels.machines.keys().cloned();
        let held = self.tunnels.holds.iter().map(|hold| &hold.toward);
        let settling = self.tunnels.settling.iter().map(|settling| &settling.toward);
        let taking = self.tunnels.takers.keys();
        let reversing = self.tunnels.reverses.keys();
        followed(listed.chain(seen).chain(linked), held.chain(settling).chain(taking).chain(reversing))
    }

    pub(crate) fn machine_link(&self, name: &str) -> MachineLink {
        let listed = self.machines.iter().find(|machine| machine.name == name);
        let seen = self.session.as_ref().and_then(|session| session.machine(name));
        MachineLink::of(listed, self.machines_known(), seen, self.reach_of(name).as_ref())
    }

    pub(super) fn tunnel_link(&self, name: &str) -> MachineLink {
        self.machine_link(name).gated(self.machine_tunnel_gate(name))
    }

    pub(crate) fn follow_machine_tunnels(&mut self, cx: &mut Context<Self>) {
        let machines = self.session.as_ref().map_or(&[][..], |session| session.machines.as_slice());
        rows::pruned(&mut self.tunnels.ended_unsent, machines);
        for name in self.machines_followed() {
            self.follow_machine(&name, cx);
        }
        self.settle_tunnels(cx);
        self.follow_takers(cx);
    }

    fn follow_machine(&mut self, name: &str, cx: &mut Context<Self>) {
        let toward = Toward::Machine(name.to_string());
        let link = self.tunnel_link(name);
        match linking(&link, self.tunnels.machines.get(name).map(String::as_str)) {
            Linking::Link { boot, destination } => {
                self.tunnels.machines.insert(name.to_string(), boot.to_string());
                self.tunnels.ended_unsent.retain(|(machine, ..)| machine != name);
                self.link_tunnels(&toward, destination, cx);
            }
            Linking::Pause => drop(self.tunnels.machines.remove(name)),
            Linking::Lose => {
                self.tunnels.machines.remove(name);
                self.tunnels.settling.retain(|settling| settling.toward != toward);
            }
            Linking::Stay => {}
        }
        let (ended, kept) = split_stale(std::mem::take(&mut self.tunnels.holds), &toward, &link);
        self.tunnels.holds = kept;
        self.tunnels.ended_unsent.extend(ended.iter().filter_map(unsent));
        self.end_holds_now(ended, cx);
        self.follow_machine_reverse(name, cx);
    }
}
