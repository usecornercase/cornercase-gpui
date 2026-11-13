use super::*;
use crate::panels::flows::LIST;

#[test]
fn a_wide_sheet_keeps_the_list_at_its_width_beside_the_page() {
    assert_eq!(columns(1_050., &LIST), Columns::Two { list: 330. });
}

#[test]
fn a_narrower_sheet_narrows_the_list_to_keep_the_page() {
    assert_eq!(columns(902., &LIST), Columns::Two { list: 282. });
    assert_eq!(columns(860., &LIST), Columns::Two { list: 240. });
}

#[test]
fn a_sheet_too_narrow_for_both_shows_one_column() {
    assert_eq!(columns(859., &LIST), Columns::One);
    assert_eq!(columns(525., &LIST), Columns::One);
}

#[test]
fn a_dragged_list_still_leaves_the_page_its_least_width() {
    let dragged = LIST.restored(Some(600.), 2_000.);
    assert_eq!(columns(1_050., &dragged), Columns::Two { list: 430. });
}
