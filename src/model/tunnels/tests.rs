use super::{Older, Release, TUNNEL_HOLD_SINCE, Way, held_line, older_note, way};
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
