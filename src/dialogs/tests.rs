use super::*;
use crate::test_util::shop_on_vps;

#[test]
fn a_workspace_on_a_machine_names_it_and_its_project() {
    assert_eq!(
        workspace_args_there("notes", false, &shop_on_vps()),
        ["new-workspace", "notes", "--machine", "vps", "--project", "8", "--json"]
    );
}

#[test]
fn a_worktree_on_a_machine_says_so() {
    assert_eq!(
        workspace_args_there("feat/x", true, &shop_on_vps()),
        ["new-workspace", "feat/x", "--machine", "vps", "--project", "8", "--worktree", "--json"]
    );
}

#[test]
fn a_dialog_body_grows_with_the_window_within_limits() {
    assert_eq!(body_height(800.), 560.);
    assert_eq!(body_height(600.), 360.);
    assert_eq!(body_height(300.), 200.);
    assert_eq!(body_height(1400.), 560.);
}

#[test]
fn an_agent_started_without_a_pick_names_no_account() {
    assert_eq!(start_args("", "", "", &[]), ["start", "--focus", "--placed"]);
}

#[test]
fn an_agent_started_with_a_pick_names_it_with_the_rest() {
    let picks = [AccountPick { agent: "claude".into(), folder: String::new() }];
    assert_eq!(
        start_args("claude", "fix it", "feat/x", &picks),
        [
            "start",
            "claude",
            "--focus",
            "--placed",
            "--prompt",
            "fix it",
            "--worktree",
            "feat/x",
            "--account",
            "claude="
        ]
    );
}

#[test]
fn the_default_agent_answers_once_its_tab_is_open() {
    assert_eq!(start_in(&[]), ["start", "--focus", "--placed"]);
}

#[test]
fn an_agent_started_in_a_place_names_it() {
    assert_eq!(start_in(&["--project", "4"]), ["start", "--project", "4", "--focus", "--placed"]);
    assert_eq!(start_in(&["--workspace", "7"]), ["start", "--workspace", "7", "--focus", "--placed"]);
}
