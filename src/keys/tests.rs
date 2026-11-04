use gpui::{Keystroke, Modifiers};
use serde_json::{Value, json};

use super::key_event;
use crate::platform::Os;

fn stroke(key: &str, typed: Option<&str>, modifiers: Modifiers) -> Keystroke {
    Keystroke { modifiers, key: key.into(), key_char: typed.map(str::to_string) }
}

fn sent(code: &str, mods: &[&str]) -> Value {
    json!({ "key": { "code": code, "mods": mods } })
}

fn alt_shift() -> Modifiers {
    Modifiers { alt: true, shift: true, ..Modifiers::default() }
}

#[test]
fn plain_text_is_left_to_the_input_method() {
    for os in [Os::Mac, Os::Linux] {
        assert_eq!(key_event(&stroke("a", Some("a"), Modifiers::default()), os), None);
        assert_eq!(key_event(&stroke("@", Some("@"), Modifiers::default()), os), None);
    }
}

#[test]
fn ctrl_sends_the_key_itself() {
    let ctrl = Modifiers { control: true, ..Modifiers::default() };
    assert_eq!(key_event(&stroke("c", None, ctrl), Os::Linux), Some(sent("c", &["ctrl"])));
    let ctrl_shift = Modifiers { shift: true, ..ctrl };
    assert_eq!(key_event(&stroke("y", None, ctrl_shift), Os::Linux), Some(sent("y", &["ctrl", "shift"])));
}

#[test]
fn alt_with_a_key_that_types_itself_is_meta() {
    let alt = Modifiers { alt: true, ..Modifiers::default() };
    for os in [Os::Mac, Os::Linux] {
        assert_eq!(key_event(&stroke("b", Some("b"), alt), os), Some(sent("b", &["alt"])));
    }
}

#[test]
fn on_linux_alt_is_meta_for_any_character() {
    assert_eq!(key_event(&stroke("a", Some("A"), alt_shift()), Os::Linux), Some(sent("A", &["alt", "shift"])));
    let alt = Modifiers { alt: true, ..Modifiers::default() };
    assert_eq!(key_event(&stroke(";", Some("ñ"), alt), Os::Linux), Some(sent("ñ", &["alt"])));
}

#[test]
fn on_macos_option_characters_stay_with_the_input_method() {
    assert_eq!(key_event(&stroke("a", Some("Å"), alt_shift()), Os::Mac), None);
}

#[test]
fn on_windows_alt_with_a_key_is_meta_as_on_linux() {
    assert_eq!(
        key_event(&stroke("b", Some("B"), alt_shift()), Os::Windows),
        key_event(&stroke("b", Some("B"), alt_shift()), Os::Linux)
    );
}

#[test]
fn on_windows_a_character_alt_gr_types_as_ctrl_alt_goes_as_text() {
    let ctrl_alt = Modifiers { control: true, alt: true, ..Modifiers::default() };
    assert_eq!(key_event(&stroke("e", Some("é"), ctrl_alt), Os::Windows), None);
    assert_eq!(key_event(&stroke("e", None, ctrl_alt), Os::Windows), Some(sent("e", &["ctrl", "alt"])));
    assert_eq!(key_event(&stroke("e", Some("é"), ctrl_alt), Os::Linux), Some(sent("e", &["ctrl", "alt"])));
}
