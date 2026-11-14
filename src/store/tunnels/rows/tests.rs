use std::collections::BTreeMap;

use super::{
    Control, Tail, TunnelRow, Whose, count, elsewhere, machine_sections, mark, orders_notes, place_rows, row_control,
    section_tail,
};
use crate::cli::Version;
use crate::model::MachineSeen;
use crate::model::tunnels::{Gate, Older, Toward, TunnelSeen};
use crate::store::tunnels::held::{Held, Hold, test_hold};
use crate::store::tunnels::reverse::{ReverseHold, ReverseState};
use crate::store::{OlderNote, UpdateOffer, UpdateThere};

fn server_rows(listed: &[TunnelSeen], held: &[Hold], reverse: Option<&ReverseHold>) -> Vec<TunnelRow> {
    place_rows(&Toward::Server, listed, held, reverse)
}

fn ours(port: u16, state: Held) -> Hold {
    test_hold(Toward::Server, port, state)
}

fn there(name: &str, port: u16, state: Held) -> Hold {
    test_hold(Toward::Machine(name.to_string()), port, state)
}

fn seen(port: u16, local: Option<u16>, state: &str, kind: &str) -> TunnelSeen {
    TunnelSeen {
        port,
        local,
        state: state.to_string(),
        error: None,
        holder: 3,
        kind: kind.to_string(),
        direction: None,
    }
}

#[test]
fn the_count_is_what_this_window_holds() {
    let holds = [
        ours(1, Held::Opening),
        ours(2, Held::Up { local: 2 }),
        ours(3, Held::Down { why: "x".to_string() }),
        ours(4, Held::Ending),
    ];
    assert_eq!(count(&holds, 0), 3);
    assert_eq!(count(&[], 0), 0);
}

#[test]
fn the_menu_says_whose_each_tunnel_is() {
    let holds = [ours(5173, Held::Up { local: 5174 })];
    let listed = [
        seen(5173, Some(5174), "up", "hold"),
        seen(3000, Some(3000), "up", "window"),
        seen(8080, Some(8080), "up", "hold"),
        seen(9000, None, "closed", "window"),
    ];
    let texts: Vec<String> = server_rows(&listed, &holds, None).iter().map(TunnelRow::text).collect();
    assert_eq!(
        texts,
        [
            "⇄ 3000 · in another window",
            "⇄ 5173 → localhost:5174 · up",
            "⇄ 8080 · in another program",
            "⇄ 9000 · closed in another window",
        ]
    );
}

#[test]
fn this_windows_hold_is_told_by_its_port_and_local_port() {
    let holds = [ours(5173, Held::Up { local: 5174 })];
    let rows = server_rows(&[seen(5173, Some(6000), "up", "hold")], &holds, None);
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().any(|row| row.whose == Whose::Program));
}

#[test]
fn a_hold_not_listed_yet_still_shows_as_opening() {
    let rows = server_rows(&[], &[ours(5173, Held::Opening)], None);
    assert_eq!(rows.iter().map(TunnelRow::text).collect::<Vec<_>>(), ["⇄ 5173 · opening"]);
    let down = server_rows(&[], &[ours(5173, Held::Down { why: "ssh stopped".to_string() })], None);
    assert_eq!(down[0].text(), "⇄ 5173 · down");
}

#[test]
fn a_port_row_offers_tunnel_then_close_tunnel() {
    let state = |held: Option<&Held>, there: bool| (row_control(true, held), mark(true, held, there).map(|m| m.text));
    assert_eq!(state(None, false), (Control::Tunnel, None));
    assert_eq!(state(Some(&Held::Opening), false), (Control::Opening, Some("⇄ …".to_string())));
    assert_eq!(state(Some(&Held::Up { local: 5174 }), false), (Control::Close, Some("⇄ :5174".to_string())));
    let down = Held::Down { why: String::new() };
    assert_eq!(state(Some(&down), false), (Control::Close, Some("⇄ down".to_string())));
    assert_eq!(state(None, true), (Control::Tunnel, Some("⇄ elsewhere".to_string())));
    assert_eq!(state(Some(&Held::Ending), false), (Control::Closing, None));
    assert_eq!((row_control(false, None), mark(false, None, true)), (Control::Hidden, None));
}

#[test]
fn a_machine_that_cannot_be_reached_keeps_close_on_its_hold() {
    let up = Held::Up { local: 63235 };
    assert_eq!(row_control(false, Some(&up)), Control::Close);
    assert_eq!(mark(false, Some(&up), false).map(|m| m.text), Some("⇄ :63235".to_string()));
    assert_eq!(row_control(false, Some(&Held::Opening)), Control::Opening);
}

#[test]
fn the_machines_tunnels_are_listed_by_machine_then_port() {
    let holds = [
        there("vps", 8765, Held::Up { local: 63235 }),
        ours(5173, Held::Opening),
        there("cap", 5173, Held::Down { why: "x".to_string() }),
        there("vps", 3000, Held::Opening),
        there("vps", 4000, Held::Ending),
    ];
    let sections = machine_sections(&holds, &BTreeMap::new(), &[]);
    let names: Vec<&str> = sections.iter().map(|section| section.name.as_str()).collect();
    assert_eq!(names, ["cap", "vps"]);
    let vps: Vec<String> = sections[1].rows.iter().map(TunnelRow::text).collect();
    assert_eq!(vps, ["⇄ 3000 · opening", "⇄ 8765 → localhost:63235 · up"]);
}

#[test]
fn the_count_takes_both_kinds_and_the_server_menu_only_its_own() {
    let holds = [ours(1, Held::Opening), there("vps", 1, Held::Opening)];
    assert_eq!(count(&holds, 0), 2);
    let rows = server_rows(&[], &holds, None);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].toward, Toward::Server);
}

#[test]
fn a_machine_holding_the_same_port_is_not_this_servers_tunnel() {
    let holds = [there("vps", 5173, Held::Up { local: 5173 })];
    let listed = [seen(5173, Some(5173), "up", "hold")];
    assert!(elsewhere(&Toward::Server, 5173, &listed, &holds));
}

#[test]
fn another_holder_is_seen_as_elsewhere_only_when_open() {
    let listed = [seen(1, None, "closed", "window"), seen(2, Some(2), "up", "window")];
    assert!(!elsewhere(&Toward::Server, 1, &listed, &[]));
    assert!(elsewhere(&Toward::Server, 2, &listed, &[]));
    assert!(!elsewhere(&Toward::Server, 2, &[seen(2, Some(3), "up", "hold")], &[ours(2, Held::Up { local: 3 })]));
}

#[test]
fn the_note_for_no_tunnel_names_the_keys_of_this_platform() {
    assert!(super::none_note(Some("Ctrl+Shift+O")).contains("in Ports (Ctrl+Shift+O),"));
    assert!(super::none_note(None).contains("in Ports,"));
}

#[test]
fn a_long_destination_is_cut_in_the_menus_heading() {
    assert_eq!(super::heading("devbox"), "Tunnels to devbox");
    let long = super::heading("builder@ci-runner-07.eu-west-1.compute.example.invalid");
    assert_eq!(long, "Tunnels to builder@ci-runner-07.eu-west…");
}

fn gate_of(name: &str, gate: Gate) -> (String, Gate) {
    (name.to_string(), gate)
}

#[test]
fn no_line_when_both_sides_take_orders() {
    let lines = orders_notes(&[gate_of("vps", Gate::Open), gate_of("lab", Gate::Waiting)]);
    assert_eq!(lines.len(), 0);
}

#[test]
fn this_computer_older_is_one_line_for_every_machine() {
    let older = Gate::Older(Older::ThisComputer(Version::parse("0.13.56")));
    let lines = orders_notes(&[gate_of("vps", older), gate_of("lab", older)]);
    let [line] = lines.as_slice() else { panic!("one line") };
    assert!(line.starts_with("A program on vps and lab cannot ask this window"), "{line}");
    assert!(line.ends_with("(it has 0.13.56)."), "{line}");
}

#[test]
fn a_machines_older_server_is_its_own_line() {
    let older = Gate::Older(Older::Server(Version::parse("0.13.56")));
    let lines = orders_notes(&[gate_of("vps", older), gate_of("lab", Gate::Open)]);
    let [line] = lines.as_slice() else { panic!("one line") };
    assert!(line.starts_with("A program on vps cannot ask") && line.ends_with("on vps (its server runs 0.13.56)."));
}

fn reverse_seen(port: u16) -> TunnelSeen {
    let line: serde_json::Value =
        serde_json::from_str(crate::test_util::fixture!("tunnels/view_line_reverse.json")).expect("the line parses");
    let mut found: TunnelSeen = serde_json::from_value(line["tunnels"][0].clone()).expect("a tunnel entry");
    found.port = port;
    found
}

fn reversing(state: ReverseState) -> ReverseHold {
    ReverseHold { destination: "vps.invalid".to_string(), label: "vps".to_string(), process: 9, state, job: None }
}

#[test]
fn a_reverse_hold_another_program_holds_is_no_row() {
    let rows = server_rows(&[reverse_seen(41234)], &[], None);
    assert_eq!(rows.len(), 0, "{rows:?}");
}

#[test]
fn a_reverse_hold_is_never_seen_as_elsewhere() {
    assert!(!elsewhere(&Toward::Server, 41234, &[reverse_seen(41234)], &[]));
}

#[test]
fn a_reverse_hold_on_the_port_of_this_windows_hold_is_not_taken_for_it() {
    let holds = [ours(41234, Held::Opening)];
    let rows = server_rows(&[reverse_seen(41234)], &holds, None);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].whose, Whose::Opening);
    let mut mixed = reverse_seen(41234);
    mixed.direction = None;
    mixed.kind = "hold".to_string();
    mixed.local = Some(41234);
    let own = [ours(41234, Held::Up { local: 41234 })];
    assert_eq!(server_rows(&[mixed], &own, None).len(), 1);
}

#[test]
fn this_windows_reverse_hold_is_its_own_row_and_counts() {
    let open = reversing(ReverseState::Open { there: 41234 });
    let rows = server_rows(&[], &[], Some(&open));
    assert_eq!(rows[0].text(), "⇄ 41234 ← this Chrome · open");
    assert!(rows[0].ours());
    let opening = reversing(ReverseState::Opening);
    assert_eq!(server_rows(&[], &[], Some(&opening))[0].text(), "⇄ this Chrome · opening");
    assert_eq!(count(&[ours(1, Held::Opening)], 1), 2);
}

#[test]
fn a_machine_with_only_a_reverse_hold_has_its_section() {
    let reverses =
        BTreeMap::from([(Toward::Machine("vps".to_string()), reversing(ReverseState::Open { there: 41234 }))]);
    let only = machine_sections(&[], &reverses, &[]);
    assert_eq!(only.len(), 1);
    let holds = [there("vps", 8765, Held::Up { local: 63235 })];
    let both = machine_sections(&holds, &reverses, &[]);
    let texts: Vec<String> = both[0].rows.iter().map(TunnelRow::text).collect();
    assert_eq!(texts, ["⇄ 8765 → localhost:63235 · up", "⇄ 41234 ← this Chrome · open"]);
}

fn refused(machine: &str, release: &str) -> OlderNote {
    OlderNote {
        tunnels: format!("{machine} runs {release}: nothing new is tunnelled to {machine}"),
        ..OlderNote::default()
    }
}

fn update(machine: &str) -> UpdateThere {
    UpdateThere { machine: machine.into(), destination: format!("{machine}.invalid"), args: Vec::new() }
}

#[test]
fn a_refused_machine_has_one_line_and_its_update_under_its_tunnels() {
    let note = refused("vps", "0.13.39");
    let tail = section_tail(Some(&note), &UpdateOffer::Ask(update("vps")));
    assert_eq!(tail, vec![Tail::Note(note.tunnels.clone()), Tail::Update(update("vps"))]);
}

#[test]
fn two_refused_machines_each_say_their_own_releases() {
    let (a, b) = (refused("vps", "0.13.39"), refused("box", "0.13.40"));
    let lines = |note: &OlderNote| section_tail(Some(note), &UpdateOffer::Hidden);
    assert_eq!(lines(&a), vec![Tail::Note(a.tunnels.clone())]);
    assert_ne!(lines(&a), lines(&b));
}

#[test]
fn a_refused_machine_updating_rests_its_entry() {
    let note = refused("vps", "0.13.39");
    let tail = section_tail(Some(&note), &UpdateOffer::Running("Updating cornercase on vps…".into()));
    assert_eq!(tail, vec![Tail::Note(note.tunnels), Tail::Updating("Updating cornercase on vps…".into())]);
}

#[test]
fn a_refusal_without_update_words_is_only_its_line() {
    let note = refused("vps", "0.13.39");
    assert_eq!(section_tail(Some(&note), &UpdateOffer::Hidden), vec![Tail::Note(note.tunnels)]);
}

#[test]
fn a_machine_that_answers_adds_nothing() {
    assert_eq!(section_tail(None, &UpdateOffer::Ask(update("vps"))), Vec::new());
}

fn vps_entry() -> MachineSeen {
    serde_json::from_str(crate::test_util::fixture!("machines/seen_tunnels.json")).expect("an entry")
}

fn down_entry() -> MachineSeen {
    let mut seen = vps_entry();
    seen.state = crate::model::machines::LinkState::Unreachable;
    seen
}

fn texts(rows: &[TunnelRow]) -> Vec<String> {
    rows.iter().map(TunnelRow::text).collect()
}

fn sections_of(holds: &[Hold], seen: &MachineSeen) -> Vec<super::TunnelSection> {
    machine_sections(holds, &BTreeMap::new(), std::slice::from_ref(seen))
}

#[test]
fn a_machines_tunnels_held_elsewhere_are_listed_under_it() {
    let sections = sections_of(&[], &vps_entry());
    assert_eq!(sections.len(), 1);
    assert_eq!(
        texts(&sections[0].rows),
        [
            "⇄ 5173 · in another window",
            "⇄ 8765 · in another program",
            "⇄ 8765 · in another program",
            "⇄ 9000 · closed in another window"
        ]
    );
}

#[test]
fn this_windows_hold_to_a_machine_is_listed_once() {
    let holds = [there("vps", 8765, Held::Up { local: 60950 })];
    let sections = sections_of(&holds, &vps_entry());
    let on_8765: Vec<&TunnelRow> = sections[0].rows.iter().filter(|row| row.port == 8765).collect();
    assert_eq!(on_8765.len(), 2);
    assert!(matches!(on_8765[0].whose, Whose::Up { local: 60950 }));
    assert_eq!(on_8765[1].whose, Whose::Program);
}

#[test]
fn another_programs_reverse_hold_on_a_machine_is_no_row_and_no_mark() {
    let sections = sections_of(&[], &vps_entry());
    assert!(sections[0].rows.iter().all(|row| row.port != 60971));
    let toward = Toward::Machine("vps".to_string());
    assert!(!elsewhere(&toward, 60971, &vps_entry().tunnels, &[]));
}

#[test]
fn a_machine_listing_only_others_tunnels_has_its_section() {
    let mut seen = vps_entry();
    seen.tunnels.truncate(1);
    let sections = sections_of(&[], &seen);
    assert_eq!(sections.iter().map(|section| section.name.as_str()).collect::<Vec<_>>(), ["vps"]);
    assert_eq!(sections_of(&[], &MachineSeen { name: "cap".into(), ..MachineSeen::default() }).len(), 0);
}

#[test]
fn others_tunnels_on_a_machine_are_not_counted() {
    let holds = [there("vps", 8765, Held::Up { local: 60950 })];
    let rows = sections_of(&holds, &vps_entry());
    assert!(rows[0].rows.len() > 1);
    assert_eq!(count(&holds, 0), 1);
}

#[test]
fn a_machine_that_is_down_keeps_its_last_tunnels_as_resting_rows() {
    let holds = [there("vps", 8765, Held::Up { local: 60950 })];
    let sections = sections_of(&holds, &down_entry());
    let section = &sections[0];
    assert_eq!(
        section.last_sent.as_deref(),
        Some("vps unreachable: the tunnels other windows or programs hold to it are as it last sent them")
    );
    let own = section.rows.iter().find(|row| row.ours()).expect("this window's row");
    let other = section.rows.iter().find(|row| !row.ours()).expect("another holder's row");
    assert!(section.rests(other));
    assert!(!section.rests(own));
}

#[test]
fn a_linked_machines_rows_never_rest() {
    let sections = sections_of(&[], &vps_entry());
    assert_eq!(sections[0].last_sent, None);
    assert!(sections[0].rows.iter().all(|row| !sections[0].rests(row)));
}

#[test]
fn a_down_machine_with_only_this_windows_rows_has_no_note() {
    let mut seen = down_entry();
    seen.tunnels.clear();
    let holds = [there("vps", 8765, Held::Up { local: 60950 })];
    assert_eq!(sections_of(&holds, &seen)[0].last_sent, None);
}

#[test]
fn a_port_another_program_holds_on_a_machine_is_elsewhere() {
    let toward = Toward::Machine("vps".to_string());
    let listed = vps_entry().tunnels;
    assert!(elsewhere(&toward, 5173, &listed, &[]));
    assert!(!elsewhere(&toward, 9000, &listed, &[]));
    assert!(!elsewhere(&toward, 4000, &listed, &[]));
}

#[test]
fn a_hold_to_the_server_is_not_a_machines_own_hold() {
    let toward = Toward::Machine("vps".to_string());
    let listed = [seen(5173, Some(5173), "up", "hold")];
    let holds = [ours(5173, Held::Up { local: 5173 })];
    assert!(elsewhere(&toward, 5173, &listed, &holds));
}
