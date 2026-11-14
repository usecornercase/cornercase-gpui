use gpui::{point, px};

use super::{ABOVE_POINTER, Entry, above, act, choice, danger, item, item_with_keys, note, resting, step, submenu};
use crate::ui::Icon;

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
fn a_menu_opened_above_the_pointer_has_its_bottom_just_over_it() {
    assert_eq!(above(point(px(10.), px(500.))), point(px(10.), px(500. - ABOVE_POINTER)));
}

#[test]
fn the_keys_skip_a_line_at_rest() {
    let entries = [resting("vps unreachable"), item("Rename", |_, _, _| {}), resting("Rename")];
    assert_eq!(step(&entries, None, true), Some(1));
    assert_eq!(step(&entries, Some(1), true), Some(1));
}

#[test]
fn the_keys_skip_a_note() {
    let entries = [item("One", |_, _, _| {}), note("needs a fix"), item("Two", |_, _, _| {})];
    assert_eq!(step(&entries, Some(0), true), Some(2));
}

#[test]
fn a_plain_entry_has_no_icon_or_keys() {
    let plain = [
        item("Copy", |_, _, _| {}),
        danger("Close", |_, _, _| {}),
        choice("work", true, |_, _, _| {}),
        act("Run", Vec::new()),
    ];
    for entry in plain {
        assert!(matches!(entry, Entry::Item { icon: None, keys: None, .. }));
    }
}

#[test]
fn an_entry_can_carry_its_icon_and_keys() {
    let entry = item_with_keys(Icon::Usage, "Usage", Some("⌘U".into()), |_, _, _| {});
    let Entry::Item { icon, keys, danger, chosen, .. } = entry else { panic!("an item") };
    assert_eq!(icon, Some(Icon::Usage));
    assert_eq!(keys.as_ref().map(AsRef::as_ref), Some("⌘U"));
    assert!(!danger && !chosen);
}

#[test]
fn the_keys_reach_an_entry_with_keys() {
    let entries = [
        item_with_keys(Icon::Usage, "Usage", None, |_, _, _| {}),
        Entry::Separator,
        item_with_keys(Icon::Ports, "Ports", None, |_, _, _| {}),
    ];
    assert_eq!(step(&entries, Some(0), true), Some(2));
    assert_eq!(step(&entries, Some(2), false), Some(0));
}
