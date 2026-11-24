use super::reveals;

#[test]
fn a_picker_reveals_itself_when_tab_gives_it_the_focus() {
    assert!(reveals(true, false, false));
}

#[test]
fn a_picker_pressed_open_or_already_focused_does_not_scroll() {
    assert!(!reveals(true, false, true));
    assert!(!reveals(true, true, false));
    assert!(!reveals(false, false, false));
}
