use super::*;

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
