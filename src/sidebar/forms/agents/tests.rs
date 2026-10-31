use super::{Closing, Plan, Stop};
use crate::test_util::fixture;

fn plan() -> Plan {
    serde_json::from_str(fixture!("agents/close-check.json")).expect("the check parses")
}

#[test]
fn a_working_agent_is_named_with_its_cut_turn() {
    let p = plan();
    assert!(p.loses_something());
    assert_eq!(
        p.message(),
        "Their tabs close, and each can be reopened later in its own conversation: claude in pane 3, its turn is cut."
    );
}

#[test]
fn closing_what_loses_something_needs_force() {
    assert_eq!(plan().command(2), ["agents", "close", "--workspace", "2", "--force"]);
}

#[test]
fn idle_agents_that_resume_close_without_force() {
    let idle = Plan { agents: vec![Closing { pane: 7, resumes: true, ..Closing::default() }], stops: Vec::new() };
    assert!(!idle.loses_something());
    assert_eq!(idle.command(5), ["agents", "close", "--workspace", "5"]);
}

#[test]
fn other_panes_that_stop_are_named() {
    let p = Plan {
        agents: vec![Closing { pane: 7, resumes: false, agent: Some("codex".into()), ..Closing::default() }],
        stops: vec![Stop { pane: 9, tab: 4 }],
    };
    assert!(p.message().contains("codex in pane 7, starts a new conversation when reopened"));
    assert!(p.message().ends_with("Also stops pane 9 in tab 4."));
}
