use gpui::{Bounds, point, px, size};

use super::*;

fn link_at(x: f32, y: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(120.), px(17.)))
}

fn room(w: f32, h: f32) -> Size<Pixels> {
    size(px(w), px(h))
}

#[test]
fn a_link_with_room_to_its_right_gets_its_card_beside() {
    let place = place(link_at(40., 30.), room(1200., 800.));
    assert_eq!((place.side, place.left, place.top, place.bottom), (Side::Beside, 166., Some(20.), None));
    assert_eq!((place.width, place.height), (WIDTH, 770.));
}

#[test]
fn a_card_beside_a_low_link_grows_upwards() {
    let place = place(link_at(40., 700.), room(1200., 800.));
    assert_eq!((place.side, place.top, place.bottom, place.height), (Side::Beside, None, Some(73.), 717.));
}

#[test]
fn a_link_near_the_right_edge_gets_its_card_below() {
    let place = place(link_at(1000., 30.), room(1200., 800.));
    assert_eq!(
        (place.side, place.left, place.top, place.bottom),
        (Side::Below, 1200. - MARGIN - WIDTH, Some(53.), None)
    );
    assert_eq!(place.height, 800. - 47. - 6. - MARGIN);
}

#[test]
fn a_low_link_near_the_right_edge_gets_its_card_above() {
    let place = place(link_at(1000., 700.), room(1200., 800.));
    assert_eq!((place.side, place.top, place.bottom), (Side::Above, None, Some(106.)));
    assert_eq!(place.height, 700. - 6. - MARGIN);
}

#[test]
fn a_narrow_terminal_gives_the_card_its_width_less_the_margins() {
    let place = place(link_at(100., 30.), room(400., 800.));
    assert_eq!((place.side, place.left, place.width), (Side::Below, MARGIN, 400. - 2. * MARGIN));
}

#[test]
fn a_picture_shrinks_to_fit_and_keeps_its_shape() {
    assert_eq!(fit((1600., 900.), (640., 400.)), (640., 360.));
    assert_eq!(fit((300., 1200.), (640., 400.)), (100., 400.));
}

#[test]
fn a_small_picture_keeps_its_size() {
    assert_eq!(fit((32., 16.), (640., 400.)), (32., 16.));
}

#[test]
fn a_picture_of_no_size_takes_the_room() {
    assert_eq!(fit((0., 0.), (640., 400.)), (640., 400.));
}

#[test]
fn the_rows_shown_are_what_fits_up_to_a_limit() {
    assert_eq!(shown_rows(500, 180., 18., 18), 10);
    assert_eq!(shown_rows(500, 900., 18., 18), 18);
    assert_eq!(shown_rows(3, 900., 18., 18), 3);
    assert_eq!(shown_rows(500, 4., 18., 18), 1);
}
