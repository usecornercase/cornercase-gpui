use super::text_for_pane;

#[test]
fn text_reaches_the_pane() {
    assert_eq!(text_for_pane("ls", false).as_deref(), Some("ls"));
}

#[test]
fn text_typed_under_a_sheet_or_dialog_does_not_reach_the_pane() {
    assert_eq!(text_for_pane("ls", true), None);
}

#[test]
fn control_characters_are_dropped() {
    assert_eq!(text_for_pane("a\u{7f}b\n", false).as_deref(), Some("ab"));
    assert_eq!(text_for_pane("\r", false), None);
}
