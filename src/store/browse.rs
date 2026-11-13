use gpui::Context;

use super::machine_read::Reach;
use super::{Store, ToastGo, ToastKind, remote_url};
use crate::model::tunnels::{Gate, MachineOpen, Toward, open_gate};
use crate::model::{Owner, WorkspaceThere};
use crate::panels::PanelKind;

pub(crate) fn listed_gate(known: bool, gate: Gate) -> Gate {
    if known { gate } else { Gate::Waiting }
}

pub(super) fn toast_go(toward: &Toward, port: u16) -> ToastGo {
    match toward {
        Toward::Server => ToastGo::Browse(remote_url(port)),
        Toward::Machine(name) => ToastGo::Tunnelled(name.clone(), port),
    }
}

pub(crate) fn owner_on_machine(shown: Owner, machine: &str) -> Option<Owner> {
    (shown.machine() == Some(machine)).then_some(shown)
}

impl Store {
    pub(crate) fn local_owner(&self) -> Owner {
        self.session.as_ref().and_then(|session| session.shown().workspace).map_or(Owner::Nobody, Owner::Here)
    }

    pub(crate) fn shown_owner(&self) -> Owner {
        let Some(place) = &self.machine_place else { return self.local_owner() };
        let listed = self.machines.iter().find(|machine| machine.name == place.machine);
        let destination = listed.map_or("", |machine| machine.destination.as_str());
        Owner::There(WorkspaceThere::of(place, destination))
    }

    pub(crate) fn owner_on(&self, machine: &str) -> Option<Owner> {
        owner_on_machine(self.shown_owner(), machine)
    }

    pub(crate) fn open_in_browser(&mut self, url: String, owner: Owner, cx: &mut Context<Self>) {
        self.browse = Some((url, owner));
        self.open(PanelKind::Browser, cx);
    }

    pub(super) fn open_tunnel(&mut self, machine: &str, port: u16, cx: &mut Context<Self>) {
        let toward = Toward::Machine(machine.to_string());
        if let Some(owner) = self.owner_on(machine) {
            return self.open_in_browser(remote_url(port), owner, cx);
        }
        match self.tunnel_local(&toward, port) {
            Some(local) => cx.open_url(&format!("http://localhost:{local}")),
            None => self.toast(format!("The tunnel of port {port} on {machine} is closed"), ToastKind::Info, cx),
        }
    }

    pub(crate) fn machine_open(&self, name: &str) -> MachineOpen {
        let toward = Toward::Machine(name.to_string());
        let gate = self.machine_tunnel_gate(name);
        open_gate(gate, self.can_tunnel(&toward), || self.tunnel_refusal(&toward))
    }

    pub(crate) fn machines_known(&self) -> bool {
        self.app.is_some() || !self.machines.is_empty()
    }

    pub(crate) fn reach_of(&self, name: &str) -> Option<Reach> {
        let seen = self.session.as_ref()?.machine(name)?;
        Some(self.reach_seen(seen))
    }

    pub(crate) fn not_opened(&mut self, why: &str, cx: &mut Context<Self>) {
        self.toast(format!("{}: nothing was opened", why.trim_end_matches('.')), ToastKind::Error, cx);
    }
}

#[cfg(test)]
mod tests;
