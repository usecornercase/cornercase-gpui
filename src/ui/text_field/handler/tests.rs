use super::{press_and_hold, taken};

#[test]
fn a_masked_field_turns_press_and_hold_off_and_a_plain_one_keeps_it() {
    assert!(!press_and_hold(true));
    assert!(press_and_hold(false));
}

#[test]
fn a_masked_field_drops_a_text_of_control_characters_only_and_keeps_an_empty_one_that_deletes_a_pre_edit() {
    for text in ["\n", "\t", "\r\n"] {
        assert!(!taken(text, true), "{text:?}");
    }
    for text in ["", "e", "\u{e9}", " ", "abc"] {
        assert!(taken(text, true), "{text:?}");
    }
    for text in ["\n", "\t", "e", ""] {
        assert!(taken(text, false), "{text:?}");
    }
}
