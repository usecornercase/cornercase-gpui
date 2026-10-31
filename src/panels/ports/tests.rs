use super::data::{List, Port, list_command, stop_warning};
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
