use gpui::{KeyBinding, KeyContext};

use super::{
    Escaped, LINES_CONTEXT, NewLine, Submit, bindings, composed_selection, escaped, handed, prompt_keys, send_key,
    shared, word_end, word_start,
};
use crate::platform::Os;

#[test]
fn escape_reverts_an_edited_field_then_passes_on() {
    assert_eq!(escaped(false, "typed", Some("saved")), Escaped::Revert);
    assert_eq!(escaped(false, "saved", Some("saved")), Escaped::Pass);
}

#[test]
fn escape_clears_a_search_then_passes_on() {
    assert_eq!(escaped(true, "query", None), Escaped::Clear);
    assert_eq!(escaped(true, "", None), Escaped::Pass);
}

#[test]
fn escape_cancels_a_plain_field() {
    assert_eq!(escaped(false, "name", None), Escaped::Cancel);
}

#[test]
fn a_word_starts_after_the_blank_before_it() {
    assert_eq!(word_start("cargo test -q", 13, false), 11);
    assert_eq!(word_start("cargo test  ", 12, false), 6);
    assert_eq!(word_start("word", 4, false), 0);
    assert_eq!(word_start("año\u{3000}más", "año\u{3000}más".len(), false), "año\u{3000}".len());
}

#[test]
fn a_word_ends_at_the_blank_after_it() {
    assert_eq!(word_end("cargo test", 0, false), 5);
    assert_eq!(word_end("cargo test", 5, false), 10);
    assert_eq!(word_end("cargo  test -q", 5, false), 11);
    assert_eq!(word_end("word", 4, false), 4);
}

#[test]
fn a_selection_leaves_the_field_unless_it_is_empty_or_masked() {
    assert_eq!(shared("hello world", &(6..11), false), Some("world"));
    assert_eq!(shared("hello", &(2..2), false), None);
    assert_eq!(shared("secret", &(0..6), true), None);
}

#[test]
fn a_masked_field_is_one_word() {
    assert_eq!(word_start("made up words", 13, true), 0);
    assert_eq!(word_end("made up words", 0, true), 13);
}

#[test]
fn a_composed_selection_is_read_within_the_new_text() {
    for (new_text, utf16, start, want) in
        [("´", 1..1, 3, 5..5), ("´", 1..1, 6, 8..8), ("´", 1..1, 0, 2..2), ("´", 0..0, 3, 3..3), ("😀", 2..2, 4, 8..8)]
    {
        assert_eq!(composed_selection(new_text, Some(&utf16), start), want, "{new_text:?} {utf16:?} {start}");
    }
    assert_eq!(composed_selection("´", None, 3), 5..5);
}

#[test]
fn enter_breaks_the_line_before_it_can_submit() {
    for os in [Os::Mac, Os::Linux, Os::Windows] {
        let keys = bindings(os);
        let new_line = keys.iter().position(|key| key.action().partial_eq(&NewLine));
        let submit = keys.iter().position(|key| key.action().partial_eq(&Submit));
        assert!(new_line > submit, "gpui runs the later of two bindings at one depth first");
    }
}

#[test]
fn a_field_of_several_lines_answers_to_both_key_contexts() {
    let context = KeyContext::parse(LINES_CONTEXT).expect("context");
    assert!(context.contains("TextField") && context.contains("Multiline"));
}

#[test]
fn a_prompt_names_the_keys_that_break_and_send_it() {
    assert_eq!(prompt_keys(Os::Mac, "schedule"), "↩ for a new line, ⌘↩ to schedule");
    assert_eq!(prompt_keys(Os::Linux, "start"), "Enter for a new line, Ctrl+Enter to start");
    assert_eq!(prompt_keys(Os::Windows, "start"), prompt_keys(Os::Linux, "start"));
}

#[test]
fn a_prompt_is_sent_by_the_key_it_names() {
    assert_eq!(send_key(Os::Mac), ("cmd-enter", "⌘↩"));
    assert_eq!(send_key(Os::Linux), ("ctrl-enter", "Ctrl+Enter"));
    for os in [Os::Mac, Os::Linux, Os::Windows] {
        let wanted = KeyBinding::new(send_key(os).0, Submit, None);
        let bound =
            bindings(os).iter().any(|key| key.action().partial_eq(&Submit) && key.keystrokes() == wanted.keystrokes());
        assert!(bound, "{os:?} binds {}", send_key(os).0);
    }
}

#[test]
fn a_masked_field_hands_the_input_method_nothing() {
    for range in [0..14, 5..7, 0..0, 20..30] {
        assert_eq!(handed("made up secret", &range, true), None);
    }
    assert_eq!(handed("ab\u{b4}", &(2..3), true), None);
}

#[test]
fn a_plain_field_hands_the_input_method_its_text_as_before() {
    let content = "a\u{f1}\u{1f600}b";
    assert_eq!(handed(content, &(0..5), false), Some((content.to_string(), 0..5)));
    assert_eq!(handed(content, &(1..2), false), Some(("\u{f1}".to_string(), 1..2)));
    assert_eq!(handed(content, &(3..4), false), Some((String::new(), 4..4)));
    assert_eq!(handed(content, &(4..9), false), Some(("b".to_string(), 4..5)));
}
