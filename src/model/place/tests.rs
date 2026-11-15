use super::*;
use crate::model::ThereWorkspace;

fn place() -> MachinePlace {
    let seen = crate::model::MachineSeen::default();
    MachinePlace::at(&seen, &ThereWorkspace::default(), None)
}

#[test]
fn showing_a_bot_leaves_a_machines_place() {
    let shown = Showing::Machine(place()).after(Moved::Bot("prs".into()));
    assert_eq!((shown.bot(), shown.machine()), (Some("prs"), None));
}

#[test]
fn picking_a_machines_place_leaves_the_chat() {
    assert_eq!(Showing::Bot("prs".into()).after(Moved::Machine(place())).bot(), None);
}

#[test]
fn a_move_to_a_local_place_leaves_the_chat() {
    assert_eq!(Showing::Bot("prs".into()).after(Moved::Local), Showing::Local);
}

#[test]
fn the_removal_of_the_shown_bot_leaves_its_chat() {
    assert_eq!(Showing::Bot("prs".into()).after(Moved::Removed("prs".into())), Showing::Local);
}

#[test]
fn the_removal_of_another_bot_keeps_the_chat() {
    assert_eq!(Showing::Bot("prs".into()).after(Moved::Removed("ops".into())).bot(), Some("prs"));
}

#[test]
fn a_list_without_the_shown_bot_leaves_its_chat() {
    let shown = Showing::Bot("prs".into());
    assert_eq!(shown.clone().after(Moved::Listed(vec!["ops".into()])), Showing::Local);
    assert_eq!(shown.after(Moved::Listed(vec!["prs".into()])).bot(), Some("prs"));
}
