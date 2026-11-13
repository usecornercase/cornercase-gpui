use super::{Control, Whose, count, elsewhere, machine_sections, mark, menu_rows, others_note, row_control};
use crate::model::tunnels::{Toward, TunnelSeen};
use crate::store::tunnels::held::{Held, Hold, test_hold};

fn ours(port: u16, state: Held) -> Hold {
    test_hold(Toward::Server, port, state)
}

fn there(name: &str, port: u16, state: Held) -> Hold {
    test_hold(Toward::Machine(name.to_string()), port, state)
}

fn seen(port: u16, local: Option<u16>, state: &str, kind: &str) -> TunnelSeen {
    TunnelSeen { port, local, state: state.to_string(), error: None, holder: 3, kind: kind.to_string() }
}

#[test]
fn the_count_is_what_this_window_holds() {
    let holds = [
        ours(1, Held::Opening),
        ours(2, Held::Up { local: 2 }),
        ours(3, Held::Down { why: "x".to_string() }),
        ours(4, Held::Ending),
    ];
    assert_eq!(count(&holds), 3);
    assert_eq!(count(&[]), 0);
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
    let texts: Vec<String> = menu_rows(&listed, &holds).iter().map(super::TunnelRow::text).collect();
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
    let rows = menu_rows(&[seen(5173, Some(6000), "up", "hold")], &holds);
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().any(|row| row.whose == Whose::Program));
}

#[test]
fn a_hold_not_listed_yet_still_shows_as_opening() {
    let rows = menu_rows(&[], &[ours(5173, Held::Opening)]);
    assert_eq!(rows.iter().map(super::TunnelRow::text).collect::<Vec<_>>(), ["⇄ 5173 · opening"]);
    let down = menu_rows(&[], &[ours(5173, Held::Down { why: "ssh stopped".to_string() })]);
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
    let sections = machine_sections(&holds);
    let names: Vec<&str> = sections.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["cap", "vps"]);
    let vps: Vec<String> = sections[1].1.iter().map(super::TunnelRow::text).collect();
    assert_eq!(vps, ["⇄ 3000 · opening", "⇄ 8765 → localhost:63235 · up"]);
}

#[test]
fn the_count_takes_both_kinds_and_the_server_menu_only_its_own() {
    let holds = [ours(1, Held::Opening), there("vps", 1, Held::Opening)];
    assert_eq!(count(&holds), 2);
    let rows = menu_rows(&[], &holds);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].toward, Toward::Server);
}

#[test]
fn a_machine_holding_the_same_port_is_not_this_servers_tunnel() {
    let holds = [there("vps", 5173, Held::Up { local: 5173 })];
    let listed = [seen(5173, Some(5173), "up", "hold")];
    assert!(elsewhere(5173, &listed, &holds));
}

#[test]
fn the_other_holders_note_names_the_machines_and_the_issue() {
    let note = others_note("vps, cap");
    assert!(note.contains("vps, cap") && note.contains("usecornercase/cornercase-terminal#787"), "{note}");
}

#[test]
fn another_holder_is_seen_as_elsewhere_only_when_open() {
    let listed = [seen(1, None, "closed", "window"), seen(2, Some(2), "up", "window")];
    assert!(!elsewhere(1, &listed, &[]));
    assert!(elsewhere(2, &listed, &[]));
    assert!(!elsewhere(2, &[seen(2, Some(3), "up", "hold")], &[ours(2, Held::Up { local: 3 })]));
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
