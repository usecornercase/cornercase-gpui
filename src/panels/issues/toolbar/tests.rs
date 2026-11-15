use super::{Label, Shape};
use crate::ui::split::SHEET_AT_1280;

const CLOSED: f32 = 82.;

const fn anyone(width: f32) -> Label {
    Label { whole: width, anyone: width }
}

const fn person(whole: f32, anyone: f32) -> Label {
    Label { whole, anyone }
}

const GITHUB: [Label; 2] = [anyone(99.), anyone(86.)];

#[test]
fn the_filters_keep_one_row_where_every_part_just_fits_at_its_least() {
    assert_eq!(Shape::of(594., CLOSED, GITHUB), Shape::OneRow);
}

#[test]
fn a_pixel_less_moves_include_closed_beside_the_tabs() {
    assert_eq!(Shape::of(593., CLOSED, GITHUB), Shape::ClosedBesideTabs);
}

#[test]
fn the_least_sheet_moves_include_closed_beside_the_tabs() {
    assert_eq!(Shape::of(523., CLOSED, GITHUB), Shape::ClosedBesideTabs);
}

#[test]
fn a_long_handle_needs_no_more_room_than_anyone() {
    assert_eq!(Shape::of(594., CLOSED, [person(305., 99.), anyone(86.)]), Shape::OneRow);
}

#[test]
fn two_long_handles_keep_one_row_down_to_where_anyone_does() {
    let both = [person(305., 99.), person(291., 86.)];
    assert_eq!(Shape::of(594., CLOSED, both), Shape::OneRow);
    assert_eq!(Shape::of(593., CLOSED, both), Shape::ClosedBesideTabs);
}

#[test]
fn me_in_both_filters_keeps_one_row_in_a_narrower_sheet() {
    let both = [person(76., 99.), person(63., 86.)];
    assert_eq!(Shape::of(548., CLOSED, both), Shape::OneRow);
    assert_eq!(Shape::of(547., CLOSED, both), Shape::ClosedBesideTabs);
}

#[test]
fn another_trackers_field_names_move_where_the_row_breaks() {
    let shortcut = [anyone(85.), anyone(106.)];
    assert_eq!(Shape::of(600., CLOSED, shortcut), Shape::OneRow);
    assert_eq!(Shape::of(599., CLOSED, shortcut), Shape::ClosedBesideTabs);
}

#[test]
fn a_sheet_not_yet_measured_keeps_one_row() {
    assert_eq!(Shape::of(SHEET_AT_1280, CLOSED, GITHUB), Shape::OneRow);
}

#[test]
fn a_sheet_of_no_width_moves_include_closed_beside_the_tabs() {
    assert_eq!(Shape::of(0., CLOSED, GITHUB), Shape::ClosedBesideTabs);
}
