use super::{Linking, MachineLink, followed, linking, split_ended, split_stale, tunnel_target};
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

fn link(listed: Option<&Machine>, known: bool, reach: &Reach, gate: Gate) -> MachineLink {
    MachineLink::of(listed, known, Some(&seen()), Some(reach)).gated(gate)
}

#[test]
fn a_linked_machine_with_both_releases_is_up_on_its_boot() {
    let machine = listed();
    let expected = MachineLink::Up { boot: "boot-a".into(), destination: "vps.invalid".into() };
    assert_eq!(link(Some(&machine), true, &up(), Gate::Open), expected);
}

#[test]
fn a_link_that_comes_back_by_itself_only_waits() {
    let machine = listed();
    let down = Reach::Down("vps unreachable".into());
    assert_eq!(link(Some(&machine), true, &down, Gate::Open), MachineLink::Waiting(Some("vps.invalid".into())));
    assert_eq!(link(Some(&machine), true, &up(), Gate::Waiting), MachineLink::Waiting(Some("vps.invalid".into())));
}

#[test]
fn a_refused_or_older_machine_is_lost() {
    let machine = listed();
    let refused = Reach::Refused("not linked".into());
    assert_eq!(link(Some(&machine), true, &refused, Gate::Open), MachineLink::Lost);
    let older = Gate::Older(Older::Server(crate::cli::Version::parse("0.13.45")));
    assert_eq!(link(Some(&machine), true, &up(), older), MachineLink::Lost);
    let here = Gate::Older(Older::ThisComputer(None));
    assert_eq!(link(Some(&machine), true, &up(), here), MachineLink::Lost);
}

#[test]
fn a_server_version_not_read_yet_only_waits() {
    let machine = listed();
    let unknown = Gate::Older(Older::Server(None));
    assert_eq!(link(Some(&machine), true, &up(), unknown), MachineLink::Waiting(Some("vps.invalid".into())));
}

#[test]
fn a_machine_that_is_not_listed_is_lost_once_the_list_is_known() {
    assert_eq!(link(None, true, &up(), Gate::Open), MachineLink::Lost);
    assert_eq!(link(None, false, &up(), Gate::Open), MachineLink::Waiting(None));
    assert_eq!(MachineLink::of(Some(&listed()), true, None, None), MachineLink::Lost);
}

#[test]
fn a_hold_stays_only_for_the_destination_it_was_started_with() {
    let up = |destination: &str| MachineLink::Up { boot: "b".into(), destination: destination.into() };
    assert!(up("vps.invalid").stays_for("vps.invalid"));
    assert!(!up("other.invalid").stays_for("vps.invalid"));
    assert!(MachineLink::Waiting(None).stays_for("vps.invalid"));
    assert!(MachineLink::Waiting(Some("vps.invalid".into())).stays_for("vps.invalid"));
    assert!(!MachineLink::Lost.stays_for("vps.invalid"));
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
    let (ended, kept) = split_stale(holds, &Toward::Machine("vps".into()), &MachineLink::Lost);
    assert_eq!(ended.len(), 1);
    assert_eq!(kept.len(), 2);
}

#[test]
fn a_hold_to_a_destination_changed_while_the_link_waits_ends() {
    let waiting = MachineLink::Waiting(Some("other.invalid".into()));
    assert!(!waiting.stays_for("vps.invalid"));
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

#[test]
fn a_reachable_listed_machine_is_up_before_any_gate() {
    let machine = listed();
    let expected = MachineLink::Up { boot: "boot-a".into(), destination: "vps.invalid".into() };
    assert_eq!(MachineLink::of(Some(&machine), true, Some(&seen()), Some(&up())), expected);
}

#[test]
fn the_gate_leaves_a_waiting_or_lost_link_as_it_is() {
    let older = Gate::Older(Older::Server(crate::cli::Version::parse("0.13.45")));
    assert_eq!(MachineLink::Waiting(None).gated(Gate::Open), MachineLink::Waiting(None));
    let waiting = MachineLink::Waiting(Some("vps.invalid".into()));
    assert_eq!(waiting.clone().gated(older), waiting);
    assert_eq!(MachineLink::Lost.gated(Gate::Open), MachineLink::Lost);
}

#[test]
fn a_refused_reach_only_waits_while_the_list_is_not_read() {
    let refused = Reach::Refused("vps is no longer listed".into());
    assert_eq!(MachineLink::of(None, false, Some(&seen()), Some(&refused)), MachineLink::Waiting(None));
}

#[test]
fn a_destination_not_known_when_made_is_never_another() {
    let up = MachineLink::Up { boot: "b".into(), destination: "vps.invalid".into() };
    assert!(up.stays_for(""));
    assert!(MachineLink::Waiting(Some("vps.invalid".into())).stays_for(""));
    assert!(MachineLink::Waiting(None).stays_for(""));
    assert!(!MachineLink::Lost.stays_for(""));
}

fn note() -> crate::store::OlderNote {
    crate::store::OlderNote {
        line: "vps runs an older cornercase".into(),
        tunnels: "vps is older: nothing new is tunnelled to vps until it is updated".into(),
        ..crate::store::OlderNote::default()
    }
}

fn older() -> MachineLink {
    MachineLink::Older("vps.invalid".into())
}

#[test]
fn the_link_for_each_reach() {
    let machine = listed();
    let linked = MachineLink::Up { boot: "boot-a".into(), destination: "vps.invalid".into() };
    let waiting = MachineLink::Waiting(Some("vps.invalid".into()));
    assert_eq!(link(Some(&machine), true, &up(), Gate::Open), linked);
    assert_eq!(link(Some(&machine), true, &Reach::Waiting("w".into()), Gate::Open), waiting);
    assert_eq!(link(Some(&machine), true, &Reach::Older(note()), Gate::Open), older());
    assert_eq!(link(Some(&machine), true, &Reach::Down("d".into()), Gate::Open), waiting);
    assert_eq!(link(Some(&machine), true, &Reach::Refused("r".into()), Gate::Open), MachineLink::Lost);
}

#[test]
fn a_refused_machine_keeps_what_it_holds_for_its_own_destination() {
    assert!(older().stays_for("vps.invalid"));
    assert!(older().stays_for(""));
    assert!(!older().stays_for("other.invalid"));
    let toward = Toward::Machine("vps".into());
    let hold = || test_hold(toward.clone(), 8123, Held::Up { local: 1 });
    let (ended, kept) = split_stale(vec![hold()], &toward, &older());
    assert_eq!((ended.len(), kept.len()), (0, 1));
    let (ended, kept) = split_stale(vec![hold()], &toward, &MachineLink::Older("other.invalid".into()));
    assert_eq!((ended.len(), kept.len()), (1, 0));
}

#[test]
fn a_refused_machine_loses_its_taker() {
    let up = MachineLink::Up { boot: "b".into(), destination: "vps.invalid".into() };
    assert!(!older().keeps_taker("vps.invalid"));
    assert!(up.keeps_taker("vps.invalid"));
    assert!(MachineLink::Waiting(Some("vps.invalid".into())).keeps_taker("vps.invalid"));
}

#[test]
fn a_side_known_older_still_ends_a_refused_machine() {
    let known = Gate::Older(Older::Server(Some(crate::cli::Version::new(0, 13, 45))));
    assert_eq!(older().gated(known), MachineLink::Lost);
    assert_eq!(older().gated(Gate::Open), older());
    assert_eq!(older().gated(Gate::Waiting), older());
}

#[test]
fn a_refused_machine_pauses_and_links_again_when_dropped() {
    let up = MachineLink::Up { boot: "b".into(), destination: "vps.invalid".into() };
    assert_eq!(linking(&older(), Some("b")), Linking::Pause);
    assert_eq!(linking(&up, None), Linking::Link { boot: "b", destination: "vps.invalid" });
    assert_eq!(linking(&up, Some("b")), Linking::Stay);
    assert_eq!(linking(&MachineLink::Lost, Some("b")), Linking::Lose);
}

#[test]
fn nothing_new_is_asked_of_a_refused_machine() {
    assert_eq!(tunnel_target(Reach::Older(note())), Err(note().tunnels));
    assert!(tunnel_target(Reach::Up(vps_target())).is_ok());
    assert_eq!(tunnel_target(Reach::Down("d".into())), Err("d".into()));
}

#[test]
fn a_waiting_reach_waits() {
    let machine = listed();
    let waiting = Reach::Waiting("waiting for the machines list".into());
    let expected = MachineLink::Waiting(Some("vps.invalid".into()));
    assert_eq!(link(Some(&machine), true, &waiting, Gate::Open), expected);
}
