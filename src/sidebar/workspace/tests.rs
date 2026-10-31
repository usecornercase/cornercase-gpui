use super::*;

#[test]
fn pull_request_tags_follow_state_and_checks() {
    let pr = |state: &str, checks: Option<&str>| PullRequest {
        number: 485,
        state: state.into(),
        checks: checks.map(String::from),
        ..Default::default()
    };
    assert_eq!(pull_tag(&pr("open", Some("failing"))).0, "#485✗");
    assert_eq!(pull_tag(&pr("draft", Some("pending"))).0, "#485•");
    assert_eq!(pull_tag(&pr("open", Some("passing"))).0, "#485✓");
    assert_eq!(pull_tag(&pr("open", None)).0, "#485");
    assert_eq!(pull_tag(&pr("merged", Some("failing"))).0, "#485");
    assert!(pull_tag(&pr("closed", None)).2);
}
