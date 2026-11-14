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
fn a_dialog_body_has_its_least_in_the_least_window() {
    assert_eq!(body_height(360.), 200.);
}

#[test]
fn a_dialog_body_starts_growing_past_a_window_of_440() {
    assert_eq!(body_height(439.), 200.);
    assert_eq!(body_height(440.), 200.);
    assert_eq!(body_height(441.), 201.);
}

#[test]
fn a_dialog_body_stops_growing_at_a_window_of_800() {
    assert_eq!(body_height(799.), 559.);
    assert_eq!(body_height(800.), 560.);
    assert_eq!(body_height(801.), 560.);
}

#[test]
fn a_dialog_body_in_a_usual_window_leaves_240_around_it() {
    assert_eq!(body_height(640.), 400.);
}

#[test]
fn a_window_of_no_height_still_gets_the_least_body() {
    assert_eq!(body_height(0.), 200.);
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
        ["start", "claude", "--focus", "--placed", "--prompt=fix it", "--worktree", "feat/x", "--account", "claude="]
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

#[test]
fn an_agent_started_with_a_prompt_of_several_lines_gets_it_whole() {
    assert_eq!(
        start_args("", "- fix it\nthen test", "", &[]),
        ["start", "--focus", "--placed", "--prompt=- fix it\nthen test"]
    );
}
