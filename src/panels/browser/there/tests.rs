use super::{Fate, Listing, fate};
use crate::model::machines::seen::{MachineSeen, ThereId, ThereProject, ThereWorkspace, WorkspaceThere};
use crate::model::machines::{LinkState, Machine};
use crate::store::Reach;

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

fn known(machine: Option<&Machine>) -> Listing<'_> {
    Listing { known: true, listed: machine }
}

#[test]
fn a_machine_tab_ends_when_its_machine_leaves_the_list() {
    let s = seen("b1", LinkState::Linked, &[(1, "/srv/app")]);
    assert_eq!(fate(&owner(&s), std::slice::from_ref(&s), known(None), None), Fate::Ends);
}

#[test]
fn a_machine_tab_ends_when_its_machine_points_at_another_destination() {
    let s = seen("b1", LinkState::Linked, &[(1, "/srv/app")]);
    let moved = listed("other.invalid");
    assert_eq!(fate(&owner(&s), std::slice::from_ref(&s), known(Some(&moved)), None), Fate::Ends);
}

#[test]
fn a_machine_tab_ends_when_its_machine_is_refused() {
    let s = seen("b1", LinkState::Linked, &[(1, "/srv/app")]);
    let same = listed("vps.invalid");
    let refused = Reach::Refused("vps runs an older cornercase".into());
    assert_eq!(fate(&owner(&s), std::slice::from_ref(&s), known(Some(&same)), Some(&refused)), Fate::Ends);
}

#[test]
fn a_machine_tab_ends_when_its_workspace_closed_there() {
    let s = seen("b1", LinkState::Linked, &[(1, "/srv/app")]);
    let after = seen("b1", LinkState::Linked, &[(2, "/srv/other")]);
    let same = listed("vps.invalid");
    assert_eq!(fate(&owner(&s), &[after], known(Some(&same)), None), Fate::Ends);
}

#[test]
fn a_machine_tab_moves_with_its_workspace_after_a_restart() {
    let s = seen("b1", LinkState::Linked, &[(1, "/srv/app")]);
    let after = seen("b2", LinkState::Linked, &[(4, "/srv/app")]);
    let same = listed("vps.invalid");
    let moved = fate(&owner(&s), std::slice::from_ref(&after), known(Some(&same)), None);
    assert_eq!(moved, Fate::Moves(owner(&after)));
}

#[test]
fn a_machine_tab_stays_while_the_link_is_down() {
    let s = seen("b1", LinkState::Linked, &[(1, "/srv/app")]);
    let down = seen("b1", LinkState::Unreachable, &[]);
    let same = listed("vps.invalid");
    let down_reach = Reach::Down("vps unreachable".into());
    assert_eq!(fate(&owner(&s), &[down], known(Some(&same)), Some(&down_reach)), Fate::Stays);
}

#[test]
fn a_machine_tab_stays_while_the_listing_is_not_known_yet() {
    let s = seen("b1", LinkState::Linked, &[(1, "/srv/app")]);
    let waiting = Listing { known: false, listed: None };
    assert_eq!(fate(&owner(&s), std::slice::from_ref(&s), waiting, None), Fate::Stays);
}
