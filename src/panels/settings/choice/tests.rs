use super::step;

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
