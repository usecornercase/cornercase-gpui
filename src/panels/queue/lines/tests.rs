use super::{picked, selected_text};
use crate::ui::selection::{Caret, Selection};

#[test]
fn a_selection_copies_from_the_middle_of_one_line_to_another() {
    let lines: Vec<String> = ["alpha beta", "gamma", "delta epsilon"].map(str::to_string).into();
    let across = Selection::Chars { anchor: Caret { spot: 0, at: 6 }, head: Caret { spot: 2, at: 5 } };
    assert_eq!(selected_text(&lines, across), "beta\ngamma\ndelta");
    let inside = Selection::Chars { anchor: Caret { spot: 1, at: 1 }, head: Caret { spot: 1, at: 4 } };
    assert_eq!(selected_text(&lines, inside), "amm");
    assert_eq!(selected_text(&lines, Selection::line(2)), "delta epsilon");
}

#[test]
fn the_label_counts_lines_since_none_are_numbered() {
    let one = Selection::Chars { anchor: Caret { spot: 4, at: 1 }, head: Caret { spot: 4, at: 3 } };
    assert_eq!(picked(one), "1 line");
    let three = Selection::Chars { anchor: Caret { spot: 4, at: 1 }, head: Caret { spot: 6, at: 3 } };
    assert_eq!(picked(three), "3 lines");
}

#[test]
fn a_selection_that_ends_at_the_start_of_the_next_line_keeps_the_line_break() {
    let lines: Vec<String> = ["abc", "def"].map(str::to_string).into();
    let to_end = Selection::Chars { anchor: Caret { spot: 0, at: 3 }, head: Caret { spot: 1, at: 0 } };
    assert_eq!(selected_text(&lines, to_end), "\n");
    let from_middle = Selection::Chars { anchor: Caret { spot: 0, at: 1 }, head: Caret { spot: 1, at: 0 } };
    assert_eq!(selected_text(&lines, from_middle), "bc\n");
    let inside = Selection::Chars { anchor: Caret { spot: 0, at: 1 }, head: Caret { spot: 1, at: 2 } };
    assert_eq!(selected_text(&lines, inside), "bc\nde");
}

#[test]
fn the_count_leaves_out_the_line_the_copy_ends_before() {
    let lines: Vec<String> = ["alpha", "beta", "gamma"].map(str::to_string).into();
    let across = Selection::Chars { anchor: Caret { spot: 0, at: 2 }, head: Caret { spot: 2, at: 0 } };
    assert_eq!(selected_text(&lines, across), "pha\nbeta\n");
    assert_eq!(picked(across), "2 lines");
}
