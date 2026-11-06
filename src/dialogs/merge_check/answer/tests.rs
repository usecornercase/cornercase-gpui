use super::*;
use crate::test_util::fixture;

fn answered(text: &str) -> Reading {
    let reading = reading_of(Ok(text.to_string()));
    assert!(matches!(reading, Reading::Answered(_)), "{reading:?}");
    reading
}

fn name(id: u64) -> String {
    match id {
        7 => "main".to_string(),
        18 => "mc/pricing".to_string(),
        _ => format!("workspace {id}"),
    }
}

fn shared(workspace: u64, files: &[&str]) -> Shared {
    Shared { workspace, count: files.len(), files: files.iter().map(ToString::to_string).collect() }
}

fn lines_of(reading: &Reading, pairs: &[Shared], shared_on: Option<bool>) -> Vec<Line> {
    reading.lines(&Facts { project: "shop", shared: pairs, shared_on, name: &name })
}

fn heading(text: &str) -> Line {
    Line::Heading(text.to_string(), None)
}

fn pair(text: &str, note: &str) -> Line {
    Line::Heading(text.to_string(), Some(note.to_string()))
}

fn file(path: &str) -> Line {
    Line::File(path.to_string())
}

fn note(text: &str) -> Line {
    Line::Note(text.to_string())
}

fn conflicts(text: &str) -> Line {
    Line::Conflicts(text.to_string())
}

fn conflict(path: &str, kind: &str) -> Line {
    Line::Conflict(path.to_string(), kind.to_string())
}

fn one_merge(merge: &Value) -> Reading {
    answered(&serde_json::json!({"workspace": 15, "checks": [merge]}).to_string())
}

fn many_conflicts(count: usize) -> Value {
    let conflicts: Vec<Value> =
        (0..count).map(|n| serde_json::json!({"path": format!("{n}.rs"), "kind": "contents"})).collect();
    serde_json::json!({"base": "main", "clean": false, "conflicts": conflicts})
}

#[test]
fn it_asks_cornercase_for_the_workspace_as_json() {
    assert_eq!(args(15), ["changes", "merge-check", "--workspace", "15", "--json"]);
}

#[test]
fn a_real_conflict_is_listed_under_what_it_merges_with() {
    let reading = answered(fixture!("merge_check/conflicts.json"));
    let pairs = [shared(7, &["README.md"]), shared(18, &["README.md", "notes.txt"])];
    assert_eq!(
        lines_of(&reading, &pairs, Some(true)),
        [
            heading("Into main"),
            conflicts("1 conflict"),
            conflict("README.md", "contents"),
            pair("With main", "1 shared file"),
            file("README.md"),
            conflicts("1 conflict"),
            conflict("README.md", "contents"),
            pair("With mc/pricing", "2 shared files"),
            file("README.md"),
            file("notes.txt"),
            conflicts("1 conflict"),
            conflict("README.md", "contents"),
        ]
    );
}

#[test]
fn a_clean_merge_says_so() {
    let reading = answered(fixture!("merge_check/clean.json"));
    assert_eq!(
        lines_of(&reading, &[], Some(true)),
        [
            heading("Into origin/main"),
            Line::Clean,
            Line::Apart("No other workspace of shop changes the same files.".into()),
        ]
    );
}

#[test]
fn a_branch_that_does_not_exist_is_that_checks_error() {
    let reading = answered(fixture!("merge_check/no_branch.json"));
    assert_eq!(
        lines_of(&reading, &[], None),
        [heading("Into nope"), Line::Failed("Could not check: no branch nope".into())]
    );
}

#[test]
fn no_base_at_all_says_where_to_choose_one() {
    let reading = answered(fixture!("merge_check/no_base.json"));
    assert_eq!(
        lines_of(&reading, &[], None),
        [
            heading("Into the base branch"),
            Line::Failed("Could not check: no base branch to merge into".into()),
            note("Choose a base in the Changes panel."),
        ]
    );
}

#[test]
fn nothing_shared_with_the_setting_off_says_no_other_workspace_was_looked_at() {
    let reading = answered(fixture!("merge_check/clean.json"));
    let said = lines_of(&reading, &[], Some(false));
    let off = "Shared files are off (Settings › Worktrees), so no other workspace was looked at.";
    assert_eq!(said.last(), Some(&Line::Apart(off.into())));
}

#[test]
fn nothing_shared_with_the_setting_unknown_claims_nothing() {
    let reading = answered(fixture!("merge_check/clean.json"));
    assert_eq!(lines_of(&reading, &[], None), [heading("Into origin/main"), Line::Clean]);
}

#[test]
fn while_it_checks_every_merge_says_so_and_the_pairs_are_already_listed() {
    let pairs = [shared(18, &["README.md"])];
    assert_eq!(
        lines_of(&Reading::Checking(None), &pairs, None),
        [
            heading("Into the base branch"),
            Line::Checking,
            pair("With mc/pricing", "1 shared file"),
            file("README.md"),
            Line::Checking,
        ]
    );
}

#[test]
fn checking_again_keeps_the_base_the_last_answer_named() {
    let again = answered(fixture!("merge_check/conflicts.json")).again();
    assert_eq!(lines_of(&again, &[], None), [heading("Into main"), Line::Checking]);
}

#[test]
fn checking_again_after_a_failure_names_no_base() {
    assert_eq!(Reading::Failed("gone".into()).again(), Reading::Checking(None));
}

#[test]
fn a_pair_that_appeared_after_the_check_has_no_verdict_until_checked_again() {
    let reading = answered(fixture!("merge_check/clean.json"));
    let said = lines_of(&reading, &[shared(18, &["README.md"])], Some(true));
    assert_eq!(said[2..], [pair("With mc/pricing", "1 shared file"), file("README.md")]);
}

#[test]
fn a_pair_that_stopped_sharing_after_the_check_keeps_its_verdict() {
    let reading = answered(fixture!("merge_check/conflicts.json"));
    let said = lines_of(&reading, &[shared(18, &["README.md"])], Some(true));
    assert_eq!(said.last(), Some(&conflict("README.md", "contents")));
    assert!(said.contains(&heading("With main")));
}

#[test]
fn past_twenty_conflicts_it_says_how_many_more() {
    let said = lines_of(&one_merge(&many_conflicts(23)), &[], None);
    assert_eq!(said[1], conflicts("23 conflicts"));
    assert_eq!(said[21], conflict("19.rs", "contents"));
    assert_eq!(said[22], file("… and 3 more"));
    assert_eq!(said.len(), 23);
}

#[test]
fn past_twenty_shared_files_it_counts_the_ones_view_left_out_too() {
    let files: Vec<String> = (0..100).map(|n| format!("{n}.rs")).collect();
    let pairs = [Shared { workspace: 18, count: 140, files }];
    let said = lines_of(&Reading::Failed("gone".into()), &pairs, None);
    assert_eq!(said[1], pair("With mc/pricing", "140 shared files"));
    assert_eq!(said[21], file("19.rs"));
    assert_eq!(said[22], file("… and 120 more"));
}

#[test]
fn untracked_files_the_snapshot_left_out_are_counted() {
    let reading = one_merge(&serde_json::json!({"base": "main", "clean": true, "left_out": 2}));
    assert_eq!(
        lines_of(&reading, &[], None),
        [heading("Into main"), Line::Clean, note("2 untracked files left out (over 1 MiB, or past 8 MiB in all)")]
    );
}

#[test]
fn a_refusal_is_told_in_cornercases_words() {
    let refused = anyhow::anyhow!("cornercase changes: Error: workspace 3 is not in a git repository");
    let reading = reading_of(Err(refused));
    assert_eq!(reading, Reading::Failed("workspace 3 is not in a git repository".into()));
    assert_eq!(lines_of(&reading, &[], None), [Line::Failed("workspace 3 is not in a git repository".into())]);
}

#[test]
fn an_answer_it_cannot_read_says_so() {
    let Reading::Failed(why) = reading_of(Ok("not json".to_string())) else { panic!("should fail") };
    assert!(why.starts_with("cannot read the answer of cornercase changes merge-check"), "{why}");
}

#[test]
fn checking_is_over_once_it_answers_or_fails() {
    assert!(Reading::Checking(None).checking());
    assert!(!answered(fixture!("merge_check/clean.json")).checking());
    assert!(!Reading::Failed("gone".into()).checking());
}

#[test]
fn the_agent_is_told_every_conflict_of_every_merge() {
    let reading = answered(fixture!("merge_check/conflicts.json"));
    assert_eq!(
        reading.ask_text(&name).as_deref(),
        Some(
            "the merge check found conflicts into main: README.md (contents); \
             with main: README.md (contents); with mc/pricing: README.md (contents) "
        )
    );
}

#[test]
fn the_agent_is_told_a_base_it_could_not_name_as_the_base_branch() {
    let reading = one_merge(&serde_json::json!({"conflicts": [{"path": "a.rs", "kind": "modify/delete"}]}));
    assert_eq!(
        reading.ask_text(&name).as_deref(),
        Some("the merge check found conflicts into the base branch: a.rs (modify/delete) ")
    );
}

#[test]
fn the_agent_is_told_how_many_conflicts_it_was_not_listed() {
    let text = one_merge(&many_conflicts(21)).ask_text(&name).expect("conflicts");
    assert!(text.ends_with("19.rs (contents), and 1 more "), "{text}");
}

#[test]
fn nothing_to_ask_without_a_conflict() {
    assert_eq!(answered(fixture!("merge_check/clean.json")).ask_text(&name), None);
    assert_eq!(answered(fixture!("merge_check/no_branch.json")).ask_text(&name), None);
    assert_eq!(Reading::Checking(None).ask_text(&name), None);
    assert_eq!(Reading::Failed("gone".into()).ask_text(&name), None);
}

#[test]
fn the_setting_is_read_from_the_settings_list() {
    let config: Value = serde_json::from_str(fixture!("settings/config.json")).expect("config");
    assert!(shared_files_on(&config).is_none(), "the older fixture has no such setting");
    let on = serde_json::json!({"settings": [{"key": "shared_files", "value": true}]});
    let off =
        serde_json::json!({"settings": [{"key": "tabs", "value": "top"}, {"key": "shared_files", "value": false}]});
    assert_eq!(shared_files_on(&on), Some(true));
    assert_eq!(shared_files_on(&off), Some(false));
}
