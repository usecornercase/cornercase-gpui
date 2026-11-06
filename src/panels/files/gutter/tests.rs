use crate::test_util::fixture;

use super::*;

fn gutter() -> Gutter {
    Gutter::parse(fixture!("files/changes-file.json")).expect("a change")
}

fn hunk(new_start: usize, lines: &[(Kind, Option<usize>, &str)]) -> Hunk {
    let line = |(kind, new, text): &(Kind, Option<usize>, &str)| Line { kind: *kind, new: *new, text: (*text).into() };
    Hunk { new_start, lines: lines.iter().map(line).collect() }
}

#[test]
fn a_changed_line_is_marked_and_keeps_what_it_replaced() {
    let gutter = gutter();
    let block = gutter.at(1).expect("line 2 changed");
    assert_eq!(block.mark, Mark::Changed);
    assert_eq!(block.old, ["two"]);
    assert_eq!(block.before, 1);
}

#[test]
fn added_lines_are_marked_as_one_block_with_nothing_old() {
    let gutter = gutter();
    let (first, second) = (gutter.at(3).expect("line 4"), gutter.at(4).expect("line 5"));
    assert_eq!(first, second);
    assert_eq!(first.mark, Mark::Added);
    assert_eq!(first.old, Vec::<String>::new());
}

#[test]
fn lines_removed_mark_the_line_above_them() {
    let gutter = gutter();
    let block = gutter.at(5).expect("line 6 had lines removed after it");
    assert_eq!(block.mark, Mark::Removed);
    assert_eq!(block.old, ["five"]);
    assert_eq!(block.before, 6);
}

#[test]
fn an_unchanged_line_has_no_mark() {
    let gutter = gutter();
    assert_eq!(gutter.at(0), None);
    assert_eq!(gutter.at(2), None);
    assert_eq!(gutter.at(99), None);
}

#[test]
fn old_lines_are_found_by_the_line_they_sit_above() {
    let gutter = gutter();
    assert_eq!(gutter.before(6).map(|block| block.old.clone()), Some(vec!["five".to_string()]));
    assert_eq!(gutter.before(3), None);
}

#[test]
fn lines_removed_above_the_first_line_mark_its_top() {
    let blocks = blocks_of(&hunk(1, &[(Kind::Removed, None, "gone"), (Kind::Context, Some(1), "kept")]));
    assert_eq!(blocks, [Block { lines: 0..1, mark: Mark::RemovedAbove, before: 0, old: vec!["gone".to_string()] }]);
}

#[test]
fn lines_removed_at_the_end_sit_past_the_last_line() {
    let blocks = blocks_of(&hunk(1, &[(Kind::Context, Some(1), "kept"), (Kind::Removed, None, "gone")]));
    assert_eq!(blocks, [Block { lines: 0..1, mark: Mark::Removed, before: 1, old: vec!["gone".to_string()] }]);
}

#[test]
fn a_new_file_is_marked_whole_without_reading_its_lines() {
    let gutter = Gutter::parse(fixture!("files/changes-file-new.json")).expect("a change");
    assert_eq!(gutter.at(0).map(|block| block.mark), Some(Mark::Added));
    assert_eq!(gutter.at(50_000).map(|block| block.mark), Some(Mark::Added));
}

#[test]
fn an_answer_that_is_no_diff_gives_no_gutter() {
    assert_eq!(Gutter::parse("Error: there is no change"), None);
}

#[test]
fn a_line_of_a_kind_this_version_does_not_know_counts_as_context() {
    let json =
        r#"{"status": "modified", "hunks": [{"new_start": 1, "lines": [{"kind": "moved", "new": 1, "text": "a"}]}]}"#;
    assert_eq!(Gutter::parse(json), Some(Gutter::default()));
}

#[test]
fn a_file_with_no_change_is_not_asked_about() {
    let mut margin = Margin::default();
    assert_eq!(margin.follow(None, true), Follow::Nothing);
}

#[test]
fn a_file_that_became_changed_is_asked_about() {
    let mut margin = Margin::default();
    assert_eq!(margin.follow(Some(FileStatus::Modified), false), Follow::Read(1));
}

#[test]
fn the_same_status_is_not_asked_about_again_unless_it_is_time() {
    let mut margin = Margin::default();
    margin.follow(Some(FileStatus::Modified), false);
    assert_eq!(margin.follow(Some(FileStatus::Modified), false), Follow::Nothing);
    assert_eq!(margin.follow(Some(FileStatus::Modified), true), Follow::Read(2));
}

#[test]
fn a_file_that_is_no_longer_changed_drops_its_marks_and_a_late_answer() {
    let mut margin = Margin::default();
    let Follow::Read(asked) = margin.follow(Some(FileStatus::Modified), false) else { panic!("a read") };
    margin.answered(asked, Some(gutter()));
    margin.unfolded.insert(6);
    let Follow::Read(late) = margin.follow(Some(FileStatus::Modified), true) else { panic!("a read") };
    assert_eq!(margin.follow(None, false), Follow::Cleared);
    assert!(margin.unfolded.is_empty());
    assert!(!margin.answered(late, Some(gutter())));
    assert_eq!(margin.gutter, None);
}

#[test]
fn an_answer_with_nothing_new_changes_nothing() {
    let mut margin = Margin::default();
    margin.follow(Some(FileStatus::Modified), false);
    assert!(margin.answered(1, Some(gutter())));
    margin.follow(Some(FileStatus::Modified), true);
    assert!(!margin.answered(2, Some(gutter())));
}

#[test]
fn unfolded_lines_stay_only_while_their_block_is_there() {
    let mut margin = Margin::default();
    margin.follow(Some(FileStatus::Modified), false);
    margin.answered(1, Some(gutter()));
    margin.toggle(6);
    assert_eq!(margin.unfolded_before(6).map(|block| block.mark), Some(Mark::Removed));
    margin.follow(Some(FileStatus::Modified), true);
    margin.answered(2, Some(Gutter::default()));
    assert_eq!(margin.unfolded_before(6), None);
    assert!(margin.unfolded.is_empty());
}

#[test]
fn a_second_click_folds_the_old_lines_again() {
    let mut margin = Margin::default();
    margin.follow(Some(FileStatus::Modified), false);
    margin.answered(1, Some(gutter()));
    margin.toggle(6);
    margin.toggle(6);
    assert_eq!(margin.unfolded_before(6), None);
}
