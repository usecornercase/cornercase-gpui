use gpui::{Bounds, point, px, size};

use super::super::{Caret, Selection};
use super::{Fit, Laid, Off, Selecting, Step, far_enough};

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

fn laid_over(body_top: f32) -> Laid<usize> {
    let laid = Laid::<usize>::default();
    laid.set_body(Bounds::new(point(px(40.), px(body_top)), size(px(300.), px(400.))));
    laid
}

fn row_at(top: f32) -> Bounds<gpui::Pixels> {
    Bounds::new(point(px(40.), px(top)), size(px(300.), px(18.)))
}

#[test]
fn a_row_measured_below_the_view_scrolls_just_enough_to_show_it_and_a_row_under_it() {
    assert_eq!(laid_over(0.).past_edge_of(row_at(410.), 0.), Off::By(px(46.)));
}

#[test]
fn a_row_measured_above_the_view_scrolls_back_to_keep_a_row_over_it() {
    assert_eq!(laid_over(0.).past_edge_of(row_at(-40.), 0.), Off::By(px(-58.)));
}

#[test]
fn clearing_a_selection_says_it_selected_something() {
    let mut selecting = Selecting::<usize>::new(Some(Selection::line(1)));
    assert!(selecting.clear());
    assert_eq!(selecting.selection, None);
}

#[test]
fn clearing_a_selection_of_nothing_is_no_step() {
    let mut selecting = Selecting::new(Some(chars((1, 2), (1, 2))));
    assert!(!selecting.clear());
    assert_eq!(selecting.selection, None);
}

#[test]
fn a_selection_back_on_its_anchor_shows_nothing() {
    assert!(!Selecting::<usize>::default().shows());
    assert!(!Selecting::new(Some(chars((1, 2), (1, 2)))).shows());
    assert!(Selecting::new(Some(chars((1, 2), (1, 4)))).shows());
    assert!(Selecting::<usize>::new(Some(Selection::line(1))).shows());
}
