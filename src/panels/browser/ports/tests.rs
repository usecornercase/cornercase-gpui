use std::collections::HashSet;

use super::reused_tab;
use super::{appeared, origin, same_page, web_ports};
use crate::model::Owner;
use crate::panels::browser::tabs::{Tab, Through};
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
    assert_eq!(appeared(&ports(), &mut seen, &Owner::Here(22)).as_deref(), Some("http://localhost:3000"));
}

#[test]
fn a_port_seen_before_never_opens_again() {
    let mut seen = HashSet::new();
    appeared(&ports(), &mut seen, &Owner::Here(22));
    assert_eq!(appeared(&ports(), &mut seen, &Owner::Here(22)), None);
}

#[test]
fn a_port_that_appears_later_opens_once() {
    let mut all = ports();
    let late = all.pop().expect("two ports");
    let mut seen = HashSet::new();
    appeared(&all, &mut seen, &Owner::Here(22));
    all.push(late);
    assert_eq!(appeared(&all, &mut seen, &Owner::Here(22)).as_deref(), Some("http://localhost:8000"));
    assert_eq!(appeared(&all, &mut seen, &Owner::Here(22)), None);
}

#[test]
fn a_port_seen_in_another_workspace_still_opens_in_this_one() {
    let mut seen = HashSet::new();
    appeared(&ports(), &mut seen, &Owner::Here(22));
    assert_eq!(appeared(&ports(), &mut seen, &Owner::Here(23)).as_deref(), Some("http://localhost:3000"));
}

#[test]
fn the_origin_is_the_scheme_host_and_port() {
    assert_eq!(origin("http://localhost:3000/admin?tab=2"), "http://localhost:3000");
    assert_eq!(origin("http://localhost:3000"), "http://localhost:3000");
    assert_eq!(origin("about:blank"), "about:blank");
}

#[test]
fn an_address_without_a_path_comes_forward_on_any_page_of_its_origin() {
    assert!(same_page("http://localhost:3000/admin", "http://localhost:3000"));
    assert!(same_page("http://localhost:3000/", "http://localhost:3000/"));
    assert!(!same_page("http://localhost:8000/", "http://localhost:3000"));
}

#[test]
fn an_address_with_a_path_comes_forward_only_on_that_page() {
    assert!(same_page("https://github.com/a/b/pull/12", "https://github.com/a/b/pull/12"));
    assert!(!same_page("https://github.com/a/b", "https://github.com/a/b/pull/12"));
    assert!(!same_page("http://localhost:3000/", "http://localhost:3000/?debug=1"));
}

fn tab(target: &str, url: &str, through: Option<Through>) -> Tab {
    let mut tab = Tab::new(target, url, Owner::Here(1));
    tab.through = through;
    tab
}

#[test]
fn reopening_a_closed_tunnel_reuses_the_tab_of_its_old_hold() {
    let old = Through { remote: 8765, local: 61000 };
    let new = Through { remote: 8765, local: 62000 };
    let tabs = [tab("T", "http://localhost:61000/", Some(old))];
    let found = reused_tab(&tabs, &Owner::Here(1), "http://localhost:62000", Some(new));
    assert_eq!(found, Some(("T".to_string(), false)));
    assert_eq!(reused_tab(&tabs, &Owner::Here(2), "http://localhost:62000", Some(new)), None);
    assert_eq!(reused_tab(&tabs, &Owner::Here(1), "http://localhost:62000", None), None);
    let other = Through { remote: 9000, local: 62000 };
    assert_eq!(reused_tab(&tabs, &Owner::Here(1), "http://localhost:62000", Some(other)), None);
}

#[test]
fn a_machines_ports_never_feed_the_own_tunnels() {
    use crate::model::ThereId;
    use crate::panels::ports::Listed;
    let there: Port<ThereId> = Port { number: 8765, url: Some("http://localhost:8765".into()), ..Port::default() };
    assert_eq!(super::here_numbers(&Listed::There(vec![there])), Vec::<u16>::new());
    assert_eq!(super::here_numbers(&Listed::Here(ports())), [3000, 8000]);
}
