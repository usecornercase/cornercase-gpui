use super::{Fitting, fitting, most, trimmed};

const GAP: f32 = 8.;
const LINE: f32 = 22.;

fn fits(heights: &[f32], room: f32) -> Fitting {
    fitting(heights, GAP, room, LINE)
}

#[test]
fn every_item_is_drawn_when_all_fit() {
    assert_eq!(fits(&[40., 40., 40.], 200.), Fitting { shown: 3, more: false });
}

#[test]
fn the_oldest_wait_when_the_room_is_short() {
    assert_eq!(fits(&[156.; 5], 608.), Fitting { shown: 3, more: true });
}

#[test]
fn the_more_line_takes_its_room_from_the_cards() {
    assert_eq!(fits(&[100.; 4], 320.), Fitting { shown: 2, more: true });
}

#[test]
fn an_item_that_fits_exactly_is_drawn() {
    assert_eq!(fits(&[100.], 100.), Fitting { shown: 1, more: false });
}

#[test]
fn an_older_short_item_never_jumps_a_taller_newer_one() {
    assert_eq!(fits(&[100., 300., 40.], 200.), Fitting { shown: 1, more: true });
}

#[test]
fn nothing_is_drawn_in_a_room_smaller_than_the_newest() {
    assert_eq!(fits(&[300.], 100.), Fitting { shown: 0, more: true });
}

#[test]
fn the_more_line_needs_room_of_its_own() {
    assert_eq!(fits(&[300.], 10.), Fitting { shown: 0, more: false });
}

#[test]
fn no_item_draws_nothing() {
    assert_eq!(fits(&[], 100.), Fitting { shown: 0, more: false });
}

#[test]
fn an_item_alone_may_take_the_whole_room() {
    assert!((most(268., 1, GAP, LINE) - 268.).abs() < f32::EPSILON);
}

#[test]
fn an_item_leaves_room_for_the_more_line_when_others_are_there() {
    assert!((most(268., 2, GAP, LINE) - 238.).abs() < f32::EPSILON);
}

#[test]
fn the_most_is_never_below_nothing() {
    assert!(most(10., 2, GAP, LINE).abs() < f32::EPSILON);
}

#[test]
fn an_item_taller_than_its_most_is_asked_again_without_the_excess() {
    assert_eq!(trimmed(238., 260.), Some(216.));
    assert_eq!(trimmed(238., 200.), None);
}

#[test]
fn an_item_cut_to_its_most_is_drawn_when_rounding_leaves_it_a_hair_over() {
    assert_eq!(fitting(&[224.666_67, 40.], GAP, 256.666_66, 24.), Fitting { shown: 1, more: true });
}

#[test]
fn an_item_more_than_a_pixel_over_the_room_is_not_drawn() {
    assert_eq!(fits(&[102.], 100.), Fitting { shown: 0, more: true });
}

#[test]
fn an_item_cut_to_its_most_is_drawn_when_two_roundings_leave_it_most_of_a_device_pixel_over() {
    assert_eq!(fitting(&[241.818_18, 40.], GAP, 272.727_26, 23.636_364), Fitting { shown: 1, more: true });
}
