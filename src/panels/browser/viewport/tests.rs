use gpui::{Bounds, point, px, size};

use super::*;

fn meta(width: f32, height: f32) -> Meta {
    Meta { width, height, offset_top: 0., scale: 1. }
}

fn area() -> Bounds<Pixels> {
    Bounds::new(point(px(100.), px(50.)), size(px(800.), px(600.)))
}

#[test]
fn a_frame_the_size_of_the_panel_fills_it() {
    assert_eq!(fit(area(), &meta(800., 600.)), area());
}

#[test]
fn a_phone_sized_frame_sits_in_the_middle() {
    let shown = fit(area(), &meta(400., 600.));
    assert_eq!(shown, Bounds::new(point(px(300.), px(50.)), size(px(400.), px(600.))));
}

#[test]
fn a_frame_bigger_than_the_panel_shrinks_to_fit() {
    let shown = fit(area(), &meta(1600., 1200.));
    assert_eq!(shown.size, size(px(800.), px(600.)));
}

#[test]
fn a_point_maps_to_page_pixels() {
    assert_eq!(to_page(point(px(500.), px(350.)), area(), &meta(800., 600.)), Some((400., 300.)));
}

#[test]
fn a_point_on_a_shrunk_frame_maps_back_to_full_size() {
    assert_eq!(to_page(point(px(500.), px(350.)), area(), &meta(1600., 1200.)), Some((800., 600.)));
}

#[test]
fn a_point_beside_a_phone_frame_is_off_the_page() {
    assert_eq!(to_page(point(px(120.), px(100.)), area(), &meta(400., 600.)), None);
}

#[test]
fn the_page_is_sized_in_css_pixels_at_the_display_scale() {
    let view = View { width: 799.6, height: 600., scale: 2., ..View::default() };
    let metrics = view.metrics().expect("known");
    assert_eq!(metrics, json!({ "width": 800, "height": 600, "deviceScaleFactor": 2.0, "mobile": false }));
    assert_eq!(view.device_pixels(), (1599, 1200));
}

#[test]
fn an_unmeasured_view_sends_no_size() {
    assert_eq!(View::default().metrics(), None);
}

#[test]
fn page_cursors_become_the_window_cursors() {
    assert_eq!(cursor_style("pointer"), CursorStyle::PointingHand);
    assert_eq!(cursor_style("text"), CursorStyle::IBeam);
    assert_eq!(cursor_style("url(hand.png), pointer"), CursorStyle::PointingHand);
    assert_eq!(cursor_style("whatever"), CursorStyle::Arrow);
}
