use super::*;

fn pull(state: &str, checks: Option<&str>) -> PullRequest {
    PullRequest {
        number: 485,
        title: "Dark mode".into(),
        url: "https://github.com/acme/app/pull/485".into(),
        state: state.into(),
        checks: checks.map(String::from),
        base: Some("main".into()),
        provider: "github".into(),
        ..Default::default()
    }
}

fn layer(number: u64, state: &str, checks: Option<&str>) -> Layer {
    Layer {
        number,
        title: format!("Layer {number}"),
        url: format!("https://github.com/acme/app/pull/{number}"),
        state: state.into(),
        checks: checks.map(String::from),
        ..Default::default()
    }
}

fn stacked(layers: Vec<Layer>, position: usize) -> PullRequest {
    let size = layers.len();
    PullRequest {
        stack: Some(Stack { number: 480, base: "main".into(), position, size, layers }),
        ..pull("open", None)
    }
}

#[test]
fn a_stack_reads_from_the_session() {
    let text = r#"{"number":485,"provider":"github","stack":{"number":480,"base":"main","position":2,"size":2,
        "layers":[{"number":483,"title":"Tokens","url":"u483","state":"merged","checks":"passing",
        "conflicts":false,"review":"approved"},{"number":485,"state":"open","checks":null}]}}"#;
    let pr: PullRequest = serde_json::from_str(text).expect("valid JSON");
    let stack = pr.stack.expect("a stack");
    assert_eq!((stack.position, stack.size, stack.layers.len()), (2, 2, 2));
    assert_eq!(stack.layers[0].review.as_deref(), Some("approved"));
}

#[test]
fn the_summary_says_the_state_the_conflict_and_the_checks() {
    let mut pr = pull("open", Some("failing"));
    assert_eq!(pr.summary(), "open · checks failing");
    pr.conflicts = true;
    assert_eq!(pr.summary(), "open · conflicts with main · checks failing");
    pr.base = None;
    pr.checks = None;
    assert_eq!(pr.summary(), "open · conflicts with its base · no checks");
}

#[test]
fn a_merge_request_uses_gitlab_words() {
    let pr =
        PullRequest { provider: "gitlab".into(), number: 486, checks: Some("passing".into()), ..pull("draft", None) };
    assert_eq!(
        (pr.heading(), pr.label(), pr.site()),
        ("Merge request !486".into(), "merge request !486".into(), "GitLab")
    );
    assert_eq!(pr.summary(), "draft · pipeline passing");
}

#[test]
fn an_older_server_without_a_provider_reads_as_github() {
    let pr = PullRequest { provider: String::new(), ..pull("open", None) };
    assert_eq!(pr.heading(), "Pull request #485");
}

#[test]
fn the_agent_is_told_what_conflicts_and_what_failed() {
    let mut pr = pull("open", Some("failing"));
    pr.failed = vec!["test (macos)".into(), "clippy".into()];
    pr.conflicts = true;
    assert_eq!(
        pr.problems().as_deref(),
        Some(
            "pull request #485 has merge conflicts with main and failed its checks: test (macos), clippy \
             (https://github.com/acme/app/pull/485) "
        )
    );
}

#[test]
fn nothing_is_asked_without_problems_or_once_merged() {
    assert_eq!(pull("open", Some("passing")).problems(), None);
    let merged = PullRequest { failed: vec!["test".into()], ..pull("merged", Some("failing")) };
    assert_eq!(merged.problems(), None);
}

#[test]
fn rungs_go_top_first_with_their_positions() {
    let pr = stacked(vec![layer(483, "merged", None), layer(485, "open", None), layer(486, "open", None)], 2);
    let stack = pr.stack.as_ref().expect("a stack");
    let rungs: Vec<(usize, u64, bool)> = stack.rungs(485).iter().map(|r| (r.position, r.layer.number, r.own)).collect();
    assert_eq!(rungs, [(3, 486, false), (2, 485, true), (1, 483, false)]);
}

#[test]
fn the_lowest_failing_or_conflicting_layer_below_blocks() {
    let mut conflicting = layer(483, "open", Some("passing"));
    conflicting.conflicts = true;
    let layers = vec![layer(481, "merged", Some("failing")), layer(482, "open", Some("failing")), conflicting];
    let pr = stacked([layers, vec![layer(485, "open", None)]].concat(), 4);
    let (blocker, block) = pr.blocked().expect("a blocker");
    assert_eq!((blocker.number, block), (482, Block::Checks));
}

#[test]
fn changes_requested_block_only_when_nothing_else_does() {
    let mut reviewed = layer(482, "open", None);
    reviewed.review = Some("changes_requested".into());
    let pr = stacked(vec![reviewed.clone(), layer(485, "open", None)], 2);
    assert_eq!(pr.blocked().map(|(l, b)| (l.number, b)), Some((482, Block::Review)));
    let mut conflicting = layer(483, "draft", None);
    conflicting.conflicts = true;
    let pr = stacked(vec![reviewed, conflicting, layer(485, "open", None)], 3);
    assert_eq!(pr.blocked().map(|(l, b)| (l.number, b)), Some((483, Block::Conflicts)));
}

#[test]
fn layers_above_and_a_settled_pull_request_are_never_blocked() {
    let pr = stacked(vec![layer(485, "open", None), layer(486, "open", Some("failing"))], 1);
    assert!(pr.blocked().is_none());
    let merged = PullRequest { state: "merged".into(), ..stacked(vec![layer(484, "open", Some("failing"))], 2) };
    assert!(merged.blocked().is_none());
}

#[test]
fn the_nearest_merged_layer_below_is_named() {
    let layers = vec![layer(481, "merged", None), layer(482, "merged", None), layer(485, "open", None)];
    let pr = stacked(layers, 3);
    let stack = pr.stack.as_ref().expect("a stack");
    assert_eq!(stack.merged_below(485).map(|l| l.number), Some(482));
    assert!(stack.merged_below(481).is_none());
}

#[test]
fn a_layer_is_held_by_the_workspace_on_its_branch_and_this_one_first() {
    let text = r#"{"projects":[{"id":1,"workspaces":[
        {"id":2,"name":"tokens","pull_request":{"number":483,"url":"https://x/483"}},
        {"id":3,"name":"dark","pull_request":{"number":485,"url":"https://x/485"}},
        {"id":4,"name":"again","pull_request":{"number":485,"url":"https://x/485"}}]}]}"#;
    let session: Session = serde_json::from_str(text).expect("valid JSON");
    let holder = |workspace, url| session.pull_request_holder(workspace, url).map(|w| w.id);
    assert_eq!(holder(4, "https://x/485"), Some(4));
    assert_eq!(holder(3, "https://x/483"), Some(2));
    assert_eq!(holder(3, "https://x/999"), None);
    assert_eq!(holder(3, ""), None);
}
