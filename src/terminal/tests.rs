use super::pane_menu::pane_place;
use super::selection::word_at;
use crate::model::Area;

#[test]
fn a_pane_dropped_in_the_middle_swaps_else_takes_the_nearest_edge() {
    let rect = Area { x: 10, y: 0, cols: 30, rows: 12 };
    assert_eq!(pane_place(rect, 25, 6), "swap");
    assert_eq!(pane_place(rect, 11, 6), "left");
    assert_eq!(pane_place(rect, 39, 5), "right");
    assert_eq!(pane_place(rect, 24, 0), "above");
    assert_eq!(pane_place(rect, 24, 11), "below");
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
