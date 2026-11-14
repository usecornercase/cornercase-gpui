use super::{LIST, PAGE_LEAST};
use crate::ui::split::{Columns, SHEET_AT_1280};

#[test]
fn a_wide_sheet_keeps_the_list_at_its_width_beside_the_page() {
    assert_eq!(LIST.columns(1_050., PAGE_LEAST), Columns::Two { list: 330. });
}

#[test]
fn a_narrower_sheet_narrows_the_list_to_keep_the_page() {
    assert_eq!(LIST.columns(902., PAGE_LEAST), Columns::Two { list: 282. });
    assert_eq!(LIST.columns(860., PAGE_LEAST), Columns::Two { list: 240. });
}

#[test]
fn a_sheet_too_narrow_for_both_shows_one_column() {
    assert_eq!(LIST.columns(859., PAGE_LEAST), Columns::One);
    assert_eq!(LIST.columns(525., PAGE_LEAST), Columns::One);
}

#[test]
fn a_dragged_list_still_leaves_the_page_its_least_width() {
    let dragged = LIST.restored(Some(600.), 2_000.);
    assert_eq!(dragged.columns(1_050., PAGE_LEAST), Columns::Two { list: 430. });
}

#[test]
fn a_sheet_not_yet_measured_draws_two_columns_at_the_lists_width() {
    assert_eq!(LIST.columns(SHEET_AT_1280, PAGE_LEAST), Columns::Two { list: 330. });
}

#[test]
fn a_dragged_list_is_drawn_whole_again_in_a_wider_sheet() {
    let dragged = LIST.restored(Some(600.), 2_000.);
    assert_eq!(dragged.columns(1_050., PAGE_LEAST), Columns::Two { list: 430. });
    assert_eq!(dragged.columns(1_300., PAGE_LEAST), Columns::Two { list: 600. });
}
