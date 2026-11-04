use gpui::{Keystroke, Modifiers};
use serde_json::json;

use super::*;
use crate::platform::Os;

fn key(name: &str, typed: &str) -> Keystroke {
    Keystroke { modifiers: Modifiers::default(), key: name.into(), key_char: Some(typed.into()) }
}

#[test]
fn a_key_and_the_text_it_typed_go_as_one_key_down() {
    let mut typing = Typing::default();
    typing.pressed(&key("a", "a"), false);
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
    typing.pressed(&key("a", "a"), false);
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
    typing.pressed(&key("k", "k"), false);
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
    typing.pressed(&key("e", "e"), false);
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
fn a_key_up_goes_where_its_key_down_went_whatever_shift_says_at_release() {
    let mut typing = Typing::default();
    assert!(!typing.claim_down("t", false));
    assert!(!typing.claimed_up("t"));
    assert!(typing.claim_down("t", true));
    assert!(typing.claimed_up("t"));
    assert!(!typing.claimed_up("t"));
}

#[test]
fn a_key_down_the_page_gets_forgets_an_earlier_claim_on_its_key() {
    let mut typing = Typing::default();
    typing.claim_down("t", true);
    typing.claim_down("c", true);
    typing.claim_down("t", false);
    assert!(!typing.claimed_up("t"));
    assert!(typing.claimed_up("c"));
}

#[test]
fn the_window_keeps_its_command_shortcuts_and_the_page_gets_the_rest() {
    let cmd = |name: &str| Keystroke {
        modifiers: Modifiers { platform: true, ..Modifiers::default() },
        key: name.into(),
        key_char: None,
    };
    assert!(window_claims(&cmd("c"), false, Os::Mac));
    assert!(window_claims(&cmd("l"), false, Os::Mac));
    assert!(!window_claims(&cmd("a"), false, Os::Mac));
    assert!(!window_claims(&key("c", "c"), false, Os::Mac));
}

#[test]
fn on_linux_the_window_keeps_ctrl_shift_and_chrome_s_clipboard_keys() {
    let held = |name: &str, modifiers| Keystroke { modifiers, key: name.into(), key_char: None };
    let ctrl = Modifiers { control: true, ..Modifiers::default() };
    let ctrl_shift = Modifiers { shift: true, ..ctrl };
    assert!(window_claims(&held("c", ctrl), false, Os::Linux));
    assert!(window_claims(&held("t", ctrl_shift), true, Os::Linux));
    assert!(window_claims(&held("{", ctrl), true, Os::Linux));
    assert!(!window_claims(&held("{", ctrl), false, Os::Linux));
    assert!(!window_claims(&held("a", ctrl), false, Os::Linux));
    assert!(!window_claims(&held("z", ctrl), false, Os::Linux));
}

#[test]
fn a_held_key_whose_text_the_input_method_gives_goes_as_a_repeat() {
    let mut typing = Typing::default();
    typing.pressed(&key("a", "a"), true);
    let (_, params) = typing.commit("A").expect("sent");
    assert_eq!((params["text"].as_str(), params["autoRepeat"].as_bool()), (Some("A"), Some(true)));
}

#[test]
fn on_windows_alt_f4_is_left_to_the_window_so_it_closes() {
    let alt_f4 =
        Keystroke { modifiers: Modifiers { alt: true, ..Modifiers::default() }, key: "f4".into(), key_char: None };
    assert!(window_claims(&alt_f4, false, Os::Windows));
    assert!(!window_claims(&alt_f4, false, Os::Linux));
}
