use gpui::px;

use super::{Below, Head, Reveal, next_scroll, reveal_of, room_below};

const CARET: (gpui::Pixels, gpui::Pixels) = (px(294.), px(20.));
const MARGINS: (gpui::Pixels, gpui::Pixels) = (px(12.), px(12.));

#[test]
fn a_field_that_takes_the_keys_as_its_dialog_opens_leaves_the_body_at_its_top() {
    let why = reveal_of(false, true, Head::Opening);
    assert_eq!(why, Reveal::Still);
    assert_eq!(next_scroll(why, px(0.), CARET, MARGINS, px(212.)), px(0.));
}

#[test]
fn a_field_one_pixel_past_the_fold_still_opens_at_the_top() {
    let why = reveal_of(false, true, Head::Opening);
    assert_eq!(next_scroll(why, px(0.), CARET, MARGINS, px(325.)), px(0.));
    assert_eq!(next_scroll(why, px(0.), CARET, MARGINS, px(326.)), px(0.));
}

#[test]
fn the_first_key_typed_after_the_dialog_opens_brings_its_field_into_view() {
    let why = reveal_of(true, false, Head::Yes);
    assert_eq!(why, Reveal::Caret);
    assert_eq!(next_scroll(why, px(0.), CARET, MARGINS, px(212.)), px(114.));
}

#[test]
fn the_field_a_dialog_opened_on_shows_the_form_from_its_top_when_the_keys_come_back() {
    let why = reveal_of(false, true, Head::Yes);
    assert_eq!(why, Reveal::Form);
    assert_eq!(next_scroll(why, px(178.), CARET, MARGINS, px(326.)), px(0.));
}

#[test]
fn another_field_that_takes_the_keys_moves_the_form_as_little_as_it_takes() {
    assert_eq!(reveal_of(false, true, Head::No), Reveal::Caret);
}

#[test]
fn a_press_on_the_field_a_dialog_opened_on_keeps_the_form_where_it_stands() {
    let why = reveal_of(true, true, Head::Yes);
    assert_eq!(why, Reveal::Caret);
    assert_eq!(next_scroll(why, px(50.), (px(100.), px(20.)), MARGINS, px(212.)), px(50.));
}

#[test]
fn a_field_that_ends_its_form_keeps_room_down_to_the_form_s_end() {
    assert_eq!(room_below(Below::ToEnd, false, px(128.)), px(128.));
}

#[test]
fn a_field_at_its_form_s_end_still_keeps_its_margin() {
    assert_eq!(room_below(Below::ToEnd, false, px(5.)), px(12.));
}

#[test]
fn a_field_asking_for_nothing_keeps_the_margin_of_its_kind() {
    assert_eq!(room_below(Below::Margin, false, px(300.)), px(12.));
    assert_eq!(room_below(Below::Margin, true, px(300.)), px(32.));
}

#[test]
fn a_body_where_everything_fits_does_not_move() {
    assert_eq!(next_scroll(Reveal::Form, px(0.), CARET, MARGINS, px(572.)), px(0.));
    assert_eq!(next_scroll(Reveal::Caret, px(0.), CARET, MARGINS, px(572.)), px(0.));
}
