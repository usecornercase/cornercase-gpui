use gpui::{Keystroke, Modifiers};
use serde_json::json;

use super::*;

fn stroke(key: &str, key_char: Option<&str>, modifiers: Modifiers) -> Keystroke {
    Keystroke { modifiers, key: key.to_string(), key_char: key_char.map(str::to_string) }
}

fn plain(key: &str, key_char: &str) -> Keystroke {
    stroke(key, Some(key_char), Modifiers::default())
}

fn bare(key: &str, modifiers: Modifiers) -> Keystroke {
    stroke(key, None, modifiers)
}

fn alt() -> Modifiers {
    Modifiers { alt: true, ..Modifiers::default() }
}

fn cmd() -> Modifiers {
    Modifiers { platform: true, ..Modifiers::default() }
}

fn shift_cmd() -> Modifiers {
    Modifiers { platform: true, shift: true, ..Modifiers::default() }
}

#[test]
fn modifiers_follow_the_devtools_bits() {
    let all = Modifiers { alt: true, control: true, platform: true, shift: true, function: false };
    assert_eq!(modifiers(all), 15);
    assert_eq!(modifiers(alt()), 1);
    assert_eq!(modifiers(Modifiers { shift: true, ..Modifiers::default() }), 8);
}

#[test]
fn a_letter_names_its_key_code_and_virtual_key() {
    let key = describe(&plain("a", "a"));
    assert_eq!(key, Key { name: "a".into(), code: "KeyA".into(), vk: 65 });
}

#[test]
fn a_shifted_letter_keeps_its_code_and_prints_upper_case() {
    let shifted = stroke("a", Some("A"), Modifiers { shift: true, ..Modifiers::default() });
    assert_eq!(describe(&shifted), Key { name: "A".into(), code: "KeyA".into(), vk: 65 });
}

#[test]
fn a_shifted_digit_comes_from_its_digit_key() {
    assert_eq!(describe(&plain("!", "!")), Key { name: "!".into(), code: "Digit1".into(), vk: 49 });
    assert_eq!(describe(&plain(")", ")")), Key { name: ")".into(), code: "Digit0".into(), vk: 48 });
}

#[test]
fn an_option_character_prints_what_the_layout_types() {
    let at = stroke("2", Some("@"), alt());
    assert_eq!(describe(&at), Key { name: "@".into(), code: "Digit2".into(), vk: 50 });
}

#[test]
fn punctuation_and_space_have_their_codes() {
    assert_eq!(describe(&plain("/", "/")).code, "Slash");
    assert_eq!(describe(&plain("?", "?")).vk, 191);
    assert_eq!(describe(&plain("space", " ")), Key { name: " ".into(), code: "Space".into(), vk: 32 });
}

#[test]
fn named_keys_use_the_dom_names() {
    assert_eq!(describe(&bare("left", alt())), Key { name: "ArrowLeft".into(), code: "ArrowLeft".into(), vk: 37 });
    assert_eq!(describe(&bare("f5", Modifiers::default())), Key { name: "F5".into(), code: "F5".into(), vk: 116 });
    assert_eq!(describe(&plain("enter", "\n")).name, "Enter");
}

#[test]
fn a_key_outside_a_us_layout_keeps_its_character_without_a_code() {
    assert_eq!(describe(&plain("ñ", "ñ")), Key { name: "ñ".into(), code: String::new(), vk: 0 });
}

#[test]
fn a_command_key_is_a_raw_key_down_with_its_editing_command() {
    let event = key_down(&bare("a", cmd()), false);
    assert_eq!(event["type"], "rawKeyDown");
    assert_eq!(event["modifiers"], 4);
    assert_eq!(event["commands"], json!(["selectAll"]));
    assert!(event.get("text").is_none());
}

#[test]
fn shift_command_z_redoes() {
    assert_eq!(commands(&bare("z", shift_cmd())), ["redo"]);
    assert_eq!(commands(&bare("z", cmd())), ["undo"]);
}

#[test]
fn option_arrows_move_by_words_and_command_arrows_by_lines() {
    assert_eq!(commands(&bare("left", alt())), ["moveWordLeft"]);
    assert_eq!(commands(&bare("right", cmd())), ["moveToRightEndOfLine"]);
    assert_eq!(commands(&bare("down", cmd())), ["moveToEndOfDocument"]);
    let selecting = Modifiers { alt: true, shift: true, ..Modifiers::default() };
    assert_eq!(commands(&bare("right", selecting)), ["moveWordRightAndModifySelection"]);
}

#[test]
fn option_up_takes_two_commands() {
    assert_eq!(commands(&bare("up", alt())), ["moveBackward", "moveToBeginningOfParagraph"]);
}

#[test]
fn deleting_words_and_lines_has_commands() {
    assert_eq!(commands(&bare("backspace", alt())), ["deleteWordBackward"]);
    assert_eq!(commands(&bare("backspace", cmd())), ["deleteToBeginningOfLine"]);
}

#[test]
fn a_key_without_a_command_sends_none() {
    assert_eq!(commands(&bare("k", cmd())), Vec::<&str>::new());
}

#[test]
fn enter_carries_a_carriage_return_and_no_command_so_forms_submit() {
    let event = key_down(&plain("enter", "\n"), false);
    assert_eq!((event["type"].as_str(), event["text"].as_str()), (Some("keyDown"), Some("\r")));
    assert_eq!(event["commands"], json!([]));
}

#[test]
fn typed_text_goes_with_its_key() {
    let event = typed(&stroke("2", Some("@"), alt()), "@", false);
    assert_eq!(event["type"], "keyDown");
    assert_eq!(
        (event["text"].as_str(), event["key"].as_str(), event["code"].as_str()),
        (Some("@"), Some("@"), Some("Digit2"))
    );
    assert_eq!(event["modifiers"], 1);
}

#[test]
fn a_release_names_the_key() {
    let event = key_up(&plain("a", "a"));
    assert_eq!((event["type"].as_str(), event["code"].as_str()), (Some("keyUp"), Some("KeyA")));
}

#[test]
fn printable_keys_are_left_to_the_input_method() {
    assert_eq!(route(&plain("a", "a"), false), Route::Text);
    assert_eq!(route(&stroke("2", Some("@"), alt()), false), Route::Text);
    assert_eq!(route(&plain("space", " "), false), Route::Text);
}

#[test]
fn a_held_printable_key_repeats_its_text() {
    assert_eq!(route(&plain("a", "a"), true), Route::Repeat("a".into()));
}

#[test]
fn named_command_and_control_keys_go_as_keys() {
    assert_eq!(route(&plain("enter", "\n"), false), Route::Key);
    assert_eq!(route(&bare("left", alt()), false), Route::Key);
    assert_eq!(route(&bare("a", cmd()), false), Route::Key);
    assert_eq!(route(&bare("a", Modifiers { control: true, ..Modifiers::default() }), false), Route::Key);
}

#[test]
fn every_command_chord_lists_its_modifiers_in_order() {
    let order = ["ctrl-", "alt-", "shift-", "cmd-"];
    for (chord, _) in COMMANDS {
        let positions: Vec<usize> = order.iter().filter_map(|m| chord.find(m)).collect();
        assert!(positions.windows(2).all(|w| w[0] < w[1]), "{chord} lists its modifiers out of order");
    }
}
