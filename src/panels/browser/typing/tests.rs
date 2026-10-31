use gpui::{Keystroke, Modifiers};
use serde_json::json;

use super::*;

fn key(name: &str, typed: &str) -> Keystroke {
    Keystroke { modifiers: Modifiers::default(), key: name.into(), key_char: Some(typed.into()) }
}

#[test]
fn a_key_and_the_text_it_typed_go_as_one_key_down() {
    let mut typing = Typing::default();
    typing.pressed(&key("a", "a"));
    let (method, params) = typing.commit("a").expect("sent");
    assert_eq!(method, "Input.dispatchKeyEvent");
    assert_eq!(
        (params["type"].as_str(), params["text"].as_str(), params["code"].as_str()),
        (Some("keyDown"), Some("a"), Some("KeyA"))
    );
}

#[test]
fn the_key_is_used_once() {
    let mut typing = Typing::default();
    typing.pressed(&key("a", "a"));
    typing.commit("a");
    assert_eq!(typing.commit("ö"), Some(("Input.insertText", json!({ "text": "ö" }))));
}

#[test]
fn text_without_a_key_is_inserted() {
    let mut typing = Typing::default();
    assert_eq!(typing.commit("🙂"), Some(("Input.insertText", json!({ "text": "🙂" }))));
}

#[test]
fn marked_text_becomes_a_composition() {
    let mut typing = Typing::default();
    typing.pressed(&key("k", "k"));
    let (method, params) = typing.compose("か", None);
    assert_eq!(
        (method, params),
        ("Input.imeSetComposition", json!({ "text": "か", "selectionStart": 1, "selectionEnd": 1 }))
    );
    assert_eq!(typing.marked_range(), Some(0..1));
}

#[test]
fn committing_a_composition_inserts_its_text_not_a_key() {
    let mut typing = Typing::default();
    typing.compose("´", None);
    typing.pressed(&key("e", "e"));
    assert_eq!(typing.commit("é"), Some(("Input.insertText", json!({ "text": "é" }))));
    assert_eq!(typing.marked_range(), None);
}

#[test]
fn a_composition_committed_empty_still_ends() {
    let mut typing = Typing::default();
    typing.compose("に", None);
    assert_eq!(typing.commit(""), Some(("Input.insertText", json!({ "text": "" }))));
}

#[test]
fn control_characters_from_the_input_method_are_dropped() {
    let mut typing = Typing::default();
    assert_eq!(typing.commit("\n"), None);
}

#[test]
fn unmarking_commits_what_was_marked() {
    let mut typing = Typing::default();
    typing.compose("にほん", Some(0..3));
    assert_eq!(typing.unmark(), Some(("Input.insertText", json!({ "text": "にほん" }))));
    assert_eq!(typing.unmark(), None);
}

#[test]
fn the_window_keeps_its_command_shortcuts_and_the_page_gets_the_rest() {
    let cmd = |name: &str| Keystroke {
        modifiers: Modifiers { platform: true, ..Modifiers::default() },
        key: name.into(),
        key_char: None,
    };
    assert!(window_claims(&cmd("c")));
    assert!(window_claims(&cmd("l")));
    assert!(!window_claims(&cmd("a")));
    assert!(!window_claims(&key("c", "c")));
}
