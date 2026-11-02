use super::{hovered, step};

#[test]
fn the_arrows_start_at_either_end() {
    assert_eq!(step(None, 4, true), Some(0));
    assert_eq!(step(None, 4, false), Some(3));
}

#[test]
fn the_arrows_stop_at_the_ends() {
    assert_eq!(step(Some(3), 4, true), Some(3));
    assert_eq!(step(Some(0), 4, false), Some(0));
    assert_eq!(step(Some(1), 4, true), Some(2));
}

#[test]
fn a_list_without_choices_highlights_nothing() {
    assert_eq!(step(None, 0, true), None);
}

#[test]
fn the_pointer_takes_the_highlight_and_leaving_its_entry_drops_it() {
    assert_eq!(hovered(None, 2, true), Some(2));
    assert_eq!(hovered(Some(2), 2, false), None);
}

#[test]
fn leaving_another_entry_keeps_the_highlight() {
    assert_eq!(hovered(Some(1), 2, false), Some(1));
}
