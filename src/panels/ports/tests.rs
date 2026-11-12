use std::collections::HashSet;

use super::data::{List, Port, list_command, stop_label, stop_many_warning, stop_warning, to_stop};
use super::rows;
use crate::test_util::fixture;

fn ports() -> Vec<Port> {
    serde_json::from_str::<List>(fixture!("ports/workspace.json")).expect("the ports parse").ports
}

#[test]
fn a_port_reads_its_url_program_and_pane() {
    let p = &ports()[0];
    assert_eq!((p.url.as_deref(), p.pane, p.workspace), (Some("http://localhost:61440"), Some(28), Some(22)));
    assert_eq!(p.command_line(), "python3 -m http.server 0 --bind 127.0.0.1");
}

#[test]
fn stopping_names_the_port_and_the_pid() {
    assert_eq!(ports()[0].stop_command(), ["ports", "stop", "61440", "--pid", "52064"]);
}

#[test]
fn only_a_port_of_a_workspace_can_be_stopped() {
    let outside = Port { workspace: None, ..Port::default() };
    assert!(!outside.stoppable());
    assert!(ports()[0].stoppable());
}

#[test]
fn the_place_says_how_the_port_belongs_to_the_workspace() {
    let by_pane = Port { pane: Some(4), via: Some("pane".into()), ..Port::default() };
    assert_eq!(by_pane.place(None, false), "pane 4");
    let by_agent = Port { pane: Some(4), via: Some("agent".into()), agent: Some("claude".into()), ..Port::default() };
    assert_eq!(by_agent.place(None, false), "agent claude · pane 4");
    let by_folder = Port { via: Some("cwd".into()), workspace: Some(2), ..Port::default() };
    assert_eq!(by_folder.place(None, false), "in the worktree, no tab");
    assert_eq!(Port::default().place(None, true), "no workspace");
}

#[test]
fn the_list_command_asks_for_the_workspace_or_every_port() {
    assert_eq!(list_command(false, Some(3)), ["ports", "--json", "--workspace", "3"]);
    assert_eq!(list_command(true, Some(3)), ["ports", "--json", "--all"]);
}

#[test]
fn the_stop_warning_names_an_agent_and_a_program_no_tab_started() {
    let p = Port { via: Some("agent".into()), pane: None, program: "vite".into(), ..Port::default() };
    let text = stop_warning(&p);
    assert!(text.contains("An agent started it"));
    assert!(text.contains("No tab started it"));
}

fn port(number: u16, workspace: Option<u64>) -> Port {
    Port { number, pid: u64::from(number) + 1, program: "vite".into(), workspace, pane: Some(4), ..Port::default() }
}

fn numbers(ports: &[&Port]) -> Vec<u16> {
    ports.iter().map(|p| p.number).collect()
}

#[test]
fn with_nothing_ticked_stop_all_takes_every_port_of_the_workspace() {
    let ports = [port(1, Some(2)), port(3, None), port(5, Some(2))];
    assert_eq!(numbers(&to_stop(&ports, &HashSet::new(), false)), [1, 5]);
}

#[test]
fn the_ticked_ports_are_the_ones_stopped() {
    let ports = [port(1, Some(2)), port(5, Some(2))];
    let ticked = HashSet::from([ports[1].key()]);
    assert_eq!(numbers(&to_stop(&ports, &ticked, false)), [5]);
}

#[test]
fn every_port_stops_only_what_is_ticked() {
    let ports = [port(1, Some(2)), port(5, Some(3))];
    assert!(to_stop(&ports, &HashSet::new(), true).is_empty());
    let ticked = HashSet::from([ports[0].key()]);
    assert_eq!(numbers(&to_stop(&ports, &ticked, true)), [1]);
}

#[test]
fn a_port_of_no_workspace_is_never_stopped_even_ticked() {
    let ports = [port(1, None)];
    let ticked = HashSet::from([ports[0].key()]);
    assert!(to_stop(&ports, &ticked, false).is_empty());
}

#[test]
fn the_button_says_how_many_it_stops() {
    assert_eq!(stop_label(3, false), "Stop all…");
    assert_eq!(stop_label(2, true), "Stop 2…");
}

#[test]
fn the_question_names_each_program_and_warns_once() {
    let (a, b) = (port(1, Some(2)), Port { via: Some("agent".into()), pane: None, ..port(5, Some(2)) });
    let text = stop_many_warning(&[&a, &b]);
    assert!(text.contains("vite on port 1 (pid 2), vite on port 5 (pid 6)"));
    assert!(text.contains("An agent started some of them."));
    assert!(text.contains("No tab started some of them"));
}

#[test]
fn the_ports_note_says_how_ports_there_open_by_way() {
    use crate::model::tunnels::{Older, Way};

    let held = rows::remote_notes("devbox", &Way::Cornercase);
    assert_eq!(held.len(), 1);
    assert!(held[0].contains("Tunnel forwards one") && held[0].contains("through cornercase"));
    let own = rows::remote_notes("devbox", &Way::Own(Older::Server(None)));
    assert_eq!(own.len(), 2);
    assert!(own[0].contains("through an ssh tunnel") && own[1].contains("0.13.50 or newer on devbox"));
    assert_eq!(rows::remote_notes("devbox", &Way::Waiting).len(), 1);
}
