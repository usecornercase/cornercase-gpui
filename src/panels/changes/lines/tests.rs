use std::collections::HashMap;

use crate::test_util::fixture;
use crate::ui::selection::{Caret, Selection};

use super::super::diff::{self, Part, Spot};
use super::super::wire::{Gap, GapLine, Hunk, Line, LineKind, Mode, Patch};
use super::super::{DiffData, Selected, Whose};

fn line(kind: LineKind, old: Option<u32>, new: Option<u32>, text: &str) -> Line {
    Line { kind, old, new, text: text.into(), ..Line::default() }
}

fn data() -> DiffData {
    let hunks = vec![
        Hunk {
            old_start: 3,
            new_start: 3,
            context: String::new(),
            lines: vec![
                line(LineKind::Context, Some(3), Some(3), "alpha"),
                line(LineKind::Removed, Some(4), None, "beta"),
                line(LineKind::Added, None, Some(4), "BETA"),
                line(LineKind::Context, Some(5), Some(5), "gamma"),
            ],
        },
        Hunk {
            old_start: 10,
            new_start: 10,
            context: String::new(),
            lines: vec![
                line(LineKind::Removed, Some(10), None, "gone"),
                line(LineKind::Context, Some(11), Some(10), "kept"),
            ],
        },
    ];
    let gap =
        Gap { lines: vec![GapLine { old: 6, new: 6, text: "between".into(), ..GapLine::default() }], ..Gap::default() };
    opened(Patch { hunks, ..Patch::default() }, gap)
}

fn opened(patch: Patch, gap: Gap) -> DiffData {
    let gaps = HashMap::from([(1, (gap.clone(), Vec::new()))]);
    let gap_lines = HashMap::from([(1, gap)]);
    let rows = diff::rows(&patch.hunks, &gap_lines);
    let selected = Selected { workspace: Whose::Here(7), path: "src/a.rs".into(), mode: Mode::Uncommitted };
    DiffData { selected, patch, looks: Vec::new(), gaps, rows }
}

fn makefile() -> DiffData {
    let patch = serde_json::from_str(fixture!("changes/file_tabs.json")).expect("the patch parses");
    let gap = serde_json::from_str(fixture!("changes/gap_tabs.json")).expect("the gap parses");
    opened(patch, gap)
}

fn one_line_chars(spot: Spot, from: usize, to: usize) -> Selection<Spot> {
    chars((spot, from), (spot, to))
}

fn patch(hunk: usize, line: usize) -> Spot {
    Spot { hunk, part: Part::Patch, line }
}

fn gap(hunk: usize, line: usize) -> Spot {
    Spot { hunk, part: Part::Gap, line }
}

fn lines(anchor: Spot, head: Spot) -> Selection<Spot> {
    Selection::Lines { anchor, head }
}

fn chars(anchor: (Spot, usize), head: (Spot, usize)) -> Selection<Spot> {
    Selection::Chars { anchor: Caret { spot: anchor.0, at: anchor.1 }, head: Caret { spot: head.0, at: head.1 } }
}

#[test]
fn spots_sort_as_the_rows_are_drawn() {
    let rows = data().rows;
    let spots: Vec<Spot> = rows.iter().filter_map(|row| row.spot()).collect();
    let mut sorted = spots.clone();
    sorted.sort();
    assert_eq!(spots, sorted);
    assert!(patch(0, 3) < gap(1, 0) && gap(1, 0) < patch(1, 0));
}

#[test]
fn a_hunk_s_header_and_a_closed_gap_are_no_lines() {
    assert_eq!(diff::Row::Hunk(0).spot(), None);
    assert_eq!(diff::Row::Gap { hunk: 1, lines: 3 }.spot(), None);
}

#[test]
fn a_range_copies_its_lines_without_signs_or_numbers_over_removed_and_added() {
    assert_eq!(data().selected_text(lines(patch(0, 0), patch(0, 2))), "alpha\nbeta\nBETA");
}

#[test]
fn a_range_across_hunks_copies_the_open_gap_between_them() {
    assert_eq!(data().selected_text(lines(patch(0, 3), patch(1, 0))), "gamma\nbetween\ngone");
}

#[test]
fn characters_copy_only_what_they_cover() {
    assert_eq!(data().selected_text(chars((patch(0, 0), 2), (patch(0, 0), 4))), "ph");
    assert_eq!(data().selected_text(chars((patch(0, 2), 2), (patch(0, 3), 2))), "TA\nga");
}

#[test]
fn a_range_refers_to_the_new_side_s_lines() {
    assert_eq!(data().selected_lines(lines(patch(0, 0), patch(0, 3))), Some((3, 5)));
}

#[test]
fn removed_lines_inside_a_range_do_not_change_its_ends() {
    assert_eq!(data().selected_lines(lines(patch(0, 1), patch(0, 2))), Some((4, 4)));
}

#[test]
fn only_removed_lines_refer_to_where_their_hunk_changed() {
    assert_eq!(data().selected_lines(Selection::line(patch(0, 1))), Some((4, 4)));
    assert_eq!(data().selected_lines(Selection::line(patch(1, 0))), Some((10, 10)));
}

#[test]
fn characters_ending_at_a_line_s_start_leave_it_out_of_the_reference() {
    assert_eq!(data().selected_lines(chars((patch(0, 0), 1), (patch(0, 3), 0))), Some((3, 4)));
}

#[test]
fn an_open_gap_s_line_refers_to_its_new_number() {
    assert_eq!(data().selected_lines(Selection::line(gap(1, 0))), Some((6, 6)));
}

#[test]
fn the_label_names_the_new_side_s_lines() {
    assert_eq!(data().selection_label(lines(patch(0, 0), patch(0, 3))).as_deref(), Some("lines 3-5"));
    assert_eq!(data().selection_label(Selection::line(patch(0, 2))).as_deref(), Some("line 4"));
}

#[test]
fn the_label_of_only_removed_lines_names_their_old_numbers() {
    assert_eq!(data().selection_label(Selection::line(patch(0, 1))).as_deref(), Some("removed line 4"));
}

#[test]
fn a_selection_of_lines_no_longer_there_names_nothing() {
    let gone = Selection::line(patch(9, 0));
    assert_eq!(data().selected_lines(gone), None);
    assert_eq!(data().selection_label(gone), None);
}

#[test]
fn a_selected_line_with_a_tab_copies_it_as_the_file_has_it() {
    assert_eq!(makefile().selected_text(Selection::line(patch(0, 1))), "\techo hi");
}

#[test]
fn a_range_copies_each_line_as_the_file_has_it_over_removed_and_added() {
    let copied = makefile().selected_text(lines(patch(0, 0), patch(0, 3)));
    assert_eq!(copied, "all:\n\techo hi\n\techo two\t# end\n\techo bye\t# end");
}

#[test]
fn an_open_gap_s_line_copies_its_tab() {
    assert_eq!(makefile().selected_text(Selection::line(gap(1, 4))), "8\tgap");
}

#[test]
fn characters_after_a_tab_copy_what_is_drawn_there() {
    assert_eq!(makefile().selected_text(one_line_chars(patch(0, 1), 4, 8)), "echo");
}

#[test]
fn characters_crossing_a_tab_copy_the_tab_once() {
    assert_eq!(makefile().selected_text(one_line_chars(patch(0, 3), 9, 17)), "bye\t#");
}

#[test]
fn characters_ending_inside_a_tab_take_the_whole_tab() {
    assert_eq!(makefile().selected_text(one_line_chars(patch(0, 3), 9, 14)), "bye\t");
}

#[test]
fn characters_starting_inside_a_tab_take_the_whole_tab() {
    assert_eq!(makefile().selected_text(one_line_chars(patch(0, 1), 2, 8)), "\techo");
}

#[test]
fn characters_across_lines_keep_each_line_s_tabs() {
    let copied = makefile().selected_text(chars((patch(0, 2), 9), (patch(0, 3), 8)));
    assert_eq!(copied, "two\t# end\n\techo");
}

#[test]
fn characters_after_a_tab_keep_whole_accented_letters() {
    assert_eq!(makefile().selected_text(one_line_chars(patch(1, 6), 5, 14)), "éa\tñ");
}

#[test]
fn characters_of_a_line_without_a_tab_copy_what_is_drawn() {
    assert_eq!(makefile().selected_text(one_line_chars(patch(0, 0), 0, 3)), "all");
}

#[test]
fn a_line_whose_own_text_does_not_draw_as_shown_copies_what_is_shown() {
    let mut data = data();
    data.patch.hunks[0].lines[0].raw = Some("\tsomething else".into());
    assert_eq!(data.selected_text(Selection::line(patch(0, 0))), "alpha");
}
