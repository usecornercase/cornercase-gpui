use super::*;
use crate::model::Stack;

fn pr(state: &str, checks: Option<&str>) -> PullRequest {
    PullRequest { number: 485, state: state.into(), checks: checks.map(String::from), ..Default::default() }
}

fn layer(number: u64, state: &str, checks: Option<&str>) -> Layer {
    Layer { number, state: state.into(), checks: checks.map(String::from), ..Default::default() }
}

#[test]
fn pull_request_tags_follow_state_and_checks() {
    assert_eq!(pull_tag(&pr("open", Some("failing"))).0, "#485✗");
    assert_eq!(pull_tag(&pr("draft", Some("pending"))).0, "#485•");
    assert_eq!(pull_tag(&pr("open", Some("passing"))).0, "#485✓");
    assert_eq!(pull_tag(&pr("open", None)).0, "#485");
    assert_eq!(pull_tag(&pr("merged", Some("failing"))).0, "#485");
    assert!(pull_tag(&pr("closed", None)).2);
}

#[test]
fn a_conflict_marks_the_tag_whatever_its_checks() {
    let conflicting = PullRequest { conflicts: true, ..pr("open", Some("passing")) };
    assert_eq!(pull_tag(&conflicting), ("#485!".to_string(), colours::waiting(), false));
}

#[test]
fn a_merge_request_keeps_the_hash_on_its_tag() {
    let pr = PullRequest { provider: "gitlab".into(), ..pr("open", Some("passing")) };
    assert_eq!(pull_tag(&pr).0, "#485✓");
}

#[test]
fn the_stack_tag_takes_the_colour_of_what_blocks_below() {
    let stack = |below: Layer| Stack {
        number: 480,
        position: 2,
        size: 3,
        layers: vec![below, layer(485, "open", None), layer(486, "open", None)],
        ..Default::default()
    };
    let stacked = |below| PullRequest { stack: Some(stack(below)), ..pr("open", None) };
    assert_eq!(stack_tag(&stacked(layer(483, "open", Some("passing")))), Some(("2/3".into(), colours::muted())));
    assert_eq!(stack_tag(&stacked(layer(483, "open", Some("failing")))).map(|t| t.1), Some(colours::failed()));
    let conflicting = Layer { conflicts: true, ..layer(483, "open", None) };
    assert_eq!(stack_tag(&stacked(conflicting)).map(|t| t.1), Some(colours::waiting()));
    assert_eq!(stack_tag(&pr("open", None)), None);
}

#[test]
fn its_own_layer_shows_its_own_checks_rather_than_the_stack_rollup() {
    let own = layer(485, "open", Some("passing"));
    let pull = PullRequest { conflicts: true, ..pr("open", Some("failing")) };
    assert_eq!(layer_sign(&pull, &own, true), Sign::Conflicts);
    assert_eq!(layer_sign(&pull, &own, false), Sign::Passed);
    assert_eq!(layer_sign(&pull, &layer(483, "merged", Some("failing")), false), Sign::Quiet);
}

fn said(lines: &[CheckLine]) -> Vec<(Sign, &str)> {
    lines.iter().map(|line| (line.sign, line.name.as_str())).collect()
}

fn check(name: &str, workflow: &str, outcome: &str, url: &str) -> Check {
    Check { name: name.into(), workflow: workflow.into(), outcome: outcome.into(), url: url.into() }
}

#[test]
fn before_every_check_is_known_failed_checks_are_listed_by_name() {
    let failing = PullRequest { failed: vec!["test".into(), "clippy".into()], ..pr("open", Some("failing")) };
    assert_eq!(said(&check_lines(&failing)), [(Sign::Failed, "test"), (Sign::Failed, "clippy")]);
}

#[test]
fn without_failures_one_line_says_how_the_checks_went() {
    assert_eq!(said(&check_lines(&pr("open", Some("pending")))), [(Sign::Pending, "Checks are running")]);
    assert_eq!(said(&check_lines(&pr("open", Some("passing")))), [(Sign::Passed, "Every check passed")]);
    assert_eq!(said(&check_lines(&pr("open", None))), [(Sign::Quiet, "No checks")]);
    let gitlab = PullRequest { provider: "gitlab".into(), ..pr("open", Some("passing")) };
    assert_eq!(said(&check_lines(&gitlab)), [(Sign::Passed, "The pipeline passed")]);
}

#[test]
fn every_check_is_listed_in_the_order_given_with_its_sign_and_link() {
    let all = vec![
        check("tests", "CI", "failed", "https://x/1"),
        check("build", "CI", "pending", "https://x/2"),
        check("review-bot", "", "passed", ""),
        check("deploy", "Pages", "skipped", "https://x/3"),
    ];
    let known = PullRequest { failed: vec!["other".into()], all_checks: Some(all), ..pr("open", Some("failing")) };
    let lines = check_lines(&known);
    assert_eq!(
        said(&lines),
        [(Sign::Failed, "tests"), (Sign::Pending, "build"), (Sign::Passed, "review-bot"), (Sign::Quiet, "deploy")]
    );
    let urls: Vec<&str> = lines.iter().map(|line| line.url.as_str()).collect();
    assert_eq!(urls, ["https://x/1", "https://x/2", "", "https://x/3"]);
}

#[test]
fn a_check_line_mutes_its_workflow_after_its_name() {
    let line = CheckLine::of_check(&check("tests (macos-15)", "CI", "failed", ""));
    let (text, muted) = line.text();
    assert_eq!(text, "tests (macos-15)  CI");
    assert_eq!(muted.map(|range| &text[range]), Some("CI"));
    assert_eq!(CheckLine::of_check(&check("review-bot", "", "passed", "")).text(), ("review-bot".into(), None));
}

#[test]
fn an_empty_list_says_there_are_no_checks() {
    let none =
        PullRequest { failed: vec!["stale".into()], all_checks: Some(Vec::new()), ..pr("open", Some("failing")) };
    assert_eq!(said(&check_lines(&none)), [(Sign::Quiet, "No checks")]);
}

#[test]
fn the_checks_chip_counts_every_check_once_known() {
    let known = PullRequest {
        all_checks: Some(vec![
            check("a", "", "failed", ""),
            check("b", "", "passed", ""),
            check("c", "", "passed", ""),
        ]),
        ..pr("open", Some("pending"))
    };
    assert_eq!((known.checks_said(), Sign::of_checks(known.rollup())), ("1 failed, 2 passed".into(), Sign::Failed));
}
