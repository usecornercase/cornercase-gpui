use super::*;
use crate::model::ThereId;
use crate::store::{MachineRead, Reach};
use crate::test_util::{vps_aim, vps_read, vps_target};

#[test]
fn a_picked_machine_place_wins_over_the_workspace_on_screen() {
    let read = vps_read(Reach::Up(vps_target()));
    assert!(matches!(source_of(Some(read), Some(3)), Source::There(_)));
    assert!(matches!(source_of(None, Some(3)), Source::Here(Some(3))));
}

#[test]
fn a_new_aim_all_or_destination_is_a_new_scope() {
    let read = vps_read(Reach::Up(vps_target()));
    let first = scope_of(&Source::There(Box::new(read.clone())), false);
    assert_eq!(first, scope_of(&Source::There(Box::new(read.clone())), false));
    assert_ne!(first, scope_of(&Source::There(Box::new(read.clone())), true));
    let aim = Aim { workspace: ThereId::heard(8), ..vps_aim() };
    assert_ne!(first, scope_of(&Source::There(Box::new(MachineRead { aim, ..read.clone() })), false));
    let moved = MachineRead { destination: "other".into(), ..read };
    assert_ne!(first, scope_of(&Source::There(Box::new(moved)), false));
    assert_ne!(first, Scope::Here { all: false, workspace: Some(3) });
}
