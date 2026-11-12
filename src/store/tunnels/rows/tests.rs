use super::{Control, Whose, count, elsewhere, mark, menu_rows, row_control};
use crate::model::tunnels::{TunnelSeen, Way};
use crate::store::tunnels::held::{Asked, Held, Hold};

fn ours(port: u16, state: Held) -> Hold {
    let mut hold = Hold::opening(port, 1, Asked::Click, "boot");
    hold.state = state;
    hold
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
    let way = Way::Cornercase;
    let state = |held: Option<&Held>, there: bool| (row_control(&way, held), mark(&way, held, there).map(|m| m.text));
    assert_eq!(state(None, false), (Control::Tunnel, None));
    assert_eq!(state(Some(&Held::Opening), false), (Control::Opening, Some("⇄ …".to_string())));
    assert_eq!(state(Some(&Held::Up { local: 5174 }), false), (Control::Close, Some("⇄ :5174".to_string())));
    let down = Held::Down { why: String::new() };
    assert_eq!(state(Some(&down), false), (Control::Close, Some("⇄ down".to_string())));
    assert_eq!(state(None, true), (Control::Tunnel, Some("⇄ elsewhere".to_string())));
    assert_eq!(state(Some(&Held::Ending), false), (Control::Closing, None));
    for way in [Way::Here, Way::Waiting] {
        assert_eq!((row_control(&way, None), mark(&way, None, true)), (Control::Hidden, None));
    }
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
