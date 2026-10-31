use super::{Entry, item, step, submenu};

fn entries() -> Vec<Entry> {
    vec![
        Entry::Note("why".into()),
        item("Copy", |_, _, _| {}),
        Entry::Separator,
        submenu("Move", Vec::new()),
        item("Close", |_, _, _| {}),
    ]
}

#[test]
fn the_keys_skip_what_cannot_be_chosen() {
    let entries = entries();
    assert_eq!(step(&entries, None, true), Some(1));
    assert_eq!(step(&entries, Some(1), true), Some(3));
    assert_eq!(step(&entries, Some(3), false), Some(1));
}

#[test]
fn the_keys_wrap_around_the_menu() {
    let entries = entries();
    assert_eq!(step(&entries, None, false), Some(4));
    assert_eq!(step(&entries, Some(4), true), Some(1));
    assert_eq!(step(&entries, Some(1), false), Some(4));
}

#[test]
fn a_menu_of_notes_has_nothing_to_choose() {
    assert_eq!(step(&[Entry::Note("none".into())], None, true), None);
}
