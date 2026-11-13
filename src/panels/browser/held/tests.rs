use super::{Dispatch, Route, Typed, dispatch, repointed, route, typed_route};
use super::{HeldOpen, Passage, Step, asks_now, passage, step, superseded, tabs_of_a_server};
use crate::model::Owner;
use crate::model::link::loopback_port;
use crate::model::tunnels::MachineOpen;
use crate::model::tunnels::{Older, Toward, Way};
use crate::panels::browser::tabs::Through;
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
    let (three, seven) = (Owner::Here(3), Owner::Here(7));
    let tabs = [("a", &three), ("b", &Owner::Nobody), ("c", &seven)];
    assert_eq!(tabs_of_a_server(tabs.into_iter()), ["a", "c"]);
}

#[test]
fn an_open_made_while_its_tunnel_is_closing_waits_and_asks_after() {
    assert!(!asks_now(false, Some(&Held::Ending)));
    assert_eq!(step(Some(&Held::Ending), false), Step::Wait);
    assert!(asks_now(false, None));
}

#[test]
fn an_open_asked_before_a_close_is_dropped_so_the_close_stands() {
    assert_eq!(step(Some(&Held::Ending), true), Step::Drop);
    assert!(!asks_now(true, None));
}

#[test]
fn a_navigation_elsewhere_drops_the_opens_waiting_for_that_tab() {
    let open = |session: Option<&str>| HeldOpen::waiting(5173, session);
    let mut opens = vec![open(Some("a")), open(Some("b")), open(None)];
    superseded(&mut opens, "a");
    assert_eq!(opens.iter().map(|o| o.session.as_deref()).collect::<Vec<_>>(), [Some("b"), None]);
}

fn every_way() -> [Way; 4] {
    [Way::Here, Way::Waiting, Way::Own(Older::Server(None)), Way::Cornercase]
}

#[test]
fn a_machine_open_never_opens_this_computers_port() {
    let toward = Toward::Machine("vps".into());
    let url = "http://localhost:8765";
    let opens = [MachineOpen::Open, MachineOpen::Waiting, MachineOpen::Closed("vps is down".into())];
    for way in every_way() {
        for open in &opens {
            let routed = route(url, Some(&toward), way, Some(open));
            assert!(!matches!(routed, Route::Direct | Route::Own(_)), "{way:?} {open:?} gave {routed:?}");
        }
    }
}

#[test]
fn a_machine_open_waits_while_the_release_is_unread() {
    let toward = Toward::Machine("vps".into());
    let routed = route("http://localhost:8765", Some(&toward), Way::Here, Some(&MachineOpen::Waiting));
    assert_eq!(routed, Route::Held(toward, 8765));
}

#[test]
fn a_machine_open_is_refused_with_why_when_the_machine_cannot_be_reached() {
    let toward = Toward::Machine("vps".into());
    let closed = MachineOpen::Closed("vps unreachable".into());
    let routed = route("http://localhost:8765", Some(&toward), Way::Cornercase, Some(&closed));
    assert_eq!(routed, Route::Refused("vps unreachable".into()));
}

#[test]
fn a_server_open_keeps_its_way() {
    let url = "http://localhost:8765";
    let server = Some(&Toward::Server);
    assert_eq!(route(url, server, Way::Own(Older::Server(None)), None), Route::Own(8765));
    assert_eq!(route(url, server, Way::Cornercase, None), Route::Held(Toward::Server, 8765));
    assert_eq!(route(url, server, Way::Waiting, None), Route::Held(Toward::Server, 8765));
    assert_eq!(route(url, server, Way::Here, None), Route::Direct);
    assert_eq!(route("https://example.com", server, Way::Cornercase, None), Route::Direct);
    assert_eq!(route(url, None, Way::Cornercase, None), Route::Direct);
}

#[test]
fn a_machines_tabs_end_with_the_servers() {
    let there = machine_owner();
    let tabs = [("a", &Owner::Here(1)), ("m", &there), ("n", &Owner::Nobody)];
    assert_eq!(tabs_of_a_server(tabs.into_iter()), ["a", "m"]);
}

fn machine_owner() -> Owner {
    use crate::model::machines::seen::{MachineSeen, ThereId, ThereProject, ThereWorkspace, WorkspaceThere};
    let ws = ThereWorkspace { id: ThereId::heard(2), ..Default::default() };
    let project = ThereProject { workspaces: vec![ws.clone()], ..Default::default() };
    let seen = MachineSeen { name: "vps".into(), projects: vec![project], ..Default::default() };
    Owner::There(WorkspaceThere::at(&seen, &ws, "vps.invalid"))
}

#[test]
fn a_machine_open_is_never_handed_over_under_any_way() {
    let toward = Toward::Machine("vps".into());
    for way in every_way() {
        assert_eq!(dispatch(&toward, way), Dispatch::Hold, "{way:?}");
    }
    assert_eq!(dispatch(&Toward::Server, Way::Here), Dispatch::Direct);
    assert_eq!(dispatch(&Toward::Server, Way::Waiting), Dispatch::Wait);
    assert_eq!(dispatch(&Toward::Server, Way::Own(Older::Server(None))), Dispatch::Own);
    assert_eq!(dispatch(&Toward::Server, Way::Cornercase), Dispatch::Hold);
}

#[test]
fn a_closed_machine_gives_a_preview_the_closed_passage() {
    use super::{Passage, machine_passage};
    let closed = MachineOpen::Closed("older".into());
    assert_eq!(machine_passage(&closed, Some(&Held::Up { local: 5 })), Passage::Closed("older".into()));
    assert_eq!(machine_passage(&MachineOpen::Open, Some(&Held::Up { local: 5 })), Passage::Through(5));
    assert_eq!(machine_passage(&MachineOpen::Open, None), Passage::Not);
}

#[test]
fn a_tab_follows_its_hold_to_another_local_port() {
    let through = Through { remote: 8765, local: 61000 };
    let (url, now) = repointed("http://localhost:61000/a", through, Some(62000)).expect("it moves");
    assert_eq!((url.as_str(), now), ("http://localhost:62000/a", Through { remote: 8765, local: 62000 }));
    assert_eq!(repointed("http://localhost:61000/", through, Some(61000)), None);
    assert_eq!(repointed("http://localhost:61000/", through, None), None);
}

#[test]
fn a_tab_navigated_elsewhere_is_not_moved() {
    let through = Through { remote: 8765, local: 61000 };
    assert_eq!(repointed("https://example.com/", through, Some(62000)), None);
    assert_eq!(loopback_port("http://localhost:61000/"), Some(61000));
}

#[test]
fn a_hold_that_is_down_moves_no_tab() {
    let through = Through { remote: 8765, local: 61000 };
    assert_eq!(repointed("http://localhost:61000/", through, None), None);
}

#[test]
fn a_typed_port_goes_through_the_hold_when_listed_or_held() {
    let toward = Toward::Machine("vps".into());
    assert_eq!(typed_route(Some(8765), Some(&toward), (true, false)), Typed::Through(8765));
}

#[test]
fn a_typed_port_that_stays_on_this_computer_says_so() {
    let toward = Toward::Machine("vps".into());
    assert_eq!(typed_route(Some(5173), Some(&toward), (false, false)), Typed::HereSaid(5173));
    assert_eq!(typed_route(None, Some(&toward), (false, false)), Typed::Here);
    assert_eq!(typed_route(Some(5173), None, (false, false)), Typed::Here);
}

#[test]
fn a_typed_hold_local_port_stays_without_a_toast() {
    let toward = Toward::Machine("vps".into());
    assert_eq!(typed_route(Some(61000), Some(&toward), (false, true)), Typed::Here);
}

#[test]
fn a_newer_typed_address_cancels_a_queued_one_without_a_session() {
    use super::{HeldOpen, superseded_typed};
    let mut opens = vec![HeldOpen::waiting(8765, None), HeldOpen::waiting(9000, Some("s"))];
    superseded_typed(&mut opens, &Owner::Nobody);
    assert_eq!(opens.len(), 1);
}

#[test]
fn a_newer_held_typed_address_cancels_the_older_one_without_a_session() {
    use super::{HeldOpen, superseded_typed};
    let mut opens = vec![HeldOpen::waiting(8765, None), HeldOpen::waiting(3000, None)];
    superseded_typed(&mut opens, &Owner::Nobody);
    assert!(opens.is_empty());
}
