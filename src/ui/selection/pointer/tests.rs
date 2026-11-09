use gpui::{point, px};

use super::super::{Caret, Selection};
use super::{Fit, Selecting, Step, far_enough};

fn caret(spot: usize, at: usize) -> Caret<usize> {
    Caret { spot, at }
}

fn chars(anchor: (usize, usize), head: (usize, usize)) -> Selection<usize> {
    Selection::Chars { anchor: caret(anchor.0, anchor.1), head: caret(head.0, head.1) }
}

fn text() -> Vec<String> {
    ["abcdef", "gh", "ijklmn"].map(String::from).to_vec()
}

#[test]
fn a_press_that_barely_moved_is_still_a_click() {
    assert!(!far_enough(point(px(10.), px(10.)), point(px(11.), px(11.))));
}

#[test]
fn a_press_that_moved_three_pixels_is_a_drag() {
    assert!(far_enough(point(px(10.), px(10.)), point(px(13.), px(10.))));
    assert!(far_enough(point(px(10.), px(10.)), point(px(10.), px(6.))));
}

#[test]
fn keys_start_from_the_top_shown_with_nothing_selected() {
    let mut selecting = Selecting::<usize>::default();
    let head = selecting.step((Step::Down, caret(1, 0)), &text()[..], Fit::Cut);
    assert_eq!(head, Some(caret(2, 0)));
    assert_eq!(selecting.selection, Some(chars((1, 0), (2, 0))));
}

#[test]
fn the_column_holds_only_while_the_keys_made_the_selection() {
    let mut selecting = Selecting::new(Some(chars((0, 4), (0, 5))));
    let down = |selecting: &mut Selecting<usize>| selecting.step((Step::Down, caret(0, 0)), &text()[..], Fit::Cut);
    assert_eq!(down(&mut selecting), Some(caret(1, 2)));
    assert_eq!(down(&mut selecting), Some(caret(2, 5)));
    selecting.selection = Some(chars((2, 1), (2, 1)));
    let up = selecting.step((Step::Up, caret(0, 0)), &text()[..], Fit::Cut);
    assert_eq!(up, Some(caret(1, 1)));
}
