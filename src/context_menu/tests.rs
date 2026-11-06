use gpui::{point, px};

use super::{ABOVE_POINTER, Entry, FRAME_PADDING, ROW_HEIGHT, SEPARATOR_HEIGHT, above, choice, item, step, submenu};

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

#[test]
fn the_keys_skip_a_plugins_name() {
    let entries = vec![item("Close", |_, _, _| {}), Entry::Heading("Notes".into()), item("Say", |_, _, _| {})];
    assert_eq!(step(&entries, Some(0), true), Some(2));
    assert_eq!(step(&entries, Some(2), false), Some(0));
}

#[test]
fn the_keys_reach_a_choice_whether_it_is_chosen_or_not() {
    let entries = [choice("work", true, |_, _, _| {}), Entry::Separator, choice("home", false, |_, _, _| {})];
    assert_eq!(step(&entries, Some(0), true), Some(2));
}

#[test]
fn only_a_choice_can_show_as_chosen() {
    let chosen = |entry: Entry| matches!(entry, Entry::Item { chosen: true, .. });
    assert!(chosen(choice("work", true, |_, _, _| {})));
    assert!(!chosen(choice("home", false, |_, _, _| {})));
    assert!(!chosen(item("Copy", |_, _, _| {})));
}

#[test]
fn a_menu_opened_above_the_pointer_ends_just_over_it() {
    let entries = [item("One", |_, _, _| {}), item("Two", |_, _, _| {}), Entry::Separator, item("More", |_, _, _| {})];
    let height = 3. * ROW_HEIGHT + SEPARATOR_HEIGHT + 2. * FRAME_PADDING;
    assert_eq!(above(point(px(10.), px(500.)), &entries), point(px(10.), px(500. - height - ABOVE_POINTER)));
}
