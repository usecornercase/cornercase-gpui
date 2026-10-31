use gpui::CursorStyle;

use super::pane_menu::move_pane_args;
use super::selection::word_at;
use super::{cycled, divider_cursor};
use crate::render::moving::{Aim, Place};

#[test]
fn a_divider_shows_the_way_it_moves() {
    assert_eq!(divider_cursor(true), CursorStyle::ResizeRow);
    assert_eq!(divider_cursor(false), CursorStyle::ResizeColumn);
}

#[test]
fn a_dropped_pane_asks_cornercase_to_move_it_beside_its_target() {
    let args = move_pane_args(4, Aim { target: 7, place: Place::Below });
    assert_eq!(args, ["move-pane", "--pane", "4", "--target", "7", "--place", "below"]);
}

#[test]
fn a_word_is_a_run_of_one_class() {
    let text = "ls ~/src/web-shop && echo";
    let line: Vec<&str> = text.char_indices().map(|(i, c)| &text[i..i + c.len_utf8()]).collect();
    assert_eq!(word_at(&line, 5), (3, 16));
    assert_eq!(word_at(&line, 0), (0, 1));
    assert_eq!(word_at(&line, 18), (18, 19));
    assert_eq!(word_at(&["a", "中", "", "b", " "], 2), (0, 3));
    assert_eq!(word_at(&["a"], 4), (4, 4));
}

#[test]
fn cycling_wraps_around_both_ways() {
    let ids = [4, 7, 9];
    assert_eq!(cycled(&ids, Some(9), 1), Some(4));
    assert_eq!(cycled(&ids, Some(4), -1), Some(9));
    assert_eq!(cycled(&ids, Some(7), 1), Some(9));
    assert_eq!(cycled(&ids, None, 1), Some(7));
    assert_eq!(cycled(&[], Some(1), 1), None);
}
