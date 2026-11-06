use super::{Notes, count_text};
use crate::panels::PanelKind;

fn changed(files: u64) -> Notes {
    Notes { changed: files, ..Notes::default() }
}

#[test]
fn no_changes_show_no_count() {
    assert_eq!(count_text(0), None);
    assert!(changed(0).mark(PanelKind::Changes).is_none());
}

#[test]
fn a_few_changes_show_their_number() {
    assert_eq!(count_text(9).as_deref(), Some("9"));
}

#[test]
fn from_ten_changes_the_count_stops() {
    assert_eq!(count_text(10).as_deref(), Some("9+"));
}

#[test]
fn only_the_changes_icon_shows_the_count() {
    assert!(changed(3).mark(PanelKind::Changes).is_some());
    assert!(changed(3).mark(PanelKind::Files).is_none());
}

#[test]
fn the_exact_count_is_in_the_tooltip() {
    assert_eq!(changed(1).named(PanelKind::Changes), "Changes · 1 file");
    assert_eq!(changed(12).named(PanelKind::Changes), "Changes · 12 files");
    assert_eq!(changed(0).named(PanelKind::Changes), "Changes");
    assert_eq!(changed(12).named(PanelKind::Files), "Files");
}

#[test]
fn the_todo_icon_shows_a_dot_while_items_wait() {
    assert!(Notes { todo: true, ..Notes::default() }.mark(PanelKind::Todo).is_some());
    assert!(Notes::default().mark(PanelKind::Todo).is_none());
}
