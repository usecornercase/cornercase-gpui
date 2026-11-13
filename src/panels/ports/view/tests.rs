use std::collections::HashSet;

use super::*;
use crate::model::Session;
use crate::panels::ports::data::{List, Port};
use crate::test_util::fixture;

fn ports() -> Vec<Port> {
    serde_json::from_str::<List>(fixture!("ports/workspace.json")).expect("the ports parse").ports
}

fn view(port: &Port) -> PortView {
    view_here(port, None, false)
}

#[test]
fn stopping_names_the_port_and_the_pid() {
    assert_eq!(view(&ports()[0]).stop_command(), ["ports", "stop", "61440", "--pid", "52064"]);
}

#[test]
fn the_stop_warning_names_an_agent_and_a_program_no_tab_started() {
    let p = Port { via: Some("agent".into()), pane: None, program: "vite".into(), ..Port::default() };
    let text = stop_warning(&view(&p), None);
    assert!(text.contains("An agent started it"));
    assert!(text.contains("No tab started it"));
}

fn port(number: u16, workspace: Option<u64>) -> Port {
    Port { number, pid: u64::from(number) + 1, program: "vite".into(), workspace, pane: Some(4), ..Port::default() }
}

fn views(ports: &[Port]) -> Vec<PortView> {
    ports.iter().map(view).collect()
}

fn numbers(ports: &[&PortView]) -> Vec<u16> {
    ports.iter().map(|p| p.number).collect()
}

#[test]
fn with_nothing_ticked_stop_all_takes_every_port_of_the_workspace() {
    let ports = [port(1, Some(2)), port(3, None), port(5, Some(2))];
    assert_eq!(numbers(&to_stop(&views(&ports), &HashSet::new(), false)), [1, 5]);
}

#[test]
fn the_ticked_ports_are_the_ones_stopped() {
    let ports = [port(1, Some(2)), port(5, Some(2))];
    let ticked = HashSet::from([view(&ports[1]).key]);
    assert_eq!(numbers(&to_stop(&views(&ports), &ticked, false)), [5]);
}

#[test]
fn every_port_stops_only_what_is_ticked() {
    let ports = [port(1, Some(2)), port(5, Some(3))];
    assert_eq!(to_stop(&views(&ports), &HashSet::new(), true), Vec::<&PortView>::new());
    let ticked = HashSet::from([view(&ports[0]).key]);
    assert_eq!(numbers(&to_stop(&views(&ports), &ticked, true)), [1]);
}

#[test]
fn a_port_of_no_workspace_is_never_stopped_even_ticked() {
    let ports = [port(1, None)];
    let ticked = HashSet::from([view(&ports[0]).key]);
    assert_eq!(to_stop(&views(&ports), &ticked, false), Vec::<&PortView>::new());
}

#[test]
fn the_button_says_how_many_it_stops() {
    assert_eq!(stop_label(3, false), "Stop all…");
    assert_eq!(stop_label(2, true), "Stop 2…");
}

#[test]
fn the_question_names_each_program_and_warns_once() {
    let (a, b) = (port(1, Some(2)), Port { via: Some("agent".into()), pane: None, ..port(5, Some(2)) });
    let text = stop_many_warning(&[&view(&a), &view(&b)], None);
    assert!(text.contains("vite on port 1 (pid 2), vite on port 5 (pid 6)"));
    assert!(text.contains("An agent started some of them."));
    assert!(text.contains("No tab started some of them"));
}

fn session() -> Session {
    serde_json::from_str(fixture!("machines/view.json")).expect("a view line")
}

fn machine_port(number: u16, pane: u64, workspace: u64) -> Port<ThereId> {
    Port {
        number,
        pid: 90,
        program: "Python".into(),
        via: Some("pane".into()),
        pane: Some(ThereId::heard(pane)),
        workspace: Some(ThereId::heard(workspace)),
        ..Port::default()
    }
}

#[test]
fn a_machine_ports_place_names_the_machines_tab_not_ours() {
    let mut session = session();
    session.projects.iter_mut().flat_map(|p| p.workspaces.iter_mut()).flat_map(|w| w.tabs.iter_mut()).for_each(|tab| {
        tab.name = "ours".into();
    });
    let seen = session.machine("vps").expect("vps");
    let there = view_there(&machine_port(8765, 1, 3), Some(seen), false);
    assert_eq!(there.place, "pane 1 · zsh");
    assert!(session.tab_of_pane(1).is_some_and(|tab| tab.name == "ours"));
}

#[test]
fn every_port_there_names_the_machines_workspace() {
    let session = session();
    let seen = session.machine("vps").expect("vps");
    let there = view_there(&machine_port(8765, 5, 7), Some(seen), true);
    assert_eq!(there.place, "shop › main · pane 5 · zsh");
    let none = view_there(&Port { workspace: None, pane: None, via: None, ..machine_port(1, 5, 7) }, Some(seen), true);
    assert_eq!(none.place, "no workspace");
}

#[test]
fn go_on_a_machines_row_aims_at_its_pane_there() {
    let session = session();
    let seen = session.machine("vps").expect("vps");
    let there = view_there(&machine_port(8765, 5, 7), Some(seen), false);
    let notice = NoticeThere {
        machine: "vps".into(),
        boot: seen.boot.clone(),
        panes: vec![ThereId::heard(5)],
        gone: "That pane is no longer on vps".into(),
    };
    assert_eq!(there.go, Some(Go::There(notice)));
    let local = view_here(&Port { pane: Some(5), ..Port::default() }, None, false);
    assert_eq!(local.go, Some(Go::Here(5)));
    assert_eq!(view_there(&machine_port(1, 5, 7), None, false).go, None);
}

#[test]
fn the_same_port_and_pid_here_and_there_have_different_keys() {
    let here = Port { number: 8765, pid: 90, ..Port::default() };
    assert_ne!(view_here(&here, None, false).key, there_key(&machine_port(8765, 5, 7)));
}

#[test]
fn stopping_on_a_machine_names_it() {
    let there = view_there(&machine_port(8765, 5, 7), None, false);
    assert_eq!(stop_title(&there, Some("vps")), "Stop Python on port 8765 on vps?");
    assert_eq!(stop_title(&there, None), "Stop Python on port 8765?");
    assert!(stop_warning(&there, Some("vps")).starts_with("Python (pid 90) on vps gets SIGTERM"));
    assert!(stop_many_warning(&[&there], Some("vps")).contains("Python on port 8765 (pid 90) on vps"));
}

#[test]
fn a_local_port_has_open_and_both_links_and_a_remote_one_only_once_tunnelled() {
    let url = Some("http://localhost:8765");
    let here = row_links(url, false, false, None);
    assert_eq!((here.open.as_deref(), here.system.as_deref(), here.copy.as_deref()), (url, url, url));
    let remote = row_links(url, false, true, None);
    assert_eq!((remote.open.as_deref(), remote.system, remote.copy.as_deref()), (url, None, url));
    let held = row_links(url, false, true, Some(63235));
    assert_eq!(held.system.as_deref(), Some("http://localhost:63235"));
}

#[test]
fn a_machines_port_has_no_open_and_its_links_only_once_tunnelled() {
    let url = Some("http://localhost:8765");
    assert_eq!(row_links(url, true, false, None), RowLinks { open: None, system: None, copy: None });
    let held = row_links(url, true, false, Some(63235));
    assert_eq!(held.open, None);
    assert_eq!(held.system.as_deref(), Some("http://localhost:63235"));
    assert_eq!(held.copy.as_deref(), Some("http://localhost:63235"));
    assert_eq!(row_links(None, false, false, None).copy, None);
}

#[test]
fn a_machines_own_address_is_no_local_link_until_it_is_tunnelled() {
    let url = Some("http://localhost:8765");
    assert_eq!(address_text(url, 8765, Some("vps"), None), ("http://localhost:8765 on vps".to_string(), false));
    assert_eq!(address_text(url, 8765, Some("vps"), Some(63235)), ("http://localhost:63235".to_string(), true));
    assert_eq!(address_text(url, 8765, None, None), ("http://localhost:8765".to_string(), true));
    assert_eq!(address_text(None, 8765, Some("vps"), None), ("port 8765".to_string(), true));
}
