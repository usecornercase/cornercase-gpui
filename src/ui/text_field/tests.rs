use gpui::KeyContext;

use super::{Escaped, LINES_CONTEXT, NewLine, Submit, bindings, escaped, shared, word_end, word_start};
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
    assert_eq!(word_start("cargo test -q", 13), 11);
    assert_eq!(word_start("cargo test  ", 12), 6);
    assert_eq!(word_start("word", 4), 0);
    assert_eq!(word_start("año\u{3000}más", "año\u{3000}más".len()), "año\u{3000}".len());
}

#[test]
fn a_word_ends_at_the_blank_after_it() {
    assert_eq!(word_end("cargo test", 0), 5);
    assert_eq!(word_end("cargo test", 5), 10);
    assert_eq!(word_end("cargo  test -q", 5), 11);
    assert_eq!(word_end("word", 4), 4);
}

#[test]
fn a_selection_goes_to_the_primary_selection_unless_it_is_empty_or_masked() {
    assert_eq!(shared("hello world", &(6..11), false), Some("world"));
    assert_eq!(shared("hello", &(2..2), false), None);
    assert_eq!(shared("secret", &(0..6), true), None);
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
