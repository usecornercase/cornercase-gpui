use super::{Passage, Step, passage, step, tabs_of_a_server};
use crate::model::tunnels::{Older, Way};
use crate::store::Held;

#[test]
fn an_open_waits_for_its_hold_and_opens_on_the_local_port() {
    assert_eq!(step(Some(&Held::Opening), true), Step::Wait);
    assert_eq!(step(None, false), Step::Wait);
    assert_eq!(step(Some(&Held::Up { local: 5174 }), true), Step::Open(5174));
}

#[test]
fn an_open_whose_hold_failed_is_dropped() {
    assert_eq!(step(None, true), Step::Drop);
}

#[test]
fn a_preview_goes_only_through_a_tunnel_that_is_up() {
    let up = Held::Up { local: 5174 };
    assert_eq!(passage(&Way::Cornercase, Some(&up)), Passage::Through(5174));
    assert_eq!(passage(&Way::Cornercase, Some(&Held::Opening)), Passage::Waiting);
    assert_eq!(passage(&Way::Cornercase, None), Passage::Not);
    assert_eq!(passage(&Way::Waiting, None), Passage::Waiting);
    assert_eq!(passage(&Way::Own(Older::Server(None)), None), Passage::Not);
}

#[test]
fn leaving_a_server_closes_the_tabs_of_its_workspaces_only() {
    let tabs = [("a", Some(3)), ("b", None), ("c", Some(7))];
    assert_eq!(tabs_of_a_server(tabs.into_iter()), ["a", "c"]);
}

#[test]
fn an_open_waits_for_a_tunnel_being_closed_and_asks_again() {
    assert_eq!(step(Some(&Held::Ending), true), Step::Wait);
    assert_eq!(step(None, false), Step::Wait);
}
