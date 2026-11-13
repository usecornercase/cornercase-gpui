use super::{Fate, fate};
use crate::cli::Version;
use crate::model::machines::seen::{MachineSeen, ThereId, ThereProject, ThereWorkspace, WorkspaceThere};
use crate::model::machines::{LinkState, Machine};
use crate::model::tunnels::{Gate, Older};
use crate::store::{MachineLink, Reach};
use crate::test_util::vps_target;

fn seen(boot: &str, state: LinkState, ws: &[(u64, &str)]) -> MachineSeen {
    let workspaces = ws
        .iter()
        .map(|(id, path)| ThereWorkspace {
            id: ThereId::heard(*id),
            path: (*path).into(),
            name: "app".into(),
            ..Default::default()
        })
        .collect();
    let project = ThereProject { workspaces, ..Default::default() };
    MachineSeen { name: "vps".into(), boot: boot.into(), state, projects: vec![project], ..Default::default() }
}

fn owner(seen: &MachineSeen) -> WorkspaceThere {
    let ws = seen.workspaces().next().expect("a workspace");
    WorkspaceThere::at(seen, ws, "vps.invalid")
}

fn listed(destination: &str) -> Machine {
    Machine { name: "vps".into(), destination: destination.into(), ..Default::default() }
}

struct Facts<'a> {
    listed: Option<&'a Machine>,
    known: bool,
    machines: &'a [MachineSeen],
    reach: Option<Reach>,
}

fn answers(owner: &WorkspaceThere, facts: &Facts<'_>, gate: Gate) -> (bool, Fate) {
    let entry = facts.machines.iter().find(|m| m.name == "vps");
    let link = MachineLink::of(facts.listed, facts.known, entry, facts.reach.as_ref());
    (link.clone().gated(gate).stays_for(owner.destination()), fate(owner, facts.machines, &link))
}

fn up() -> Reach {
    Reach::Up(vps_target())
}

fn down() -> Reach {
    Reach::Down("vps unreachable".into())
}

fn unlisted() -> Reach {
    Reach::Refused("vps is no longer listed".into())
}

fn old() -> Option<Version> {
    Version::parse("0.13.45")
}

fn linked() -> MachineSeen {
    seen("b1", LinkState::Linked, &[(1, "/srv/app")])
}

fn unreachable() -> MachineSeen {
    seen("b1", LinkState::Unreachable, &[])
}

fn gates() -> [Gate; 4] {
    [Gate::Open, Gate::Waiting, Gate::Older(Older::Server(None)), Gate::Older(Older::Server(old()))]
}

#[test]
fn a_reachable_machine_keeps_its_holds_and_its_tabs() {
    let (s, same) = (linked(), listed("vps.invalid"));
    let facts = Facts { listed: Some(&same), known: true, machines: std::slice::from_ref(&s), reach: Some(up()) };
    assert_eq!(answers(&owner(&s), &facts, Gate::Open), (true, Fate::Stays));
}

#[test]
fn a_machine_pointed_elsewhere_ends_its_holds_and_its_tabs() {
    let (s, moved) = (linked(), listed("other.invalid"));
    let facts = Facts { listed: Some(&moved), known: true, machines: std::slice::from_ref(&s), reach: Some(up()) };
    for gate in gates() {
        assert_eq!(answers(&owner(&s), &facts, gate), (false, Fate::Ends), "{gate:?}");
    }
}

#[test]
fn a_machine_pointed_elsewhere_while_down_ends_its_holds_and_its_tabs() {
    let (s, down_now, moved) = (linked(), unreachable(), listed("other.invalid"));
    let facts = Facts { listed: Some(&moved), known: true, machines: &[down_now], reach: Some(down()) };
    assert_eq!(answers(&owner(&s), &facts, Gate::Open), (false, Fate::Ends));
}

#[test]
fn a_machine_that_only_waits_keeps_its_holds_and_its_tabs() {
    let (s, down_now, same) = (linked(), unreachable(), listed("vps.invalid"));
    let facts = Facts { listed: Some(&same), known: true, machines: &[down_now], reach: Some(down()) };
    assert_eq!(answers(&owner(&s), &facts, Gate::Open), (true, Fate::Stays));
}

#[test]
fn a_refused_machine_ends_its_holds_and_its_tabs() {
    let (s, same) = (linked(), listed("vps.invalid"));
    let refused = Some(Reach::Refused("vps runs an older cornercase".into()));
    let older = seen("b1", LinkState::Older, &[]);
    let facts = Facts { listed: Some(&same), known: true, machines: &[older], reach: refused };
    assert_eq!(answers(&owner(&s), &facts, Gate::Open), (false, Fate::Ends));
}

#[test]
fn a_machine_left_out_of_the_read_list_ends_its_holds_and_its_tabs() {
    let s = linked();
    let facts = Facts { listed: None, known: true, machines: std::slice::from_ref(&s), reach: Some(unlisted()) };
    assert_eq!(answers(&owner(&s), &facts, Gate::Open), (false, Fate::Ends));
}

#[test]
fn a_listed_machine_missing_from_the_snapshot_ends_its_holds_and_its_tabs() {
    let (s, same) = (linked(), listed("vps.invalid"));
    let facts = Facts { listed: Some(&same), known: true, machines: &[], reach: None };
    assert_eq!(answers(&owner(&s), &facts, Gate::Open), (false, Fate::Ends));
}

#[test]
fn a_machine_that_links_before_the_list_is_read_keeps_its_holds_and_its_tabs() {
    let s = linked();
    let facts = Facts { listed: None, known: false, machines: std::slice::from_ref(&s), reach: Some(unlisted()) };
    for gate in gates() {
        assert_eq!(answers(&owner(&s), &facts, gate), (true, Fate::Stays), "{gate:?}");
    }
}

#[test]
fn a_machine_down_before_the_list_is_read_keeps_its_holds_and_its_tabs() {
    let (s, down_now) = (linked(), unreachable());
    let facts = Facts { listed: None, known: false, machines: &[down_now], reach: Some(down()) };
    assert_eq!(answers(&owner(&s), &facts, Gate::Open), (true, Fate::Stays));
}

#[test]
fn a_hold_waiting_for_a_release_stays_with_its_tab() {
    let (s, same) = (linked(), listed("vps.invalid"));
    let facts = Facts { listed: Some(&same), known: true, machines: std::slice::from_ref(&s), reach: Some(up()) };
    for gate in [Gate::Waiting, Gate::Older(Older::Server(None))] {
        assert_eq!(answers(&owner(&s), &facts, gate), (true, Fate::Stays), "{gate:?}");
    }
}

#[test]
fn an_older_side_ends_the_holds_but_not_the_tabs() {
    let (s, same) = (linked(), listed("vps.invalid"));
    let facts = Facts { listed: Some(&same), known: true, machines: std::slice::from_ref(&s), reach: Some(up()) };
    for gate in [Gate::Older(Older::ThisComputer(old())), Gate::Older(Older::Server(old()))] {
        assert_eq!(answers(&owner(&s), &facts, gate), (false, Fate::Stays), "{gate:?}");
    }
}

#[test]
fn a_workspace_closed_there_ends_the_tab_but_not_the_holds() {
    let (s, same) = (linked(), listed("vps.invalid"));
    let after = seen("b1", LinkState::Linked, &[(2, "/srv/other")]);
    let facts = Facts { listed: Some(&same), known: true, machines: &[after], reach: Some(up()) };
    assert_eq!(answers(&owner(&s), &facts, Gate::Open), (true, Fate::Ends));
}

#[test]
fn a_restart_moves_the_tab_and_keeps_the_holds() {
    let (s, same) = (linked(), listed("vps.invalid"));
    let after = seen("b2", LinkState::Linked, &[(4, "/srv/app")]);
    let facts = Facts { listed: Some(&same), known: true, machines: std::slice::from_ref(&after), reach: Some(up()) };
    assert_eq!(answers(&owner(&s), &facts, Gate::Open), (true, Fate::Moves(owner(&after))));
}

#[test]
fn a_machine_gone_from_the_snapshot_before_the_list_is_read_ends_the_tab_but_not_the_holds() {
    let s = linked();
    let facts = Facts { listed: None, known: false, machines: &[], reach: None };
    assert_eq!(answers(&owner(&s), &facts, Gate::Open), (true, Fate::Ends));
}

#[test]
fn a_tab_opened_before_the_list_named_a_destination_is_never_pointed_elsewhere() {
    let s = linked();
    let blank = WorkspaceThere::at(&s, s.workspaces().next().expect("a workspace"), "");
    let (other, same) = (listed("other.invalid"), listed("vps.invalid"));
    let machines = std::slice::from_ref(&s);
    let linked_up = Facts { listed: Some(&other), known: true, machines, reach: Some(up()) };
    assert_eq!(answers(&blank, &linked_up, Gate::Open).1, Fate::Stays);
    let waiting = Facts { listed: Some(&same), known: true, machines: &[unreachable()], reach: Some(down()) };
    assert_eq!(answers(&blank, &waiting, Gate::Open).1, Fate::Stays);
}
