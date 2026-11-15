use gpui::KeyContext;

use super::*;

#[test]
fn a_chat_field_answers_to_its_own_key_context() {
    let context = KeyContext::parse(Enter::Sends.key_context()).expect("context");
    assert!(context.contains("TextField") && context.contains("Chat") && !context.contains("Multiline"));
}

#[test]
fn a_chat_field_sends_nothing_while_the_input_method_composes() {
    assert!(!submits(Enter::Sends, true));
}

#[test]
fn a_chat_field_sends_when_nothing_is_composed() {
    assert!(submits(Enter::Sends, false));
}

#[test]
fn the_other_fields_submit_whatever_the_input_method_does() {
    assert!(submits(Enter::Breaks, true) && submits(Enter::Breaks, false));
}

#[test]
fn a_chat_field_of_one_row_or_none_passes_the_arrows_on() {
    assert!(passes_arrows(Enter::Sends, true, true));
    assert!(passes_arrows(Enter::Sends, false, false));
}

#[test]
fn a_chat_field_of_several_rows_keeps_the_arrows() {
    assert!(!passes_arrows(Enter::Sends, false, true));
}

#[test]
fn a_prompt_keeps_the_arrows_whatever_its_rows() {
    assert!(!passes_arrows(Enter::Breaks, true, false));
    assert!(!passes_arrows(Enter::Breaks, false, false));
}
