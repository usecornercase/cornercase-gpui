use super::{Entry, item, step, submenu};

fn entries() -> Vec<Entry> {
    vec![item("Copy", |_, _, _| {}), Entry::Separator, submenu("Move", Vec::new()), item("Close", |_, _, _| {})]
}

#[test]
fn the_keys_skip_what_cannot_be_chosen() {
    let entries = entries();
    assert_eq!(step(&entries, None, true), Some(0));
    assert_eq!(step(&entries, Some(0), true), Some(2));
    assert_eq!(step(&entries, Some(2), false), Some(0));
}

#[test]
fn the_keys_wrap_around_the_menu() {
    let entries = entries();
    assert_eq!(step(&entries, None, false), Some(3));
    assert_eq!(step(&entries, Some(3), true), Some(0));
    assert_eq!(step(&entries, Some(0), false), Some(3));
}

#[test]
fn a_menu_of_separators_has_nothing_to_choose() {
    assert_eq!(step(&[Entry::Separator], None, true), None);
}
