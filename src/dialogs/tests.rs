use super::*;

#[test]
fn a_dialog_body_grows_with_the_window_within_limits() {
    assert_eq!(body_height(800.), 560.);
    assert_eq!(body_height(600.), 360.);
    assert_eq!(body_height(300.), 200.);
    assert_eq!(body_height(1400.), 560.);
}

#[test]
fn an_agent_started_without_a_pick_names_no_account() {
    assert_eq!(start_args("", "", "", &[]), ["start", "--focus"]);
}

#[test]
fn an_agent_started_with_a_pick_names_it_with_the_rest() {
    let picks = [AccountPick { agent: "claude".into(), folder: String::new() }];
    assert_eq!(
        start_args("claude", "fix it", "feat/x", &picks),
        ["start", "claude", "--focus", "--prompt", "fix it", "--worktree", "feat/x", "--account", "claude="]
    );
}
