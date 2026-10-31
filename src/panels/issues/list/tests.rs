use gpui::px;

use super::*;

fn issues(keys: &[&str]) -> Vec<Issue> {
    keys.iter().map(|key| Issue { source: "github".into(), key: (*key).into(), ..Issue::default() }).collect()
}

fn top(rows: &Rows) -> (usize, gpui::Pixels) {
    let top = rows.list.logical_scroll_top();
    (top.item_ix, top.offset_in_item)
}

#[test]
fn another_list_resets_the_rows_and_keeps_the_top_row() {
    let mut rows = Rows::new();
    rows.sync(issues(&["#1", "#2", "#3"]), false, 400.);
    rows.list.scroll_to(ListOffset { item_ix: 1, offset_in_item: px(4.) });
    rows.sync(issues(&["#1", "#4", "#5", "#6"]), false, 400.);
    assert_eq!(rows.list.item_count(), 4);
    assert_eq!(top(&rows), (1, px(4.)));
}

#[test]
fn the_same_list_keeps_its_measured_rows() {
    let mut rows = Rows::new();
    rows.sync(issues(&["#1", "#2"]), false, 400.);
    let before = rows.issues.clone();
    rows.sync(issues(&["#1", "#2"]), false, 400.);
    assert!(Rc::ptr_eq(&before, &rows.issues));
}

#[test]
fn a_new_width_measures_the_rows_again() {
    let mut rows = Rows::new();
    rows.sync(issues(&["#1", "#2"]), false, 400.);
    let before = rows.issues.clone();
    rows.sync(issues(&["#1", "#2"]), false, 420.);
    assert!(!Rc::ptr_eq(&before, &rows.issues));
}

#[test]
fn the_cap_note_is_one_more_row() {
    let mut rows = Rows::new();
    rows.sync(issues(&["#1", "#2"]), true, 400.);
    assert_eq!(rows.list.item_count(), 3);
}

#[test]
fn going_to_the_top_shows_the_first_row() {
    let mut rows = Rows::new();
    rows.sync(issues(&["#1", "#2", "#3"]), false, 400.);
    rows.list.scroll_to(ListOffset { item_ix: 2, offset_in_item: px(9.) });
    rows.to_top();
    assert_eq!(top(&rows), (0, px(0.)));
}
