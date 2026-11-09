use gpui::{Bounds, Pixels, Size, point, px, size};

use super::{At, LEAST, Opening, Placed, Rect, SHOWN, Screen, State, fits, keep, opening};
use crate::app::frame;
use crate::platform::Os;

fn bounds(x: f32, y: f32, w: f32, h: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(w), px(h)))
}

fn screen(uuid: &str, x: f32, w: f32) -> Screen {
    Screen { uuid: Some(uuid.into()), bounds: bounds(x, 0., w, 1000.) }
}

fn placed(display: &str, b: Bounds<Pixels>, state: State) -> Placed {
    Placed { display: Some(display.into()), bounds: Rect::from(b), state }
}

#[expect(clippy::unnecessary_wraps, reason = "the opening takes an Option")]
fn primary() -> Option<Size<Pixels>> {
    Some(size(px(1920.), px(1000.)))
}

fn screens() -> Vec<Screen> {
    vec![screen("A", 0., 1920.), screen("B", 0., 1600.)]
}

fn windowed(display: &str, b: Bounds<Pixels>) -> Placed {
    placed(display, b, State::Windowed)
}

fn open(saved: &Placed, screens: &[Screen], os: Os) -> Opening {
    opening(Some(saved), screens, primary(), os)
}

fn centred_at(saved: (f32, f32), state: State) -> Opening {
    let shown = frame::opening_size(saved, LEAST, primary());
    let width = if state == State::Windowed { shown.0 } else { 1920. };
    Opening { at: At::Centred(shown), state, width }
}

#[test]
fn the_first_launch_opens_as_before() {
    let shown = frame::opening_size(SHOWN, LEAST, primary());
    let expected = Opening { at: At::Centred(shown), state: State::Windowed, width: 1280. };
    assert_eq!(opening(None, &screens(), primary(), Os::Mac), expected);
}

#[test]
fn a_window_reopens_where_it_was_on_its_display() {
    let b = bounds(120., 64., 1000., 700.);
    let got = open(&windowed("B", b), &screens(), Os::Mac);
    assert_eq!(got, Opening { at: At::On { screen: 1, bounds: b }, state: State::Windowed, width: 1000. });
}

#[test]
fn a_window_on_a_display_left_of_the_primary_reopens_there() {
    let screens = vec![screen("A", 0., 1920.), screen("L", -1920., 1920.)];
    let b = bounds(-1500., 100., 1000., 700.);
    let got = open(&windowed("L", b), &screens, Os::Windows);
    assert_eq!(got.at, At::On { screen: 1, bounds: b });
}

#[test]
fn a_window_whose_display_is_gone_opens_centred_at_its_size() {
    let got = open(&windowed("gone", bounds(120., 64., 1000., 700.)), &screens(), Os::Mac);
    assert_eq!(got, centred_at((1000., 700.), State::Windowed));
}

#[test]
fn a_window_whose_title_row_is_out_of_reach_opens_centred() {
    for b in [bounds(100., -50., 1000., 700.), bounds(100., 980., 1000., 700.), bounds(1850., 64., 1000., 700.)] {
        let got = open(&windowed("A", b), &screens(), Os::Mac);
        assert!(matches!(got.at, At::Centred(_)), "{b:?}");
    }
}

#[test]
fn a_window_larger_than_its_display_now_opens_centred() {
    let got = open(&windowed("B", bounds(0., 0., 1800., 700.)), &screens(), Os::Mac);
    assert!(matches!(got.at, At::Centred(_)));
}

#[test]
fn a_window_partly_past_a_side_edge_reopens_there() {
    let b = bounds(1620., 64., 1000., 700.);
    assert!(fits(b, bounds(0., 0., 1920., 1000.)));
    let got = open(&windowed("A", b), &screens(), Os::Mac);
    assert_eq!(got.at, At::On { screen: 0, bounds: b });
}

#[test]
fn on_linux_the_window_keeps_its_size_and_state_but_not_its_place() {
    let saved = placed("A", bounds(120., 64., 1000., 700.), State::Maximised);
    assert_eq!(open(&saved, &screens(), Os::Linux), centred_at((1000., 700.), State::Maximised));
}

#[test]
fn a_broken_saved_size_opens_as_the_first_launch() {
    for b in [bounds(0., 0., 0., 700.), bounds(0., 0., -5., 700.), bounds(f32::NAN, 0., 1000., 700.)] {
        let got = open(&windowed("A", b), &screens(), Os::Mac);
        assert_eq!(got, opening(None, &screens(), primary(), Os::Mac));
    }
}

#[test]
fn a_maximised_or_full_screen_window_reopens_so() {
    for state in [State::Maximised, State::FullScreen] {
        let on = open(&placed("A", bounds(10., 64., 1000., 700.), state), &screens(), Os::Mac);
        assert_eq!(on.state, state);
        let off = open(&placed("gone", bounds(10., 64., 1000., 700.), state), &screens(), Os::Mac);
        assert_eq!(off.state, state);
    }
}

#[test]
fn the_widths_of_a_maximised_window_fit_its_display() {
    let b = bounds(10., 64., 1000., 700.);
    assert_eq!(open(&placed("B", b, State::Maximised), &screens(), Os::Mac).width, 1600.);
    assert_eq!(open(&placed("gone", b, State::FullScreen), &screens(), Os::Mac).width, 1920.);
    let none = opening(Some(&placed("gone", b, State::FullScreen)), &screens(), None, Os::Mac);
    assert_eq!(none.width, 1000.);
}

#[test]
fn a_windowed_change_takes_the_new_place_and_display() {
    let before = windowed("A", bounds(0., 64., 1000., 700.));
    let seen = windowed("B", bounds(50., 80., 1100., 720.));
    assert_eq!(keep(Some(&before), seen.clone()), seen);
}

#[test]
fn a_change_while_maximised_or_full_screen_keeps_the_windowed_place() {
    let before = windowed("A", bounds(0., 64., 1000., 700.));
    let seen = placed("C", bounds(0., 0., 2560., 1400.), State::Maximised);
    let kept = keep(Some(&before), seen);
    assert_eq!(kept, Placed { state: State::Maximised, ..before });
}

#[test]
fn the_first_change_with_nothing_before_takes_what_it_sees() {
    let seen = placed("C", bounds(0., 0., 2560., 1400.), State::FullScreen);
    assert_eq!(keep(None, seen.clone()), seen);
}
