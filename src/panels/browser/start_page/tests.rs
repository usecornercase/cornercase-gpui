use super::{Reach, held_reach, how_reached, reach, shown_url};
use crate::panels::browser::tunnels::{State, Tunnel};
use crate::panels::ports::data::Port;
use crate::store::Held;

fn port(number: u16) -> Port {
    Port { number, url: Some(format!("http://localhost:{number}")), ..Port::default() }
}

#[test]
fn on_this_mac_a_port_opens_at_its_own_url() {
    assert!(matches!(reach(&port(3000), None, false), Reach::Direct(url) if url == "http://localhost:3000"));
}

#[test]
fn on_another_machine_a_port_opens_at_its_tunnel() {
    let tunnel = Tunnel::at(3000, 61234, State::Ready);
    let reached = reach(&port(3000), Some(&tunnel), true);
    assert!(matches!(reached, Reach::Tunnelled { ref url, .. } if url == "http://localhost:61234"));
}

#[test]
fn a_port_without_a_tunnel_yet_waits_for_one() {
    assert!(matches!(reach(&port(3000), None, true), Reach::Opening));
}

#[test]
fn a_tunnelled_port_says_which_machine_it_comes_from() {
    let same = reach(&port(3000), Some(&Tunnel::at(3000, 3000, State::Ready)), true);
    assert_eq!(how_reached(&port(3000), &same, "devbox"), "via devbox");
    let moved = reach(&port(3000), Some(&Tunnel::at(3000, 61234, State::Ready)), true);
    assert_eq!(how_reached(&port(3000), &moved, "devbox"), "via devbox, port 3000 there");
}

#[test]
fn a_lost_tunnel_says_it_is_tried_again() {
    let lost = reach(&port(3000), Some(&Tunnel::at(3000, 3000, State::Retrying)), true);
    assert_eq!(how_reached(&port(3000), &lost, "devbox"), "via devbox: the tunnel stopped, trying again");
}

#[test]
fn a_local_port_needs_no_note() {
    assert_eq!(how_reached(&port(3000), &reach(&port(3000), None, false), "devbox"), "");
}

#[test]
fn the_listed_address_drops_its_scheme() {
    assert_eq!(shown_url("http://localhost:3000/a"), "localhost:3000/a");
}

#[test]
fn a_port_not_tunnelled_says_where_it_is_and_that_open_tunnels_it() {
    let reached = held_reach(&port(3000), None);
    assert!(matches!(reached, Reach::NotTunnelled));
    assert_eq!(how_reached(&port(3000), &reached, "devbox"), "port 3000 on devbox");
}

#[test]
fn a_held_tunnel_reads_as_tunnelled() {
    let up = held_reach(&port(3000), Some(&Held::Up { local: 61234 }));
    assert!(matches!(up, Reach::Tunnelled { ref url, state: State::Ready } if url == "http://localhost:61234"));
    assert_eq!(how_reached(&port(3000), &up, "devbox"), "via devbox, port 3000 there");
    let down = held_reach(&port(3000), Some(&Held::Down { why: String::new() }));
    assert_eq!(how_reached(&port(3000), &down, "devbox"), "via devbox: the tunnel stopped, trying again");
    assert!(matches!(held_reach(&port(3000), Some(&Held::Opening)), Reach::Opening));
}

#[test]
fn a_tunnel_being_closed_says_so() {
    let closing = held_reach(&port(3000), Some(&Held::Ending));
    assert_eq!(how_reached(&port(3000), &closing, "devbox"), "closing the tunnel of port 3000 on devbox");
}
