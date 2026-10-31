use super::*;
use crate::test_util::fixture;

fn session() -> Session {
    serde_json::from_str(fixture!("browser/session.json")).expect("the session parses")
}

fn pane_mut(session: &mut Session, id: u64) -> &mut PaneInfo {
    let panes = session.projects.iter_mut().flat_map(|p| &mut p.workspaces).flat_map(|w| &mut w.tabs);
    panes.flat_map(|t| &mut t.panes).find(|p| p.id == id).expect("the pane exists")
}

#[test]
fn agents_reach_the_browser_on_localhost() {
    assert_eq!(endpoint(9222), "http://127.0.0.1:9222");
}

#[test]
fn a_workspace_lists_its_agents_in_tab_order_with_their_titles() {
    let labels: Vec<(u64, String)> = workspace_agents(&session(), 10).into_iter().map(|c| (c.pane, c.label)).collect();
    assert_eq!(labels, [(1000, "claude · Fix the login form".to_string()), (1010, "codex · Review".to_string())]);
}

#[test]
fn a_workspace_without_agents_lists_none() {
    assert_eq!(workspace_agents(&session(), 11), []);
}

#[test]
fn an_agent_without_a_title_or_tab_is_named_by_its_program() {
    let pane = PaneInfo { agent: Some("claude".into()), ..PaneInfo::default() };
    assert_eq!(agent_label(&pane, None), "claude");
}

#[test]
fn without_a_choice_the_shown_tabs_agent_is_the_target() {
    assert_eq!(pick_agent(&session(), None), Some(1010));
}

#[test]
fn a_remembered_agent_of_the_workspace_is_the_target() {
    assert_eq!(pick_agent(&session(), Some(1000)), Some(1000));
}

#[test]
fn a_remembered_pane_that_is_not_an_agent_here_is_ignored() {
    assert_eq!(pick_agent(&session(), Some(1100)), Some(1010));
}

#[test]
fn an_idle_agent_is_ready_for_a_paste() {
    assert_eq!(unready(&session(), 1010), None);
}

#[test]
fn an_agent_waiting_for_an_answer_refuses_a_paste() {
    let mut session = session();
    pane_mut(&mut session, 1010).status = Some("waiting".into());
    assert_eq!(unready(&session, 1010).as_deref(), Some("codex is waiting for an answer: answer it first"));
}

#[test]
fn an_agent_showing_a_dialog_refuses_a_paste() {
    let mut session = session();
    pane_mut(&mut session, 1000).dialog = true;
    assert_eq!(unready(&session, 1000).as_deref(), Some("claude shows a dialog: close it first"));
}

#[test]
fn a_closed_agent_refuses_a_paste() {
    assert_eq!(unready(&session(), 4242).as_deref(), Some("That agent is gone"));
}

#[test]
fn the_note_names_the_address_and_both_ways_to_connect() {
    let note = driving_note("http://127.0.0.1:9222");
    assert!(note.contains("chrome-devtools-mcp --browser-url=http://127.0.0.1:9222"));
    assert!(note.contains("connectOverCDP(\"http://127.0.0.1:9222\")"));
    assert!(!note.contains('\n'));
}

#[test]
fn a_submitted_text_goes_after_the_end_of_the_options() {
    let text = "-v is not a flag here".to_string();
    assert_eq!(send_args(12, text.clone()), ["send", "--pane", "12", "--enter", "--", &text]);
}
