use super::{
    Gate, Older, Release, TUNNEL_HOLD_SINCE, TUNNEL_ORDERS_SINCE, Toward, Way, gate_note, held_line, machine_gate,
    older_note, orders_gate, orders_note, way,
};
use crate::cli::Version;
use crate::model::Session;
use crate::test_util::fixture;

const VIEW_LINE: &str = fixture!("tunnels/view_line.json");
const HELD_LINE: &str = fixture!("tunnels/held_line.txt");
const SESSION: &str = fixture!("session/held.json");

fn release(text: &str) -> Release {
    Release::Read(Version::parse(text))
}

#[test]
fn a_view_line_lists_the_tunnels_of_its_server() {
    let session: Session = serde_json::from_str(VIEW_LINE).expect("a view line");
    let [tunnel] = session.tunnels.as_slice() else { panic!("one tunnel") };
    assert_eq!((tunnel.port, tunnel.local, tunnel.state.as_str()), (8765, Some(65433), "up"));
    assert_eq!((tunnel.kind.as_str(), tunnel.holder, tunnel.error.as_deref()), ("hold", 5, None));
}

#[test]
fn a_view_line_without_tunnels_lists_none() {
    let session: Session = serde_json::from_str(SESSION).expect("a session");
    assert_eq!(session.tunnels.len(), 0);
}

#[test]
fn the_held_line_gives_the_local_port() {
    let held = held_line(HELD_LINE).expect("a held line");
    assert_eq!((held.port, held.local), (8765, 65433));
}

#[test]
fn another_line_is_no_held_line() {
    assert_eq!(held_line("Error: could not tunnel port 5173"), None);
    assert_eq!(held_line("{\"port\":5173"), None);
    assert_eq!(held_line("{\"port\":0,\"local\":5173}"), None);
    assert_eq!(held_line("{\"port\":5173,\"local\":0}"), None);
}

#[test]
fn the_window_on_this_computer_tunnels_nothing() {
    assert_eq!(way(None, release("0.13.50"), release("0.13.50")), Way::Here);
}

#[test]
fn holds_need_0_13_50_here_and_on_the_server() {
    let on = |here, server| way(Some("devbox"), release(here), release(server));
    assert_eq!(on("0.13.50", "0.13.50"), Way::Cornercase);
    assert_eq!(on("0.13.51", "0.13.50"), Way::Cornercase);
    assert_eq!(on("0.13.39", "0.13.50"), Way::Own(Older::ThisComputer(Version::parse("0.13.39"))));
    assert_eq!(on("0.13.50", "0.13.48"), Way::Own(Older::Server(Version::parse("0.13.48"))));
    assert_eq!(way(Some("devbox"), Release::Read(None), release("0.13.50")), Way::Own(Older::ThisComputer(None)));
    assert_eq!(way(Some("devbox"), release("0.13.50"), Release::Read(None)), Way::Own(Older::Server(None)));
    assert_eq!(TUNNEL_HOLD_SINCE, Version::new(0, 13, 50));
}

#[test]
fn nothing_starts_before_this_computers_release_is_read() {
    assert_eq!(way(Some("devbox"), Release::Unread, release("0.13.50")), Way::Waiting);
    assert_eq!(way(Some("devbox"), release("0.13.50"), Release::Unread), Way::Waiting);
}

#[test]
fn the_older_note_names_the_side_to_update() {
    let here = older_note(&Older::ThisComputer(Version::parse("0.13.39")), "devbox");
    assert!(here.contains("cornercase 0.13.50 or newer on this "), "{here}");
    assert!(here.ends_with("(it has 0.13.39)."), "{here}");
    let server = older_note(&Older::Server(Version::parse("0.13.48")), "devbox");
    assert!(server.contains("on devbox lists them") && server.ends_with("(its server runs 0.13.48)."), "{server}");
    assert!(!older_note(&Older::Server(None), "devbox").contains('('));
}

fn version(text: &str) -> Option<Version> {
    Version::parse(text)
}

#[test]
fn a_machines_tunnel_needs_both_releases() {
    assert_eq!(machine_gate(Release::Unread, version("0.13.54")), Gate::Waiting);
    let here_old = Release::Read(version("0.13.39"));
    assert_eq!(machine_gate(here_old, version("0.13.54")), Gate::Older(Older::ThisComputer(version("0.13.39"))));
    assert_eq!(machine_gate(Release::Read(None), version("0.13.54")), Gate::Older(Older::ThisComputer(None)));
    let new = release("0.13.54");
    assert_eq!(machine_gate(new, version("0.13.45")), Gate::Older(Older::Server(version("0.13.45"))));
    assert_eq!(machine_gate(new, None), Gate::Older(Older::Server(None)));
    assert_eq!(machine_gate(here_old, version("0.13.45")), Gate::Older(Older::ThisComputer(version("0.13.39"))));
}

#[test]
fn a_machines_tunnel_opens_at_exactly_0_13_50_on_both() {
    assert_eq!(machine_gate(release("0.13.50"), version("0.13.50")), Gate::Open);
    assert_eq!(machine_gate(release("0.13.54"), version("0.14.0")), Gate::Open);
    assert!(matches!(machine_gate(release("0.13.49"), version("0.13.50")), Gate::Older(_)));
    assert!(matches!(machine_gate(release("0.13.50"), version("0.13.49")), Gate::Older(_)));
}

#[test]
fn the_gate_note_names_the_side_and_the_machine() {
    let server = gate_note(&Older::Server(version("0.13.45")), "vps");
    assert_eq!(server, "Tunnel needs cornercase 0.13.50 or newer on vps (its server runs 0.13.45).");
    let here = gate_note(&Older::ThisComputer(version("0.13.39")), "vps");
    assert!(
        here.starts_with("Tunnel needs cornercase 0.13.50 or newer on this ") && here.ends_with("(it has 0.13.39).")
    );
    assert_eq!(gate_note(&Older::Server(None), "vps"), "Tunnel needs cornercase 0.13.50 or newer on vps.");
}

#[test]
fn a_toward_has_a_key_and_a_machine_only_for_a_machine() {
    assert_eq!(Toward::Server.key(), "server");
    assert_eq!(Toward::Machine("vps".into()).key(), "machine:vps");
    assert_eq!((Toward::Server.machine(), Toward::Machine("vps".into()).machine()), (None, Some("vps")));
}

fn older_this(text: &str) -> Gate {
    Gate::Older(Older::ThisComputer(Version::parse(text)))
}

fn older_server(text: &str) -> Gate {
    Gate::Older(Older::Server(Version::parse(text)))
}

#[test]
fn orders_need_0_13_57_here_and_on_the_server() {
    let gate = |here: Release, server: &str| orders_gate(here, Version::parse(server));
    assert_eq!(gate(release("0.13.57"), "0.13.57"), Gate::Open);
    assert_eq!(gate(release("0.14.0"), "0.13.58"), Gate::Open);
    assert_eq!(gate(release("0.13.56"), "0.13.57"), older_this("0.13.56"));
    assert_eq!(gate(release("0.13.39"), "0.13.57"), older_this("0.13.39"));
    assert_eq!(gate(release("0.13.57"), "0.13.56"), older_server("0.13.56"));
    assert_eq!(orders_gate(release("0.13.57"), None), Gate::Older(Older::Server(None)));
    assert_eq!(orders_gate(Release::Read(None), Version::parse("0.13.57")), Gate::Older(Older::ThisComputer(None)));
}

#[test]
fn orders_wait_for_this_computers_release() {
    assert_eq!(orders_gate(Release::Unread, Version::parse("0.13.57")), Gate::Waiting);
}

#[test]
fn holds_open_below_orders_from_0_13_50() {
    for minor in 50..=56 {
        let text = format!("0.13.{minor}");
        assert_eq!(machine_gate(release(&text), Version::parse(&text)), Gate::Open, "{text}");
        assert!(matches!(orders_gate(release(&text), Version::parse(&text)), Gate::Older(_)), "{text}");
    }
}

#[test]
fn the_orders_note_names_the_side_that_needs_0_13_57() {
    let here = orders_note(&Older::ThisComputer(Version::parse("0.13.56")), "vps");
    let prefix = "A program on vps cannot ask this window for a tunnel: it needs cornercase 0.13.57 or newer on this ";
    assert!(here.starts_with(prefix) && here.ends_with(" (it has 0.13.56)."), "{here}");
    let server = orders_note(&Older::Server(Version::parse("0.13.56")), "vps");
    let words = "A program on vps cannot ask this window for a tunnel: it needs cornercase 0.13.57 or newer on vps";
    assert_eq!(server, format!("{words} (its server runs 0.13.56)."));
    assert_eq!(orders_note(&Older::Server(None), "vps"), format!("{words}."));
    assert!(!here.contains('#') && !server.contains('#'));
    assert_eq!(TUNNEL_ORDERS_SINCE, Version::new(0, 13, 57));
}
