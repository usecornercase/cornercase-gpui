use serde_json::{Value, json};

use super::{
    Draft, LoopAfter, Place, RoleDraft, StageDraft, fresh_role_name, gone_note, has_breaks, loop_after, neighbour,
    only_role_in, parse_saved, removed_from, renamed_in, save_args, split_words, swapped, without,
};
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
    let raw = json!({"name": "Q", "roles": [{"name": "a", "later": "kept"}], "version": 1});
    let value = Draft::from_value(&raw).to_value().expect("value");
    assert_eq!(value["roles"][0]["later"], "kept");
    assert_eq!(value["version"], 1);
}

#[test]
fn arguments_come_back_as_they_were_however_they_are_quoted() {
    let args = json!(["--plain", "two words", "it's", "", "a\\b", "line\nbreak"]);
    let role = RoleDraft::from_value(&json!({"name": "a", "args": args}));
    assert_eq!(role.args, "--plain 'two words' 'it'\\''s' '' 'a\\b' 'line\nbreak'");
    assert_eq!(role.to_value(false).expect("role")["args"], args);
}

#[test]
fn variables_come_back_as_they_were_however_they_are_quoted() {
    let env = json!({"EMPTY": "", "KEY": "1", "PLACE": "the big house"});
    let role = RoleDraft::from_value(&json!({"name": "a", "env": env}));
    assert_eq!(role.env, "EMPTY= KEY=1 'PLACE=the big house'");
    assert_eq!(role.to_value(false).expect("role")["env"], env);
}

#[test]
fn a_variable_without_an_equals_sign_is_refused_by_name() {
    let role = RoleDraft { env: "KEY".into(), ..RoleDraft::new("coder", "claude", true) };
    assert_eq!(role.to_value(false), Err("coder: variables are KEY=value, and `KEY` has no =".into()));
}

#[test]
fn a_variable_without_a_name_is_refused() {
    let role = RoleDraft { env: "=1".into(), ..RoleDraft::new("coder", "claude", true) };
    assert!(role.to_value(false).is_err());
}

#[test]
fn words_split_at_blanks_outside_quotes() {
    assert_eq!(split_words("--a  'b c' \"d e\" f\\ g"), ["--a", "b c", "d e", "f g"]);
}

#[test]
fn an_empty_field_has_no_words_and_empty_quotes_are_a_word() {
    assert_eq!(split_words("  "), Vec::<String>::new());
    assert_eq!(split_words("''"), [""]);
}

#[test]
fn an_open_quote_runs_to_the_end_of_the_field() {
    assert_eq!(split_words("--a 'b c"), ["--a", "b c"]);
}

#[test]
fn an_empty_model_or_effort_is_left_to_the_agent() {
    let mut role = RoleDraft::new("coder", "claude", true);
    role.model = " opus ".into();
    let value = role.to_value(false).expect("role");
    assert_eq!((value["model"].clone(), value["effort"].clone()), (json!("opus"), Value::Null));
}

#[test]
fn a_role_with_its_own_worktree_says_so() {
    let mut role = RoleDraft::new("coder", "claude", true);
    role.own_worktree = true;
    assert_eq!(role.to_value(false).expect("role")["worktree"], "own");
    assert_eq!(RoleDraft::new("c", "claude", false).to_value(false).expect("role")["worktree"], "shared");
}

#[test]
fn the_orchestrator_keeps_its_own_edits_and_worktree() {
    let lead = RoleDraft::new("orchestrator", "claude", false).to_value(true).expect("role");
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
fn a_stage_of_one_role_is_only_that_role() {
    assert!(only_role_in("coder", "coder"));
    assert!(only_role_in(" coder, ", "coder"));
}

#[test]
fn a_stage_it_shares_or_an_empty_one_is_not_only_that_role() {
    assert!(!only_role_in("coder, reviewer", "coder"));
    assert!(!only_role_in("", "coder"));
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

#[test]
fn moving_a_stage_keeps_its_loops_pointing_at_the_same_stages() {
    let moved = swapped(2, 3);
    assert_eq!(loop_after("1", 2, &moved), LoopAfter::Same);
    assert_eq!(loop_after("1", 3, &moved), LoopAfter::Same);
    assert_eq!(loop_after("2", 4, &moved), LoopAfter::To("3".into()));
    assert_eq!(loop_after("3", 4, &moved), LoopAfter::To("2".into()));
}

#[test]
fn a_stage_moved_above_the_stage_it_goes_back_to_loses_its_loop() {
    assert_eq!(loop_after("1", 1, swapped(1, 2)), LoopAfter::Gone);
}

#[test]
fn a_loop_follows_the_stage_it_goes_back_to_when_that_stage_moves() {
    assert_eq!(loop_after("1", 3, swapped(1, 2)), LoopAfter::To("2".into()));
}

#[test]
fn a_target_moved_below_its_loop_drops_the_loop() {
    assert_eq!(loop_after("2", 2, swapped(2, 3)), LoopAfter::Gone);
}

#[test]
fn removing_a_stage_renumbers_the_loops_after_it() {
    assert_eq!(loop_after("3", 3, without(2)), LoopAfter::To("2".into()));
}

#[test]
fn removing_a_later_stage_leaves_a_loop_alone() {
    assert_eq!(loop_after("1", 2, without(3)), LoopAfter::Same);
}

#[test]
fn removing_the_stage_a_loop_goes_back_to_drops_the_loop() {
    assert_eq!(loop_after("2", 2, without(2)), LoopAfter::Gone);
}

#[test]
fn a_stage_without_a_loop_or_with_a_half_typed_one_is_left_alone() {
    assert_eq!(loop_after("", 2, swapped(1, 2)), LoopAfter::Same);
    assert_eq!(loop_after("x", 2, swapped(1, 2)), LoopAfter::Same);
}

#[test]
fn a_loop_back_to_stage_zero_is_dropped() {
    assert_eq!(loop_after("0", 2, swapped(1, 2)), LoopAfter::Gone);
}

#[test]
fn a_stage_moves_to_the_one_beside_it() {
    assert_eq!(neighbour(1, true, 3), Some(0));
    assert_eq!(neighbour(1, false, 3), Some(2));
}

#[test]
fn the_first_stage_cannot_go_up_nor_the_last_go_down() {
    assert_eq!(neighbour(0, true, 3), None);
    assert_eq!(neighbour(2, false, 3), None);
}

#[test]
fn a_stage_that_is_gone_does_not_move() {
    assert_eq!(neighbour(5, true, 3), None);
}

#[test]
fn losing_no_loop_says_nothing() {
    assert_eq!(gone_note(&[]), None);
}

#[test]
fn a_lost_loop_is_named_by_its_stage() {
    assert_eq!(gone_note(&[2]).as_deref(), Some("Stage 2 lost its loop: it no longer goes back to an earlier stage"));
}

#[test]
fn several_lost_loops_are_listed() {
    assert_eq!(
        gone_note(&[1, 2, 4]).as_deref(),
        Some("Stages 1, 2 and 4 lost their loops: they no longer go back to an earlier stage")
    );
}

#[test]
fn only_line_breaks_ask_for_a_field_of_several_lines() {
    assert!(has_breaks("first\nsecond"));
    assert!(!has_breaks("a long sentence"));
}

#[test]
fn the_plan_of_a_draft_follows_its_stages() {
    let mut draft = Draft::blank("New");
    draft.stages.push(StageDraft {
        roles: "coder".into(),
        back_to: "1".into(),
        at_most: "2".into(),
        ..StageDraft::default()
    });
    let plan = draft.plan().expect("plan");
    assert_eq!(plan.stages.len(), 2);
    assert_eq!(plan.stages[1].back.as_ref().map(|back| back.to), Some(1));
}

#[test]
fn a_draft_that_cannot_be_read_has_no_plan() {
    let mut draft = Draft::blank("New");
    draft.stages[0].back_to = "soon".into();
    assert_eq!(draft.plan(), None);
}
