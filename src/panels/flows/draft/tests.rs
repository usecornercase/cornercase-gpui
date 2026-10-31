use serde_json::{Value, json};

use super::{Draft, Place, RoleDraft, StageDraft, fresh_role_name, parse_saved, removed_from, renamed_in, save_args};
use crate::panels::flows::model::parse_listing;

const LIST: &str = crate::test_util::fixture!("flows/list.json");
const SAVE_CHECK: &str = crate::test_util::fixture!("flows/save_check.json");

fn arena() -> Value {
    parse_listing(LIST).expect("list").flows[0].raw.clone()
}

#[test]
fn the_listing_keeps_each_flow_as_cornercase_wrote_it() {
    assert_eq!(arena()["name"], "Arena");
    assert!(arena()["orchestrator"].is_object());
}

#[test]
fn an_untouched_draft_gives_back_the_same_flow() {
    let raw = arena();
    assert_eq!(Draft::from_value(&raw).to_value(), Ok(raw));
}

#[test]
fn keys_the_form_does_not_show_are_kept() {
    let raw = json!({"name": "Q", "roles": [{"name": "a", "args": ["--x"], "env": {"K": "1"}}], "version": 1});
    let value = Draft::from_value(&raw).to_value().expect("value");
    assert_eq!(value["roles"][0]["args"], json!(["--x"]));
    assert_eq!(value["roles"][0]["env"], json!({"K": "1"}));
    assert_eq!(value["version"], 1);
}

#[test]
fn an_empty_model_or_effort_is_left_to_the_agent() {
    let mut role = RoleDraft::new("coder", "claude", true);
    role.model = " opus ".into();
    let value = role.to_value(false);
    assert_eq!((value["model"].clone(), value["effort"].clone()), (json!("opus"), Value::Null));
}

#[test]
fn a_role_with_its_own_worktree_says_so() {
    let mut role = RoleDraft::new("coder", "claude", true);
    role.own_worktree = true;
    assert_eq!(role.to_value(false)["worktree"], "own");
    assert_eq!(RoleDraft::new("c", "claude", false).to_value(false)["worktree"], "shared");
}

#[test]
fn the_orchestrator_keeps_its_own_edits_and_worktree() {
    let lead = RoleDraft::new("orchestrator", "claude", false).to_value(true);
    assert!(lead.get("edits").is_none() && lead.get("worktree").is_none());
}

#[test]
fn a_stage_lists_its_roles_from_commas() {
    let stage = StageDraft::with_roles(" coder,  reviewer ,");
    assert_eq!(stage.to_value(1), Ok(json!({"roles": ["coder", "reviewer"], "back": null})));
}

#[test]
fn a_loop_needs_numbers() {
    let stage = StageDraft {
        roles: "r".into(),
        back_to: "1".into(),
        when: "findings".into(),
        at_most: "x".into(),
        ..StageDraft::default()
    };
    assert_eq!(stage.to_value(2), Err("stage 2: at most takes a number".into()));
    let stage = StageDraft { at_most: "3".into(), ..stage };
    assert_eq!(stage.to_value(2).expect("stage")["back"], json!({"to": 1, "when": "findings", "at_most": 3}));
}

#[test]
fn a_blank_flow_can_be_saved_as_it_is() {
    let value = Draft::blank("New").to_value().expect("value");
    assert_eq!(value["stages"], json!([{"roles": ["coder"], "back": null}]));
    assert_eq!(value["end"], "commits");
}

#[test]
fn renaming_a_role_renames_it_in_the_stages() {
    assert_eq!(renamed_in("coder, reviewer", "coder", "writer"), Some("writer, reviewer".into()));
    assert_eq!(renamed_in("reviewer", "coder", "writer"), None);
}

#[test]
fn removing_a_role_takes_it_out_of_the_stages() {
    assert_eq!(removed_from("coder, reviewer", "coder"), Some("reviewer".into()));
    assert_eq!(removed_from("reviewer", "coder"), None);
}

#[test]
fn a_new_role_takes_a_free_name() {
    assert_eq!(fresh_role_name(&["role 1".into(), "coder".into()]), "role 2");
}

#[test]
fn saving_a_file_names_its_path() {
    assert_eq!(
        save_args(&Place::File("/f/q.toml".into()), Some(2), false),
        ["flow", "save", "-", "--json", "--path", "/f/q.toml", "--project", "2"]
    );
}

#[test]
fn a_new_project_flow_goes_to_the_project_and_a_check_saves_nothing() {
    assert_eq!(save_args(&Place::Project, None, true), ["flow", "save", "-", "--json", "--to", "project", "--check"]);
    assert_eq!(save_args(&Place::Yours, None, false), ["flow", "save", "-", "--json"]);
}

#[test]
fn a_check_gives_the_problems_of_the_draft() {
    let saved = parse_saved(SAVE_CHECK).expect("saved");
    assert!(!saved.written);
    assert_eq!(saved.problems.len(), 2);
    assert_eq!(saved.problems[1], "`ghost` is in no stage");
}
