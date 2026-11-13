use super::{guarded, guards_keys};
use crate::platform::Os;

#[test]
fn a_masked_field_guards_its_keys_only_while_it_holds_the_focus_in_the_front_window() {
    for masked in [false, true] {
        for focused in [false, true] {
            for front in [false, true] {
                let expected = masked && focused && front;
                assert_eq!(guarded(Os::Mac, masked, focused, front), expected, "{masked} {focused} {front}");
            }
        }
    }
}

#[test]
fn a_field_that_is_not_masked_never_guards_its_keys() {
    for os in [Os::Mac, Os::Linux, Os::Windows] {
        assert!(!guards_keys(os, false));
        assert!(!guarded(os, false, true, true));
    }
}

#[test]
fn linux_and_windows_have_no_secure_event_input() {
    for os in [Os::Linux, Os::Windows] {
        assert!(!guards_keys(os, true));
        assert!(!guarded(os, true, true, true));
    }
}
