use serde_json::{Value, json};

use super::{
    Back, BackDraft, Draft, LoopAfter, Place, RoleDraft, Stage, StageDraft, alone, first_line, follow_loops,
    fresh_role_name, gone_note, has_breaks, loop_after, move_role, neighbour, parse_saved, place_note, remove_role,
    save_args, saves_line, seat, split_words, swapped, targets, without,
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
fn a_blank_flow_can_be_saved_as_it_is() {
    let value = Draft::blank("New").to_value().expect("value");
    assert_eq!(value["stages"], json!([{"roles": ["coder"], "back": null}]));
    assert_eq!(value["end"], "commits");
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
    assert_eq!(loop_after(1, 2, &moved), LoopAfter::Same);
    assert_eq!(loop_after(1, 3, &moved), LoopAfter::Same);
    assert_eq!(loop_after(2, 4, &moved), LoopAfter::To(3));
    assert_eq!(loop_after(3, 4, &moved), LoopAfter::To(2));
}

#[test]
fn a_stage_moved_above_the_stage_it_goes_back_to_loses_its_loop() {
    assert_eq!(loop_after(1, 1, swapped(1, 2)), LoopAfter::Gone);
}

#[test]
fn a_loop_follows_the_stage_it_goes_back_to_when_that_stage_moves() {
    assert_eq!(loop_after(1, 3, swapped(1, 2)), LoopAfter::To(2));
}

#[test]
fn a_target_moved_below_its_loop_drops_the_loop() {
    assert_eq!(loop_after(2, 2, swapped(2, 3)), LoopAfter::Gone);
}

#[test]
fn removing_a_stage_renumbers_the_loops_after_it() {
    assert_eq!(loop_after(3, 3, without(2)), LoopAfter::To(2));
}

#[test]
fn removing_a_later_stage_leaves_a_loop_alone() {
    assert_eq!(loop_after(1, 2, without(3)), LoopAfter::Same);
}

#[test]
fn removing_the_stage_a_loop_goes_back_to_drops_the_loop() {
    assert_eq!(loop_after(2, 2, without(2)), LoopAfter::Gone);
}

#[test]
fn a_loop_back_to_stage_zero_is_dropped() {
    assert_eq!(loop_after(0, 2, swapped(1, 2)), LoopAfter::Gone);
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
        roles: vec!["coder".into()],
        back: Some(BackDraft { to: 1, when: String::new(), at_most: 2 }),
        ..StageDraft::default()
    });
    let plan = draft.plan().expect("plan");
    assert_eq!(plan.stages.len(), 2);
    assert_eq!(plan.stages[1].back.as_ref().map(|back| back.to), Some(1));
}

#[test]
fn a_draft_that_cannot_be_read_has_no_plan() {
    let mut draft = Draft::blank("New");
    draft.roles[0].env = "KEY".into();
    assert_eq!(draft.plan(), None);
}

fn names(list: &[&str]) -> Vec<(u64, String)> {
    list.iter().enumerate().map(|(id, name)| (id as u64, (*name).to_string())).collect()
}

fn stage(roles: &[u64], back: Option<(usize, u32)>) -> Stage<String> {
    Stage {
        base: Value::Null,
        roles: roles.to_vec(),
        back: back.map(|(to, at_most)| Back { to, at_most }),
        when: "problems".into(),
    }
}

fn draft_of(roles: &[&str], stages: &Value) -> Draft {
    let roles: Vec<Value> = roles.iter().map(|name| json!({"name": name, "agent": "claude"})).collect();
    Draft::from_value(&json!({"name": "Q", "orchestrator": {"name": "orchestrator"}, "roles": roles, "stages": stages}))
}

fn ids(stages: &[Stage<String>]) -> Vec<Vec<u64>> {
    stages.iter().map(|stage| stage.roles.clone()).collect()
}

#[test]
fn a_flow_cornercase_accepts_is_seated_as_it_is() {
    let stages = json!([
        {"roles": ["coder"], "back": null},
        {"roles": ["reviewer", "security"], "back": {"to": 1, "when": "problems", "at_most": 3}},
        {"roles": ["tester"], "back": {"to": 1, "when": "bugs", "at_most": 2}}
    ]);
    let seating = seat(&draft_of(&["coder", "reviewer", "security", "tester"], &stages));
    assert_eq!(ids(&seating.stages), [vec![0], vec![1, 2], vec![3]]);
    assert_eq!(seating.stages[1].back, Some(Back { to: 1, at_most: 3 }));
    assert_eq!(seating.stages[2].when, "bugs");
    assert_eq!(seating.note(), None);
    let arena = Draft::from_value(&arena());
    let seating = seat(&arena);
    assert_eq!(seating.stages.len(), arena.stages.len());
    assert_eq!(seating.changes, Vec::<String>::new());
}

#[test]
fn a_role_no_stage_names_gets_a_stage_of_its_own() {
    let seating = seat(&draft_of(&["coder", "tester"], &json!([{"roles": ["coder"], "back": null}])));
    assert_eq!(ids(&seating.stages), [vec![0], vec![1]]);
    assert_eq!(seating.note().as_deref(), Some("Opened with changes: tester had no stage, so it has one of its own"));
}

#[test]
fn a_role_named_twice_stays_in_its_first_stage() {
    let stages = json!([{"roles": ["coder"]}, {"roles": ["reviewer", "coder"]}]);
    let seating = seat(&draft_of(&["coder", "reviewer"], &stages));
    assert_eq!(ids(&seating.stages), [vec![0], vec![1]]);
    assert_eq!(seating.changes, ["coder was in stage 1 and stage 2, so it stays in stage 1"]);
    let seating = seat(&draft_of(&["coder"], &json!([{"roles": ["coder", "coder"]}])));
    assert_eq!(seating.changes, ["stage 1 named coder twice, so it is there once"]);
}

#[test]
fn a_name_no_role_has_leaves_its_stage() {
    let seating = seat(&draft_of(&["coder"], &json!([{"roles": ["coder", "ghost"]}])));
    assert_eq!(ids(&seating.stages), [vec![0]]);
    assert_eq!(seating.changes, ["stage 1 named ghost, which is not a role, so it is left out"]);
}

#[test]
fn a_stage_left_empty_goes_and_its_loops_follow() {
    let back = |to| json!({"to": to, "when": "problems", "at_most": 3});
    let stages = json!([
        {"roles": ["a"]},
        {"roles": ["ghost"]},
        {"roles": ["b"], "back": back(1)},
        {"roles": ["c"], "back": back(2)},
        {"roles": ["d"], "back": back(3)}
    ]);
    let seating = seat(&draft_of(&["a", "b", "c", "d"], &stages));
    assert_eq!(ids(&seating.stages), [vec![0], vec![1], vec![2], vec![3]]);
    let loops: Vec<Option<Back>> = seating.stages.iter().map(|stage| stage.back).collect();
    assert_eq!(loops, [None, Some(Back { to: 1, at_most: 3 }), None, Some(Back { to: 2, at_most: 3 })]);
    assert!(seating.changes.contains(&"stage 2 had no roles, so it is gone, and so is the loop of stage 4".into()));
}

#[test]
fn moving_a_role_out_of_a_stage_it_was_alone_in_removes_the_stage() {
    let mut stages = vec![stage(&[0], None), stage(&[1], None), stage(&[2], Some((1, 3)))];
    assert_eq!(move_role(&mut stages, 0, 1), [2]);
    assert_eq!(ids(&stages), [vec![1, 0], vec![2]]);
    assert_eq!(stages[1].back, None);
}

#[test]
fn moving_a_role_keeps_the_order_of_the_roles() {
    let mut stages = vec![stage(&[0, 3], None), stage(&[1, 2], None)];
    let roles = names(&["coder", "reviewer", "security", "tester"]);
    assert_eq!(move_role(&mut stages, 3, 1), Vec::<usize>::new());
    assert_eq!(ids(&stages), [vec![0], vec![1, 2, 3]]);
    let drafted = stages[1].draft(&roles, "problems".into());
    assert_eq!(drafted.roles, ["reviewer", "security", "tester"]);
    assert_eq!(roles, names(&["coder", "reviewer", "security", "tester"]));
}

#[test]
fn a_role_alone_in_its_stage_cannot_ask_for_a_new_one() {
    let stages = vec![stage(&[0], None), stage(&[1, 2], None)];
    assert!(alone(&stages, 0));
    assert!(!alone(&stages, 1));
}

#[test]
fn a_new_loop_is_cornercases_default() {
    let default = BackDraft::default();
    assert_eq!(default, BackDraft { to: 1, when: "problems".into(), at_most: 3 });
    let stage = StageDraft { roles: vec!["a".into()], back: Some(default), ..StageDraft::default() };
    let mut draft = Draft::blank("New");
    draft.stages = vec![stage];
    let value = draft.to_value().expect("value");
    assert_eq!(value["stages"][0]["back"], json!({"to": 1, "when": "problems", "at_most": 3}));
}

#[test]
fn a_loop_offers_only_earlier_stages() {
    assert_eq!(targets(2).collect::<Vec<_>>(), [1, 2]);
    assert_eq!(targets(0).count(), 0);
}

#[test]
fn a_loop_cannot_go_under_once() {
    let back = Back { to: 1, at_most: 1 };
    assert!(back.step(false).lowest());
    assert_eq!(back.step(false).at_most, 1);
    assert_eq!(back.step(true).at_most, 2);
    assert_eq!(Back { at_most: u32::MAX, ..back }.step(true).at_most, u32::MAX);
}

#[test]
fn the_orchestrators_name_is_left_as_it_was() {
    let raw = json!({"orchestrator": {"name": "boss", "agent": "claude"}});
    let mut draft = Draft::from_value(&raw);
    draft.orchestrator.name = "other".into();
    assert_eq!(draft.to_value().expect("value")["orchestrator"]["name"], "boss");
    assert_eq!(Draft::blank("New").to_value().expect("value")["orchestrator"]["name"], "orchestrator");
}

#[test]
fn a_stage_without_a_loop_is_left_alone() {
    let mut stages = vec![stage(&[0], None), stage(&[1], None)];
    assert_eq!(follow_loops(&mut stages, swapped(1, 2)), Vec::<usize>::new());
    assert_eq!(stages[1].back, None);
}

#[test]
fn removing_the_last_role_of_a_stage_removes_the_stage_and_its_loops_follow() {
    let mut stages = vec![stage(&[0], None), stage(&[1], None), stage(&[2], Some((2, 3)))];
    assert_eq!(remove_role(&mut stages, 1), [2]);
    assert_eq!(ids(&stages), [vec![0], vec![2]]);
}

#[test]
fn a_closed_role_shows_the_first_line_of_its_prompt() {
    assert_eq!(first_line("\n  \n Fix it \nthen test"), Some("Fix it"));
    assert_eq!(first_line(" \n"), None);
}

#[test]
fn a_role_is_drawn_as_it_is_typed() {
    let role = RoleDraft { env: "KEY A=1".into(), model: " ".into(), ..RoleDraft::new(" coder ", "claude", true) };
    let spec = role.spec();
    assert_eq!(spec.name, "coder");
    assert_eq!(spec.model, None);
    assert_eq!(spec.env.keys().collect::<Vec<_>>(), ["A"]);
}

#[test]
fn the_header_says_where_it_saves() {
    assert_eq!(saves_line(&Place::File("/f/q.toml".into()), None), "Saves /f/q.toml");
    assert_eq!(saves_line(&Place::Yours, None), "Saves into your flows");
    assert_eq!(saves_line(&Place::Project, None), "Saves into this project's flows");
    assert_eq!(saves_line(&Place::Yours, Some("It is built in")), "Saves into your flows · It is built in");
    assert!(place_note(&Place::Project).starts_with("Kept in the project's"));
}
