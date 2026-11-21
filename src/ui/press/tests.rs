use gpui::{Modifiers, SharedString};

use super::{Arm, Key, key_of};

fn plain() -> Modifiers {
    Modifiers::default()
}

fn name(text: &'static str) -> SharedString {
    SharedString::from(text)
}

#[test]
fn return_and_space_press_and_tab_moves_on() {
    assert_eq!(key_of("enter", plain()), Key::Press);
    assert_eq!(key_of("space", plain()), Key::Press);
    assert_eq!(key_of("tab", plain()), Key::Next);
    assert_eq!(key_of("tab", Modifiers { shift: true, ..plain() }), Key::Previous);
    assert_eq!(key_of("a", plain()), Key::Other);
    assert_eq!(key_of("escape", plain()), Key::Other);
}

#[test]
fn a_modified_return_presses_nothing() {
    assert_eq!(key_of("enter", Modifiers { platform: true, ..plain() }), Key::Other);
    assert_eq!(key_of("enter", Modifiers { control: true, ..plain() }), Key::Other);
    assert_eq!(key_of("enter", Modifiers { alt: true, ..plain() }), Key::Other);
    assert_eq!(key_of("space", Modifiers { shift: true, ..plain() }), Key::Other);
    assert_eq!(key_of("tab", Modifiers { control: true, ..plain() }), Key::Other);
    assert_eq!(key_of("tab", Modifiers { shift: true, control: true, ..plain() }), Key::Other);
}

#[test]
fn a_button_answers_the_release_of_a_key_that_went_down_on_it() {
    let mut arm = Arm::default();
    arm.keystroke();
    arm.down(&name("a"), false);
    assert!(arm.up("a"));
    assert!(!arm.up("a"));
}

#[test]
fn a_release_without_its_key_down_presses_nothing() {
    let mut arm = Arm::default();
    arm.keystroke();
    assert!(!arm.up("a"));
}

#[test]
fn a_held_key_presses_once_at_its_release() {
    let mut arm = Arm::default();
    arm.keystroke();
    arm.down(&name("a"), false);
    arm.keystroke();
    arm.down(&name("a"), true);
    arm.keystroke();
    arm.down(&name("a"), true);
    assert!(arm.up("a"));
    assert!(!arm.up("a"));
}

#[test]
fn a_repeat_that_reaches_a_button_it_did_not_go_down_on_presses_nothing() {
    let mut arm = Arm::default();
    arm.keystroke();
    arm.keystroke();
    arm.down(&name("a"), true);
    assert!(!arm.up("a"));
}

#[test]
fn an_arm_left_behind_does_not_outlive_the_next_key() {
    let mut arm = Arm::default();
    arm.keystroke();
    arm.down(&name("a"), false);
    arm.keystroke();
    assert!(!arm.up("a"));

    let mut arm = Arm::default();
    arm.keystroke();
    arm.down(&name("a"), false);
    arm.keystroke();
    arm.keystroke();
    arm.down(&name("a"), true);
    assert!(!arm.up("a"));
}

#[test]
fn another_buttons_release_presses_nothing() {
    let mut arm = Arm::default();
    arm.keystroke();
    arm.down(&name("a"), false);
    assert!(!arm.up("b"));
    assert!(!arm.up("a"));
}
