use super::*;

fn workspace(closed: Option<usize>, exited: &[&str]) -> Workspace {
    let run = (!exited.is_empty())
        .then(|| super::super::WorkspaceRun { exited: exited.iter().map(|s| (*s).to_string()).collect() });
    Workspace { closed_agents: closed, run, ..Workspace::default() }
}

#[test]
fn a_workspace_reopens_its_closed_agents_before_its_runs_exited_roles() {
    assert_eq!(workspace(Some(2), &["coder"]).reopen(), Some(Reopen::Closed(2)));
}

#[test]
fn a_live_runs_exited_roles_are_reopened_once_nothing_is_kept() {
    assert_eq!(workspace(None, &["coder", "reviewer"]).reopen(), Some(Reopen::Exited(2)));
    assert_eq!(workspace(Some(0), &["coder", "reviewer"]).reopen(), Some(Reopen::Exited(2)));
}

#[test]
fn a_workspace_with_nothing_to_reopen_offers_nothing() {
    assert_eq!(workspace(None, &[]).reopen(), None);
    assert_eq!(workspace(Some(0), &[]).reopen(), None);
}

#[test]
fn a_view_line_without_exited_offers_only_the_closed_agents() {
    let older: Workspace =
        serde_json::from_str(r#"{"id":2,"closed_agents":1,"run":{"end":"pull_request","reached":false}}"#)
            .expect("a workspace");
    assert_eq!(older.reopen(), Some(Reopen::Closed(1)));
    let none: Workspace =
        serde_json::from_str(r#"{"id":2,"run":{"end":"pull_request","reached":false}}"#).expect("a workspace");
    assert_eq!(none.reopen(), None);
}

#[test]
fn a_reopen_names_what_one_press_brings_back() {
    let labels = [Reopen::Closed(1), Reopen::Closed(3), Reopen::Exited(1), Reopen::Exited(2)].map(Reopen::label);
    assert_eq!(
        labels,
        ["Reopen 1 closed agent", "Reopen 3 closed agents", "Reopen 1 exited agent", "Reopen 2 exited agents"]
    );
}

#[test]
fn only_closed_agents_come_before_the_exited_ones() {
    assert_eq!((Reopen::Closed(2).closed_first(), Reopen::Exited(2).closed_first()), (2, 0));
}

#[test]
fn a_status_line_of_0_13_72_reads_the_runs_exited_roles() {
    let session: crate::model::Session =
        serde_json::from_str(crate::test_util::fixture!("session/status-exited.json")).expect("a status");
    let (_, workspace) = session.workspace(5).expect("the run's workspace");
    assert_eq!(workspace.exited_roles(), ["coder"]);
    assert_eq!(workspace.reopen(), Some(Reopen::Exited(1)));
}

#[test]
fn a_live_run_does_not_offer_the_workspaces_other_closed_agents() {
    assert_eq!(workspace(Some(1), &[]).reopen_for_run(false), None);
    assert_eq!(workspace(Some(2), &["coder"]).reopen_for_run(false), Some(Reopen::Closed(2)));
}

#[test]
fn a_closed_run_offers_the_workspaces_closed_agents() {
    assert_eq!(workspace(Some(3), &[]).reopen_for_run(true), Some(Reopen::Closed(3)));
    assert_eq!(workspace(None, &[]).reopen_for_run(true), None);
}

#[test]
fn a_status_line_with_kept_and_exited_agents_reopens_the_kept_ones_first() {
    let session: crate::model::Session =
        serde_json::from_str(crate::test_util::fixture!("session/status-exited-kept.json")).expect("a status");
    let (_, workspace) = session.workspace(5).expect("the run's workspace");
    assert_eq!(workspace.exited_roles(), ["orchestrator", "coder"]);
    assert_eq!((workspace.closed_agents, workspace.reopen()), (Some(1), Some(Reopen::Closed(1))));
}
