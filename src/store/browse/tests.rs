use super::{MachineOpen, listed_gate, open_gate, owner_on_machine, port_open, toast_go};
use crate::cli::Version;
use crate::model::Owner;
use crate::model::machines::seen::{MachineSeen, ThereId, ThereProject, ThereWorkspace, WorkspaceThere};
use crate::model::tunnels::Toward;
use crate::model::tunnels::{Gate, Older};
use crate::store::ToastGo;
use crate::store::tunnels::Held;

fn machine_owner(machine: &str) -> Owner {
    let ws = ThereWorkspace { id: ThereId::heard(3), ..Default::default() };
    let project = ThereProject { workspaces: vec![ws.clone()], ..Default::default() };
    let seen = MachineSeen { name: machine.into(), projects: vec![project], ..Default::default() };
    Owner::There(WorkspaceThere::at(&seen, &ws, "dest"))
}

#[test]
fn a_machines_tunnel_opens_in_the_browser_only_on_its_place() {
    assert!(owner_on_machine(machine_owner("vps"), "vps").is_some());
    assert!(owner_on_machine(machine_owner("vps"), "other").is_none());
    assert!(owner_on_machine(Owner::Here(1), "vps").is_none());
    assert!(owner_on_machine(Owner::Nobody, "vps").is_none());
}

#[test]
fn a_machine_open_says_why_it_cannot_go() {
    assert_eq!(open_gate(Gate::Open, true, || unreachable!()), MachineOpen::Open);
    assert_eq!(open_gate(Gate::Waiting, false, || unreachable!()), MachineOpen::Waiting);
    assert_eq!(open_gate(Gate::Open, false, || "vps is down".into()), MachineOpen::Closed("vps is down".into()));
    let older = Gate::Older(Older::Server(Some(Version::new(0, 13, 45))));
    assert_eq!(open_gate(older, false, || "needs newer".into()), MachineOpen::Closed("needs newer".into()));
}

#[test]
fn a_machines_tunnel_toast_opens_where_its_place_is() {
    assert_eq!(toast_go(&Toward::Machine("vps".into()), 8765), ToastGo::Tunnelled("vps".into(), 8765));
    assert_eq!(toast_go(&Toward::Server, 8765), ToastGo::Browse("http://localhost:8765/".into()));
}

#[test]
fn the_tunnel_gate_waits_while_the_machines_list_is_not_read() {
    let older = Gate::Older(Older::Server(None));
    assert_eq!(listed_gate(false, older), Gate::Waiting);
    assert_eq!(listed_gate(false, Gate::Open), Gate::Waiting);
    assert_eq!(listed_gate(true, older), older);
    assert_eq!(listed_gate(true, Gate::Open), Gate::Open);
}

#[test]
fn an_up_tunnel_opens_whatever_the_machine_says() {
    let closed = || MachineOpen::Closed("vps is older".into());
    assert_eq!(port_open(closed(), Some(&Held::Up { local: 4000 })), MachineOpen::Open);
    for held in [None, Some(&Held::Opening), Some(&Held::Down { why: String::new() })] {
        assert_eq!(port_open(closed(), held), closed());
    }
    assert_eq!(port_open(MachineOpen::Waiting, None), MachineOpen::Waiting);
}
