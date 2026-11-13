use super::data::{List, Port, list_command};
use super::rows;
use crate::model::ThereId;
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
fn only_a_port_of_a_workspace_can_be_stopped() {
    let outside: Port = Port { workspace: None, ..Port::default() };
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
fn a_machines_ports_parse_with_its_own_ids() {
    let list: List<ThereId> = serde_json::from_str(fixture!("ports/machine_workspace.json")).expect("parses");
    let [port] = list.ports.as_slice() else { panic!("one port") };
    assert_eq!((port.number, port.pane, port.workspace), (8765, Some(ThereId::heard(3)), Some(ThereId::heard(2))));
    assert_eq!(port.url.as_deref(), Some("http://localhost:8765"));
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
