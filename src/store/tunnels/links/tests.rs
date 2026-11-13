use super::{Link, followed, hold_stays, machine_link, split_ended, split_stale};
use crate::model::MachineSeen;
use crate::model::machines::{LinkState, Machine};
use crate::model::tunnels::{Gate, Older, Toward, Way};
use crate::store::Reach;
use crate::store::tunnels::held::{Held, test_hold};
use crate::test_util::vps_target;

fn listed() -> Machine {
    Machine { name: "vps".into(), destination: "vps.invalid".into(), ..Machine::default() }
}

fn seen() -> MachineSeen {
    MachineSeen { name: "vps".into(), boot: "boot-a".into(), state: LinkState::Linked, ..MachineSeen::default() }
}

fn up() -> Reach {
    Reach::Up(vps_target())
}

fn link(listed: Option<&Machine>, known: bool, reach: &Reach, gate: &Gate) -> Link {
    machine_link(listed, known, Some(&seen()), Some(reach), gate)
}

#[test]
fn a_linked_machine_with_both_releases_is_up_on_its_boot() {
    let machine = listed();
    let expected = Link::Up { boot: "boot-a".into(), destination: "vps.invalid".into() };
    assert_eq!(link(Some(&machine), true, &up(), &Gate::Open), expected);
}

#[test]
fn a_link_that_comes_back_by_itself_only_waits() {
    let machine = listed();
    let down = Reach::Down("vps unreachable".into());
    assert_eq!(link(Some(&machine), true, &down, &Gate::Open), Link::Waiting(Some("vps.invalid".into())));
    assert_eq!(link(Some(&machine), true, &up(), &Gate::Waiting), Link::Waiting(Some("vps.invalid".into())));
}

#[test]
fn a_refused_or_older_machine_is_lost() {
    let machine = listed();
    let refused = Reach::Refused("not linked".into());
    assert_eq!(link(Some(&machine), true, &refused, &Gate::Open), Link::Lost);
    let older = Gate::Older(Older::Server(crate::cli::Version::parse("0.13.45")));
    assert_eq!(link(Some(&machine), true, &up(), &older), Link::Lost);
    let here = Gate::Older(Older::ThisComputer(None));
    assert_eq!(link(Some(&machine), true, &up(), &here), Link::Lost);
}

#[test]
fn a_server_version_not_read_yet_only_waits() {
    let machine = listed();
    let unknown = Gate::Older(Older::Server(None));
    assert_eq!(link(Some(&machine), true, &up(), &unknown), Link::Waiting(Some("vps.invalid".into())));
}

#[test]
fn a_machine_that_is_not_listed_is_lost_once_the_list_is_known() {
    assert_eq!(link(None, true, &up(), &Gate::Open), Link::Lost);
    assert_eq!(link(None, false, &up(), &Gate::Open), Link::Waiting(None));
    assert_eq!(machine_link(Some(&listed()), true, None, None, &Gate::Open), Link::Lost);
}

#[test]
fn a_hold_stays_only_for_the_destination_it_was_started_with() {
    let up = |destination: &str| Link::Up { boot: "b".into(), destination: destination.into() };
    assert!(hold_stays("vps.invalid", &up("vps.invalid")));
    assert!(!hold_stays("vps.invalid", &up("other.invalid")));
    assert!(hold_stays("vps.invalid", &Link::Waiting(None)));
    assert!(hold_stays("vps.invalid", &Link::Waiting(Some("vps.invalid".into()))));
    assert!(!hold_stays("vps.invalid", &Link::Lost));
}

#[test]
fn a_window_on_this_computer_never_ends_a_machines_holds() {
    let server = test_hold(Toward::Server, 5173, Held::Opening);
    let machine = test_hold(Toward::Machine("vps".into()), 8765, Held::Up { local: 1 });
    let (ended, kept) = split_ended(vec![server, machine], &Way::Here);
    assert_eq!(ended.iter().map(|h| h.port).collect::<Vec<_>>(), [5173]);
    assert_eq!(kept.iter().map(|h| h.port).collect::<Vec<_>>(), [8765]);
    let (ended, _) = split_ended(vec![test_hold(Toward::Server, 1, Held::Opening)], &Way::Cornercase);
    assert!(ended.is_empty());
}

#[test]
fn a_lost_machine_ends_only_its_own_holds() {
    let holds = vec![
        test_hold(Toward::Server, 5173, Held::Opening),
        test_hold(Toward::Machine("vps".into()), 5173, Held::Opening),
        test_hold(Toward::Machine("cap".into()), 5173, Held::Opening),
    ];
    let (ended, kept) = split_stale(holds, &Toward::Machine("vps".into()), &Link::Lost);
    assert_eq!(ended.len(), 1);
    assert_eq!(kept.len(), 2);
}

#[test]
fn a_hold_to_a_destination_changed_while_the_link_waits_ends() {
    let waiting = Link::Waiting(Some("other.invalid".into()));
    assert!(!hold_stays("vps.invalid", &waiting));
    let holds = vec![test_hold(Toward::Machine("vps".into()), 8765, Held::Up { local: 1 })];
    let (ended, kept) = split_stale(holds, &Toward::Machine("vps".into()), &waiting);
    assert_eq!((ended.len(), kept.len()), (1, 0));
}

#[test]
fn a_machine_with_only_a_settling_entry_is_still_followed() {
    let vps = Toward::Machine("vps".into());
    let names = followed(std::iter::empty(), [&vps, &Toward::Server].into_iter());
    assert_eq!(names.into_iter().collect::<Vec<_>>(), ["vps"]);
    let listed = followed(["cap".to_string()].into_iter(), std::iter::empty());
    assert_eq!(listed.into_iter().collect::<Vec<_>>(), ["cap"]);
}
