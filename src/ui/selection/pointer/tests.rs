use gpui::{point, px};

use super::far_enough;

#[test]
fn a_press_that_barely_moved_is_still_a_click() {
    assert!(!far_enough(point(px(10.), px(10.)), point(px(11.), px(11.))));
}

#[test]
fn a_press_that_moved_three_pixels_is_a_drag() {
    assert!(far_enough(point(px(10.), px(10.)), point(px(13.), px(10.))));
    assert!(far_enough(point(px(10.), px(10.)), point(px(10.), px(6.))));
}
