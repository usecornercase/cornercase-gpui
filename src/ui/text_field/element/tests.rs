use gpui::px;

use super::{Below, Head, Reveal, next_scroll, reveal_of, room_below, room_to_end};

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

#[test]
fn the_field_a_dialog_opened_on_reveals_itself_again_after_its_first_paint() {
    assert_eq!(Head::Opening.painted(), Head::Yes);
    assert_eq!(Head::Yes.painted(), Head::Yes);
    assert_eq!(Head::No.painted(), Head::No);
    assert_eq!(reveal_of(false, true, Head::Opening.painted()), Reveal::Form);
}

#[test]
fn a_key_typed_before_the_dialog_s_first_frame_brings_its_field_into_view() {
    assert_eq!(reveal_of(true, true, Head::Opening), Reveal::Caret);
}

#[test]
fn the_last_field_s_reveal_scrolls_its_form_to_the_end() {
    let (height, max_offset, line) = (px(212.), px(300.), (px(400.), px(20.)));
    let room = room_to_end(height, max_offset, line);
    assert_eq!(room, px(92.));
    let margins = (px(12.), room_below(Below::ToEnd, false, room));
    assert_eq!(next_scroll(Reveal::Caret, px(0.), line, margins, height), max_offset);
}

#[test]
fn the_last_field_of_a_form_that_fits_does_not_move_it() {
    let (height, line) = (px(572.), (px(294.), px(20.)));
    let room = room_to_end(height, px(0.), line);
    let margins = (px(12.), room_below(Below::ToEnd, false, room));
    assert_eq!(next_scroll(Reveal::Caret, px(0.), line, margins, height), px(0.));
}
