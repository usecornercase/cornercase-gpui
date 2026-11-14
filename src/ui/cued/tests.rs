use gpui::px;

use super::{DISC, Seat, hidden_below};

#[test]
fn nothing_is_hidden_when_the_content_ends_at_the_edge() {
    assert!(!hidden_below(px(100.), px(100.)));
}

#[test]
fn a_pixel_of_rounding_does_not_show_the_cue() {
    assert!(!hidden_below(px(100.), px(101.)));
}

#[test]
fn content_past_the_edge_shows_the_cue() {
    assert!(hidden_below(px(100.), px(101.5)));
}

#[test]
fn content_that_ends_short_of_the_edge_shows_no_cue() {
    assert!(!hidden_below(px(100.), px(60.)));
}

#[test]
fn content_scrolled_up_to_the_edge_shows_no_cue() {
    assert!(!hidden_below(px(100.), px(99.)));
}

#[test]
fn a_cue_on_the_border_sits_below_the_edge() {
    assert!(Seat::OnBorder.centre(px(100.)) > px(100.));
}

#[test]
fn a_cue_inside_sits_wholly_above_the_edge() {
    assert!(Seat::Inside.centre(px(100.)) + px(DISC / 2.) < px(100.));
}
