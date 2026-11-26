use super::*;
use crate::model::{MachineSeen, Project, Tab, ThereWorkspace, Workspace};
use crate::test_util::fixture;

fn parse(text: &str) -> Session {
    serde_json::from_str(text).expect("valid JSON")
}

fn folds(projects: bool, workspaces: bool, agents: bool) -> SectionFolds {
    SectionFolds { projects, workspaces, agents }
}

fn workspace(status: &str) -> Workspace {
    let tab = Tab { status: Some(status.into()), ..Default::default() };
    Workspace { tabs: vec![tab], ..Default::default() }
}

fn project(id: u64, status: &str) -> Project {
    Project { id, workspaces: vec![workspace(status)], ..Default::default() }
}

fn there_project(paired: Option<u64>, statuses: &[&str]) -> ThereProject {
    let workspaces = statuses.iter().map(|status| {
        let tab = Tab { status: Some((*status).into()), ..Default::default() };
        ThereWorkspace { tabs: vec![tab], ..Default::default() }
    });
    ThereProject { paired, workspaces: workspaces.collect(), ..Default::default() }
}

fn session(projects: Vec<Project>, there: Vec<ThereProject>) -> Session {
    let machine = MachineSeen { name: "vps".into(), projects: there, ..Default::default() };
    Session { projects, machines: vec![machine], ..Default::default() }
}

#[test]
fn a_session_reads_its_collapsed_sections() {
    let session = parse(fixture!("session/view_0_14_1.json"));
    assert_eq!(session.collapsed_sections, folds(true, false, true));
}

#[test]
fn a_session_without_collapsed_sections_is_all_open() {
    let session = parse(fixture!("session/view_0_13_28.json"));
    assert_eq!(session.collapsed_sections, folds(false, false, false));
}

#[test]
fn a_null_section_list_folds_nothing() {
    let session = parse(r#"{"collapsed_sections":null}"#);
    assert_eq!(session.collapsed_sections, folds(false, false, false));
}

#[test]
fn a_section_the_window_does_not_know_is_skipped() {
    let session = parse(r#"{"collapsed_sections":["sideways",3,"workspaces"]}"#);
    assert_eq!(session.collapsed_sections, folds(false, true, false));
}

#[test]
fn each_section_flag_is_the_one_it_reads() {
    let mut folded = SectionFolds::default();
    for section in [Section::Projects, Section::Workspaces, Section::Agents] {
        assert!(!folded.folded(section), "{section:?}");
        *folded.flag(section) = true;
        assert!(folded.folded(section), "{section:?}");
    }
}

#[test]
fn the_projects_title_counts_this_computers_projects_and_the_machines_alone_ones() {
    let ours = vec![project(1, "idle"), project(2, "idle")];
    let session = session(ours, vec![there_project(Some(1), &["idle"]), there_project(None, &["idle"])]);
    assert_eq!(session.projects_tags().count, 3);
}

#[test]
fn a_working_project_gives_the_projects_title_no_mark() {
    let session = session(vec![project(1, "working")], Vec::new());
    assert_eq!(session.projects_tags().attention, None);
}

#[test]
fn the_projects_title_marks_the_loudest_status_that_needs_you() {
    let ours = vec![project(1, "pending"), project(2, "done")];
    let session = session(ours, vec![there_project(None, &["waiting", "working"])]);
    assert_eq!(session.projects_tags().attention, Some("waiting"));
}

#[test]
fn the_projects_title_marks_a_waiting_agent_of_a_paired_machine_workspace() {
    let session = session(vec![project(1, "pending")], vec![there_project(Some(1), &["waiting"])]);
    assert_eq!(session.projects_tags().attention, Some("waiting"));
}

#[test]
fn the_workspaces_title_counts_the_paired_machines_workspaces_too() {
    let there = vec![there_project(Some(1), &["working", "failed"]), there_project(Some(2), &["waiting"])];
    let session = session(vec![project(1, "idle"), project(2, "idle")], there);
    assert_eq!(session.workspaces_tags(1), SectionTags { count: 3, attention: Some("failed") });
}

#[test]
fn an_unknown_project_gives_the_workspaces_title_nothing_to_say() {
    let session = session(vec![project(1, "waiting")], Vec::new());
    assert_eq!(session.workspaces_tags(9), SectionTags::default());
}

#[test]
fn a_machines_project_alone_counts_its_workspaces_and_what_needs_you() {
    let there = there_project(None, &["working", "done"]);
    assert_eq!(there.tags(), SectionTags { count: 2, attention: Some("done") });
}
