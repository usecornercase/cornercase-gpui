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
