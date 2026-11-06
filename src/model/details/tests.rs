use super::memory_size;
use crate::model::{PaneInfo, Project, Session, Tab, Workspace};
use crate::test_util::fixture;

const MB: u64 = 1024 * 1024;

fn pane(id: u64) -> PaneInfo {
    PaneInfo { id, ..Default::default() }
}

fn agent(id: u64, status: &str) -> PaneInfo {
    PaneInfo { id, agent: Some("claude".into()), status: Some(status.into()), ..Default::default() }
}

fn tab_of(panes: Vec<PaneInfo>) -> Tab {
    Tab { id: panes[0].id * 10, panes, ..Default::default() }
}

fn project(id: u64, tabs: Vec<Tab>) -> Project {
    Project { id, workspaces: vec![Workspace { id: id * 10, tabs, ..Default::default() }], ..Default::default() }
}

fn session(projects: Vec<Project>) -> Session {
    Session { projects, ..Default::default() }
}

#[test]
fn a_view_line_says_what_a_pane_holds_back_and_the_memory_it_uses() {
    let session: Session = serde_json::from_str(fixture!("session/held.json")).expect("a session");
    let held = session.pane(9).expect("the pane");
    assert_eq!((held.held_sends, held.draft, held.memory), (1, true, Some(2_437_120)));
    let other = session.pane(5).expect("the other pane");
    assert_eq!((other.held_sends, other.draft, other.compacted), (0, false, false));
}

#[test]
fn a_pane_reads_as_compacted_only_when_the_view_says_so() {
    let pane: PaneInfo = serde_json::from_str(r#"{"id":1,"compacted":true}"#).expect("a pane");
    assert!(pane.compacted);
    let pane: PaneInfo = serde_json::from_str(r#"{"id":1}"#).expect("a pane");
    assert!(!pane.compacted && pane.memory.is_none());
}

#[test]
fn memory_is_in_whole_megabytes_below_a_gigabyte() {
    assert_eq!(memory_size(2_437_120), "2 MB");
    assert_eq!(memory_size(1023 * MB), "1023 MB");
}

#[test]
fn memory_is_in_gigabytes_with_one_decimal_from_there() {
    assert_eq!(memory_size(1024 * MB), "1.0 GB");
    assert_eq!(memory_size(1229 * MB), "1.2 GB");
}

#[test]
fn a_little_memory_still_reads_as_one_megabyte() {
    assert_eq!(memory_size(4096), "1 MB");
}

#[test]
fn a_tab_adds_up_the_memory_of_its_panes() {
    let with = |memory| PaneInfo { memory: Some(memory), ..pane(1) };
    assert_eq!(tab_of(vec![with(3), pane(2), with(4)]).memory(), Some(7));
}

#[test]
fn a_tab_with_no_figure_shows_no_memory() {
    assert_eq!(tab_of(vec![pane(1), pane(2)]).memory(), None);
}

#[test]
fn the_next_agent_that_needs_you_comes_after_the_pane_you_are_in() {
    let tabs =
        vec![tab_of(vec![agent(1, "waiting")]), tab_of(vec![agent(2, "done")]), tab_of(vec![agent(3, "failed")])];
    let session = session(vec![project(1, tabs)]);
    assert_eq!(session.agent_needing_you(Some(1)), Some(2));
    assert_eq!(session.agent_needing_you(Some(2)), Some(3));
}

#[test]
fn the_turn_wraps_back_to_the_first_agent() {
    let session = session(vec![project(1, vec![tab_of(vec![agent(1, "pending")]), tab_of(vec![agent(2, "done")])])]);
    assert_eq!(session.agent_needing_you(Some(2)), Some(1));
}

#[test]
fn without_an_agent_on_screen_the_turn_starts_at_the_first() {
    let session = session(vec![project(1, vec![tab_of(vec![agent(1, "working")]), tab_of(vec![agent(2, "done")])])]);
    assert_eq!(session.agent_needing_you(None), Some(2));
}

#[test]
fn an_agent_folded_under_another_still_counts() {
    let parent = PaneInfo { collapsed: true, ..agent(1, "working") };
    let child = PaneInfo { parent: Some(1), ..agent(2, "waiting") };
    let session = session(vec![project(1, vec![tab_of(vec![parent]), tab_of(vec![child])])]);
    assert_eq!(session.agent_needing_you(Some(1)), Some(2));
}

#[test]
fn agents_at_work_or_idle_do_not_need_you() {
    let session = session(vec![project(1, vec![tab_of(vec![agent(1, "working")]), tab_of(vec![agent(2, "idle")])])]);
    assert_eq!(session.agent_needing_you(Some(1)), None);
}

#[test]
fn no_agents_at_all_answer_nothing() {
    assert_eq!(session(vec![project(1, vec![tab_of(vec![pane(1)])])]).agent_needing_you(Some(1)), None);
}

#[test]
fn a_workspace_without_changes_counts_no_files() {
    assert_eq!(Workspace::default().changed_files(), 0);
}
