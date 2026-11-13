use super::Owner;
use crate::model::machines::seen::{MachineSeen, ThereId, ThereProject, ThereWorkspace, WorkspaceThere};
use crate::model::tunnels::Toward;

fn machine_workspace(id: u64) -> WorkspaceThere {
    let ws =
        ThereWorkspace { id: ThereId::heard(id), path: "/srv/app".into(), name: "app".into(), ..Default::default() };
    let project = ThereProject { workspaces: vec![ws.clone()], ..Default::default() };
    let seen = MachineSeen { name: "vps".into(), boot: "b1".into(), projects: vec![project], ..Default::default() };
    WorkspaceThere::at(&seen, &ws, "vps.example")
}

#[test]
fn a_machine_workspace_and_ours_with_the_same_number_are_two_owners() {
    assert_ne!(Owner::Here(7), Owner::There(machine_workspace(7)));
    assert_ne!(Owner::There(machine_workspace(7)), Owner::There(machine_workspace(8)));
}

#[test]
fn an_owner_tunnels_toward_its_machine_or_the_window_server() {
    let there = Owner::There(machine_workspace(7));
    assert_eq!(there.toward(None), Some(Toward::Machine("vps".into())));
    assert_eq!(there.toward(Some("devbox")), Some(Toward::Machine("vps".into())));
    assert_eq!(Owner::Here(1).toward(Some("devbox")), Some(Toward::Server));
    assert_eq!(Owner::Here(1).toward(None), None);
    assert_eq!(Owner::Nobody.toward(Some("devbox")), Some(Toward::Server));
}

#[test]
fn nobody_owns_an_agents_tab_and_has_no_place() {
    assert!(!Owner::Nobody.has_place());
    assert!(Owner::Here(1).has_place());
    assert!(Owner::There(machine_workspace(1)).has_place());
    assert_eq!(Owner::Nobody.machine(), None);
}
