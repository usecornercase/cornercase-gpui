use gpui::{Bounds, Pixels, point, px, size};

use super::{Edge, Split, reach};

const LIST: Split = Split::new(400., 260., 800.);

fn container() -> Bounds<Pixels> {
    Bounds::new(point(px(100.), px(50.)), size(px(1000.), px(600.)))
}

fn assert_size(split: Split, expected: f32) {
    assert!((split.size() - expected).abs() < f32::EPSILON, "{} is not {expected}", split.size());
}

#[test]
fn a_split_starts_at_its_default() {
    assert_size(LIST, 400.);
    assert_eq!(LIST.chosen(), None);
}

#[test]
fn a_drag_follows_the_pointer_to_a_whole_pixel() {
    let mut split = LIST;
    assert!(split.drag(512.4, 1000.));
    assert_size(split, 512.);
}

#[test]
fn a_drag_stops_at_the_limits() {
    let mut split = LIST;
    split.drag(10., 1000.);
    assert_size(split, 260.);
    split.drag(5000., 5000.);
    assert_size(split, 800.);
}

#[test]
fn a_drag_leaves_room_for_the_other_side() {
    let mut split = LIST;
    split.drag(700., 500.);
    assert_size(split, 500.);
}

#[test]
fn a_split_keeps_its_minimum_when_there_is_no_room() {
    let mut split = LIST;
    split.drag(700., 100.);
    assert_size(split, 260.);
}

#[test]
fn a_drag_to_the_same_size_changes_nothing() {
    let mut split = LIST;
    assert!(split.drag(500., 1000.));
    assert!(!split.drag(500.2, 1000.));
}

#[test]
fn a_reset_goes_back_to_the_default() {
    let mut split = LIST;
    split.drag(500., 1000.);
    assert!(split.reset());
    assert_size(split, 400.);
    assert!(!split.reset());
}

#[test]
fn the_pointer_is_measured_from_the_fixed_edge() {
    let pointer = point(px(400.), px(200.));
    assert!((reach(Edge::Right, container(), pointer) - 300.).abs() < f32::EPSILON);
    assert!((reach(Edge::Left, container(), pointer) - 700.).abs() < f32::EPSILON);
    assert!((reach(Edge::Bottom, container(), pointer) - 150.).abs() < f32::EPSILON);
    assert!((reach(Edge::Top, container(), pointer) - 450.).abs() < f32::EPSILON);
}

#[test]
fn following_the_pointer_keeps_room_for_the_neighbour() {
    let mut split = LIST;
    assert!(split.follow(Edge::Right, container(), point(px(1050.), px(0.)), 320.));
    assert_size(split, 680.);
    split.follow(Edge::Bottom, container(), point(px(0.), px(250.)), 120.);
    assert_size(split, 260.);
}
