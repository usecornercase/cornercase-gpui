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

#[test]
fn failed_checks_are_listed_by_name() {
    let failing = PullRequest { failed: vec!["test".into(), "clippy".into()], ..pr("open", Some("failing")) };
    let names: Vec<String> = check_lines(&failing).into_iter().map(|(_, name)| name).collect();
    assert_eq!(names, ["test", "clippy"]);
}

#[test]
fn without_failures_one_line_says_how_the_checks_went() {
    assert_eq!(check_lines(&pr("open", Some("pending"))), [(Sign::Pending, "Checks are running".to_string())]);
    assert_eq!(check_lines(&pr("open", Some("passing"))), [(Sign::Passed, "Every check passed".to_string())]);
    assert_eq!(check_lines(&pr("open", None)), [(Sign::Quiet, "No checks".to_string())]);
    let gitlab = PullRequest { provider: "gitlab".into(), ..pr("open", Some("passing")) };
    assert_eq!(check_lines(&gitlab), [(Sign::Passed, "The pipeline passed".to_string())]);
}
