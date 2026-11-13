use crate::ui::selection::{self, Caret, Selection};

use super::super::disk::text;
use super::{covered_by, one_based, selected_text};

fn chars(anchor: (usize, usize), head: (usize, usize)) -> Selection<usize> {
    Selection::Chars { anchor: Caret { spot: anchor.0, at: anchor.1 }, head: Caret { spot: head.0, at: head.1 } }
}

#[test]
fn selected_lines_copy_as_the_file_has_them() {
    let source = text("one\n\ttwo\nthree\n");
    assert_eq!(selected_text(&source, Selection::Lines { anchor: 0, head: 1 }), "one\n\ttwo");
}

#[test]
fn selected_characters_copy_with_their_tabs() {
    let source = text("one\n\ttwo\nthree\n");
    assert_eq!(selected_text(&source, chars((0, 1), (2, 2))), "ne\n\ttwo\nth");
}

#[test]
fn characters_ending_at_a_line_s_start_copy_its_line_break() {
    let source = text("one\ntwo\n");
    assert_eq!(selected_text(&source, chars((0, 1), (1, 0))), "ne\n");
    assert_eq!(selected_text(&source, chars((0, 3), (1, 0))), "\n");
}

#[test]
fn the_copy_and_its_label_agree_on_a_selection_ending_at_a_line_s_start() {
    let source = text("a\nb\nc\nd\ne\n");
    let picked = chars((2, 0), (4, 0));
    assert_eq!(selected_text(&source, picked), "c\nd\n");
    let label = selection::lines_label(one_based(covered_by(Some(picked)).expect("lines")));
    assert_eq!(label, "lines 3-4");
}

#[test]
fn a_selection_reaching_past_the_end_copies_what_is_there() {
    let source = text("one\n");
    assert_eq!(selected_text(&source, Selection::Lines { anchor: 0, head: 9 }), "one");
}

#[test]
fn line_numbers_count_from_one() {
    assert_eq!(one_based((0, 4)), (1, 5));
}

#[test]
fn a_selection_of_nothing_covers_no_line() {
    assert_eq!(covered_by(Some(chars((3, 2), (3, 2)))), None);
    assert_eq!(covered_by(None), None);
}

#[test]
fn a_selection_covers_its_lines() {
    assert_eq!(covered_by(Some(Selection::Lines { anchor: 2, head: 4 })), Some((2, 4)));
}
