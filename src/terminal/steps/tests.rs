use super::*;
use crate::model::{Group, Project, Shown, ViewInfo, Workspace};

fn project(id: u64, group: Option<u64>, workspaces: &[u64]) -> Project {
    let workspaces = workspaces.iter().map(|&id| Workspace { id, ..Default::default() }).collect();
    Project { id, group, workspaces, ..Default::default() }
}

fn session(projects: Vec<Project>, shown: Shown) -> Session {
    let groups = vec![Group { id: 1, ..Default::default() }];
    Session { projects, groups, view: Some(ViewInfo { shown, ..Default::default() }), ..Default::default() }
}

fn showing(project: u64, workspace: u64) -> Shown {
    Shown { project: Some(project), workspace: Some(workspace), ..Default::default() }
}

fn agent(status: Option<&str>) -> PaneInfo {
    PaneInfo { id: 7, agent: Some("claude".into()), status: status.map(str::to_string), ..Default::default() }
}

#[test]
fn projects_step_in_the_sidebar_s_order_with_groups_last() {
    let projects = vec![project(1, Some(1), &[10]), project(2, None, &[20]), project(3, None, &[30])];
    let session = session(projects, showing(3, 30));
    assert_eq!(project_after(&session, 1), Some(1));
    assert_eq!(project_after(&session, -1), Some(2));
}

#[test]
fn projects_wrap_around() {
    let session = session(vec![project(1, None, &[10]), project(2, None, &[20])], showing(2, 20));
    assert_eq!(project_after(&session, 1), Some(1));
}

#[test]
fn a_lone_project_goes_nowhere() {
    let session = session(vec![project(1, None, &[10])], showing(1, 10));
    assert_eq!(project_after(&session, 1), None);
}

#[test]
fn workspaces_step_inside_the_shown_project_and_wrap() {
    let projects = vec![project(1, None, &[10, 11, 12]), project(2, None, &[20])];
    let session = session(projects, showing(1, 12));
    assert_eq!(workspace_after(&session, 1, |_| false), Some(10));
    assert_eq!(workspace_after(&session, -1, |_| false), Some(11));
}

#[test]
fn a_workspace_being_removed_is_skipped() {
    let session = session(vec![project(1, None, &[10, 11, 12])], showing(1, 10));
    assert_eq!(workspace_after(&session, 1, |id| id == 11), Some(12));
}

#[test]
fn a_lone_workspace_goes_nowhere() {
    let session = session(vec![project(1, None, &[10])], showing(1, 10));
    assert_eq!(workspace_after(&session, 1, |_| false), None);
}

#[test]
fn nothing_shown_steps_nowhere() {
    let session = session(vec![project(1, None, &[10])], Shown::default());
    assert_eq!(workspace_after(&session, 1, |_| false), None);
}

#[test]
fn an_agent_is_marked_for_review() {
    let (args, said) = mark_args(&agent(Some("done"))).expect("an agent");
    assert_eq!(args, ["mark", "--pane", "7"]);
    assert_eq!(said, MARKED);
}

#[test]
fn an_agent_pending_review_is_marked_as_reviewed() {
    let (args, said) = mark_args(&agent(Some("pending"))).expect("an agent");
    assert_eq!(args, ["mark", "--pane", "7", "--reviewed"]);
    assert_eq!(said, REVIEWED);
}

#[test]
fn the_mark_is_told_once_cornercase_took_it() {
    let (text, kind) = mark_answer(&Ok(String::new()), MARKED);
    assert_eq!(text, MARKED);
    assert!(matches!(kind, ToastKind::Info));
}

#[test]
fn a_refused_mark_says_why_and_not_that_it_was_marked() {
    let (text, kind) = mark_answer(&Err(anyhow::anyhow!("no such pane")), MARKED);
    assert!(text.contains("no such pane"), "{text}");
    assert!(matches!(kind, ToastKind::Error));
}

#[test]
fn a_pane_without_an_agent_has_nothing_to_mark() {
    assert!(mark_args(&PaneInfo::default()).is_none());
}
