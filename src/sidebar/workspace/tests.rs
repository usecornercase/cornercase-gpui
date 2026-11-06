use super::*;
use crate::model::Shared;

#[test]
fn the_pull_request_tooltip_says_its_title_summary_and_failures() {
    let pr = PullRequest {
        number: 485,
        title: "Dark mode".into(),
        state: "open".into(),
        checks: Some("failing".into()),
        failed: vec!["test".into(), "clippy".into()],
        conflicts: true,
        base: Some("main".into()),
        ..Default::default()
    };
    assert_eq!(pull_tip(&pr), "#485 Dark mode\nopen · conflicts with main · checks failing\nfailed: test, clippy");
}

#[test]
fn a_row_that_shares_nothing_has_no_tag() {
    let session = Session::default();
    assert!(shared_tag(&Workspace::default(), Some(&session)).is_none());
}

#[test]
fn a_row_that_shares_files_has_a_tag() {
    let shared = vec![Shared { workspace: 2, count: 1, files: vec!["a.rs".into()] }];
    let ws = Workspace { shared, ..Workspace::default() };
    assert!(shared_tag(&ws, Some(&Session::default())).is_some());
}

fn agent() -> PaneInfo {
    PaneInfo {
        agent: Some("claude".into()),
        role: Some("coder".into()),
        model: Some("Opus 5.5".into()),
        context: Some(12),
        ..Default::default()
    }
}

#[test]
fn an_agent_row_without_an_account_reads_as_before() {
    assert_eq!(agent_details(&agent(), None, &Look::default()), "claude · coder · Opus 5.5 · 12%");
}

#[test]
fn an_agent_row_names_its_account_after_the_model() {
    assert_eq!(agent_details(&agent(), Some("work"), &Look::default()), "claude · coder · Opus 5.5 · work · 12%");
}

#[test]
fn an_agent_row_says_a_restart_waits_for_its_turn() {
    let waiting = PaneInfo { account_restart: Some("waiting".into()), ..agent() };
    assert_eq!(
        agent_details(&waiting, Some("work"), &Look::default()),
        "claude · coder · Opus 5.5 · work · 12% · restarts after this turn"
    );
}
