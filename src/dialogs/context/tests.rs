use super::*;
use crate::cli::Target;
use crate::model::Session;

fn pane(id: u64, agent: Option<&str>) -> PaneInfo {
    PaneInfo { id, agent: agent.map(str::to_string), ..PaneInfo::default() }
}

fn tab(panes: Vec<PaneInfo>) -> Tab {
    Tab { panes, ..Tab::default() }
}

fn session(text: &str) -> Session {
    serde_json::from_str(text).expect("valid JSON")
}

const ONE_AGENT: &str = r#"{"projects":[{"id":1,"workspaces":[{"id":2,"tabs":[{"id":3,"panes":[
    {"id":4,"agent":"claude"},{"id":5}]}]}]}]}"#;

#[test]
fn a_tab_with_one_agent_offers_one_plain_entry() {
    let choices = context_choices(&tab(vec![pane(1, None), pane(2, Some("claude"))]));
    assert_eq!(choices, [(2, "View context".to_string())]);
}

#[test]
fn a_tab_without_an_agent_offers_nothing() {
    assert_eq!(context_choices(&tab(vec![pane(1, None)])), Vec::<(u64, String)>::new());
}

#[test]
fn several_agents_in_a_tab_are_told_apart_in_pane_order() {
    let choices = context_choices(&tab(vec![
        pane(1, Some("claude")),
        pane(2, None),
        pane(3, Some("codex")),
        pane(4, Some("claude")),
    ]));
    let said: Vec<(u64, &str)> = choices.iter().map(|(id, label)| (*id, label.as_str())).collect();
    assert_eq!(said, [(1, "View context · claude"), (3, "View context · codex"), (4, "View context · claude 2")]);
}

fn slice(fill: Fill) -> Slice {
    Slice { label: "x".into(), fraction: 0.1, tokens: "1".into(), share: "0.1%".into(), fill }
}

#[test]
fn only_the_free_slice_gives_up_its_width_to_the_bars_gaps() {
    assert_eq!(bar_slice(&slice(Fill::Free)).style().flex_shrink, Some(GIVES_WAY));
    assert_eq!(bar_slice(&slice(Fill::Used(0))).style().flex_shrink, None);
    assert_eq!(bar_slice(&slice(Fill::Buffer)).style().flex_shrink, None);
}

#[test]
fn a_long_file_name_can_shrink_inside_its_cell() {
    let mut part = path_part("a-file-name-wider-than-its-cell.md");
    assert_eq!(part.style().flex_shrink, None);
    assert_eq!(part.style().min_size.width, Some(px(0.).into()));
}

#[test]
fn the_dialog_stays_while_its_pane_runs_an_agent() {
    assert!(!agent_gone(Some(&session(ONE_AGENT)), 4));
}

#[test]
fn the_dialog_closes_when_its_pane_goes() {
    assert!(agent_gone(Some(&session(ONE_AGENT)), 9));
}

#[test]
fn the_dialog_closes_when_its_pane_runs_no_agent() {
    assert!(agent_gone(Some(&session(ONE_AGENT)), 5));
}

#[test]
fn a_lost_connection_does_not_close_the_dialog() {
    assert!(!agent_gone(None, 4));
}

#[test]
fn a_machines_dialog_closes_only_when_its_agent_is_gone() {
    assert!(gone(&AgentReach::Gone("vps restarted".into())));
    assert!(!gone(&AgentReach::Down("vps unreachable".into())));
    assert!(!gone(&AgentReach::Ready(Target::Local)));
}
