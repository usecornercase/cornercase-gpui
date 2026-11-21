use super::{Fit, Fitting, Standing, across, cut_to, fitting, most, standing, tops};

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

fn card(text: f32) -> impl FnMut(Fit) -> f32 {
    move |fit| text.min(fit.most) + 22.
}

#[test]
fn an_item_that_fits_its_most_is_left_as_it_is() {
    let placed = cut_to(268., card(200.));
    assert!((placed.height - 222.).abs() < f32::EPSILON && !placed.capped);
}

#[test]
fn an_item_far_over_its_most_ends_on_it() {
    let placed = cut_to(268., card(2000.));
    assert!((placed.height - 268.).abs() < f32::EPSILON && placed.capped);
}

#[test]
fn an_item_whose_chrome_alone_puts_it_over_its_most_ends_on_it() {
    for text in [247., 252.5, 260., 267.9] {
        let placed = cut_to(268., card(text));
        assert!(placed.height <= 268. + 1., "text {text} ended at {}", placed.height);
    }
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

const COLUMN: (f32, f32) = (52., 760.);
const CLEAR: f32 = 16.;
const LEAST: f32 = 80.;

fn stands(field: Option<(f32, f32)>) -> Standing {
    standing(COLUMN, field, CLEAR, LEAST)
}

const WHOLE: Standing = Standing { top: 52., bottom: 760., from_top: false };

#[test]
fn with_no_field_the_stack_stands_from_the_bottom_of_its_whole_column() {
    assert_eq!(stands(None), WHOLE);
}

#[test]
fn a_field_at_the_bottom_sends_the_stack_to_the_top_and_its_room_ends_short_of_the_field() {
    assert_eq!(stands(Some((700., 740.))), Standing { top: 52., bottom: 684., from_top: true });
}

#[test]
fn a_field_at_the_top_leaves_the_stack_at_the_bottom_and_cuts_its_room() {
    assert_eq!(stands(Some((60., 90.))), Standing { top: 106., bottom: 760., from_top: false });
}

#[test]
fn a_field_in_the_middle_gives_the_taller_part_and_the_lower_one_on_a_tie() {
    assert!(!stands(Some((300., 330.))).from_top);
    assert_eq!(stands(Some((500., 530.))), Standing { top: 52., bottom: 484., from_top: true });
    assert_eq!(stands(Some((391., 421.))), Standing { top: 437., bottom: 760., from_top: false });
}

#[test]
fn a_field_outside_the_column_changes_nothing() {
    assert_eq!(stands(Some((10., 52.))), WHOLE);
    assert_eq!(stands(Some((760., 790.))), WHOLE);
}

#[test]
fn with_no_part_tall_enough_the_stack_stands_as_if_no_field_had_the_keys() {
    let column = (52., 320.);
    let whole = Standing { top: 52., bottom: 320., from_top: false };
    assert_eq!(standing(column, Some((120., 260.)), CLEAR, LEAST), whole);
}

#[test]
fn a_field_beside_the_column_is_not_across_it() {
    assert!(!across((932., 1280.), (100., 600.)));
    assert!(!across((932., 1280.), (600., 932.)));
    assert!(across((932., 1280.), (600., 933.)));
}

#[test]
fn from_the_bottom_the_newest_ends_on_the_rooms_bottom_and_the_more_line_goes_above() {
    let (drawn, free) = tops(&[40., 60.], GAP, Standing { top: 52., bottom: 300., from_top: false });
    assert_eq!(drawn, [260., 192.]);
    assert!((free - 184.).abs() < f32::EPSILON);
}

#[test]
fn from_the_top_the_newest_starts_at_the_rooms_top_and_the_more_line_goes_below() {
    let (drawn, free) = tops(&[40., 60.], GAP, Standing { top: 52., bottom: 300., from_top: true });
    assert_eq!(drawn, [52., 100.]);
    assert!((free - 168.).abs() < f32::EPSILON);
}
