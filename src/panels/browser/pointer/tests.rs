use gpui::{Modifiers, MouseButton, NavigationDirection, ScrollDelta, point, px};
use serde_json::json;

use super::*;

fn press(button: Option<MouseButton>, buttons: u8, clicks: usize) -> Value {
    let modifiers = Modifiers { shift: true, ..Modifiers::default() };
    mouse(&Mouse { kind: "mousePressed", at: (10., 20.), button, buttons, clicks, modifiers })
}

#[test]
fn buttons_have_the_devtools_names_and_bits() {
    assert_eq!(button(MouseButton::Left), ("left", 1));
    assert_eq!(button(MouseButton::Right), ("right", 2));
    assert_eq!(button(MouseButton::Middle), ("middle", 4));
    assert_eq!(button(MouseButton::Navigate(NavigationDirection::Back)), ("back", 8));
}

#[test]
fn a_press_carries_its_button_clicks_and_modifiers() {
    let event = press(Some(MouseButton::Left), 1, 2);
    assert_eq!(
        event,
        json!({
            "type": "mousePressed", "x": 10.0, "y": 20.0, "button": "left", "buttons": 1,
            "clickCount": 2, "modifiers": 8, "pointerType": "mouse"
        })
    );
}

#[test]
fn a_move_without_a_button_names_none() {
    assert_eq!(press(None, 0, 0)["button"], "none");
}

#[test]
fn the_wheel_scrolls_the_way_the_fingers_move() {
    let event = wheel((5., 6.), ScrollDelta::Pixels(point(px(0.), px(-30.))), Modifiers::default());
    assert_eq!((event["deltaX"].as_f64(), event["deltaY"].as_f64()), (Some(0.), Some(30.)));
}

#[test]
fn a_wheel_in_lines_moves_forty_pixels_a_line() {
    let event = wheel((0., 0.), ScrollDelta::Lines(point(1., -3.)), Modifiers::default());
    assert_eq!((event["deltaX"].as_f64(), event["deltaY"].as_f64()), (Some(-40.), Some(120.)));
}

#[test]
fn queued_wheels_add_up_and_keep_the_latest_position() {
    let first = wheel((1., 1.), ScrollDelta::Pixels(point(px(-2.), px(-10.))), Modifiers::default());
    let next = wheel((3., 4.), ScrollDelta::Pixels(point(px(0.), px(-5.))), Modifiers::default());
    let sum = merged(Some(first), next);
    assert_eq!((sum["x"].as_f64(), sum["deltaX"].as_f64(), sum["deltaY"].as_f64()), (Some(3.), Some(2.), Some(15.)));
}

#[test]
fn nothing_queued_leaves_the_wheel_as_it_is() {
    let next = wheel((3., 4.), ScrollDelta::Pixels(point(px(0.), px(-5.))), Modifiers::default());
    assert_eq!(merged(None, next.clone()), next);
}

#[test]
fn a_held_button_means_a_drag() {
    let pointer = Pointer { buttons: 1, ..Pointer::default() };
    assert!(pointer.dragging());
    assert!(!Pointer::default().dragging());
}
