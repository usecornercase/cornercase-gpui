use std::collections::HashSet;

use super::{appeared, origin, web_ports};
use crate::panels::ports::data::{List, Port};
use crate::test_util::fixture;

fn ports() -> Vec<Port> {
    web_ports(serde_json::from_str::<List>(fixture!("ports/web.json")).expect("the ports parse").ports)
}

fn numbers(ports: &[Port]) -> Vec<u16> {
    ports.iter().map(|p| p.number).collect()
}

#[test]
fn only_ports_with_a_url_are_kept_once_each_by_number() {
    assert_eq!(numbers(&ports()), [3000, 8000]);
}

#[test]
fn the_lowest_port_opens_when_the_servers_first_show() {
    let mut seen = HashSet::new();
    assert_eq!(appeared(&ports(), &mut seen, 22).as_deref(), Some("http://localhost:3000"));
}

#[test]
fn a_port_seen_before_never_opens_again() {
    let mut seen = HashSet::new();
    appeared(&ports(), &mut seen, 22);
    assert_eq!(appeared(&ports(), &mut seen, 22), None);
}

#[test]
fn a_port_that_appears_later_opens_once() {
    let mut all = ports();
    let late = all.pop().expect("two ports");
    let mut seen = HashSet::new();
    appeared(&all, &mut seen, 22);
    all.push(late);
    assert_eq!(appeared(&all, &mut seen, 22).as_deref(), Some("http://localhost:8000"));
    assert_eq!(appeared(&all, &mut seen, 22), None);
}

#[test]
fn a_port_seen_in_another_workspace_still_opens_in_this_one() {
    let mut seen = HashSet::new();
    appeared(&ports(), &mut seen, 22);
    assert_eq!(appeared(&ports(), &mut seen, 23).as_deref(), Some("http://localhost:3000"));
}

#[test]
fn the_origin_is_the_scheme_host_and_port() {
    assert_eq!(origin("http://localhost:3000/admin?tab=2"), "http://localhost:3000");
    assert_eq!(origin("http://localhost:3000"), "http://localhost:3000");
    assert_eq!(origin("about:blank"), "about:blank");
}
