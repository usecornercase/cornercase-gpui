use super::*;
use crate::test_util::fixture;

fn check() -> Value {
    serde_json::from_str(fixture!("agents/close-agent-check.json")).expect("the check parses")
}

fn names(tab: u64) -> Option<String> {
    [(20, "shop"), (21, "review")].iter().find(|(id, _)| *id == tab).map(|(_, name)| (*name).to_string())
}

#[test]
fn a_pane_is_named_by_its_agent_and_its_tab() {
    let agent = serde_json::json!({"agent": "claude"});
    assert_eq!(pane_label(&agent, Some("shop".into())), "claude in “shop”");
    assert_eq!(pane_label(&agent, None), "claude");
    assert_eq!(pane_label(&serde_json::json!({}), None), "shell");
}

#[test]
fn a_machines_report_names_its_tabs_from_its_session() {
    let there = report(&check(), Some("vps"), names);
    assert_eq!(there.title, "Close claude on vps?");
    assert!(there.closes.ends_with("stay there."));
    assert_eq!(there.stops, ["claude in “shop”", "codex in “review”"]);
    assert_eq!(there.keeps, ["claude"]);
    let here = report(&check(), None, |_| None);
    assert_eq!(here.title, "Close claude?");
    assert!(here.closes.ends_with("stay."));
}
