use super::*;

#[test]
fn entries_added_at_the_end_are_appended() {
    assert_eq!(changes(&[1, 2], &[1, 2, 3, 4]), Some((0, 2)));
}

#[test]
fn entries_dropped_from_the_front_are_removed() {
    assert_eq!(changes(&[1, 2, 3], &[3, 4]), Some((2, 1)));
}

#[test]
fn a_cleared_log_drops_everything_it_showed() {
    assert_eq!(changes(&[1, 2, 3], &[]), Some((3, 0)));
    assert_eq!(changes(&[1, 2, 3], &[7]), Some((3, 1)));
}

#[test]
fn an_unchanged_log_changes_nothing() {
    assert_eq!(changes(&[1, 2], &[1, 2]), Some((0, 0)));
}

#[test]
fn a_list_that_changed_in_the_middle_starts_again() {
    assert_eq!(changes(&[1, 2, 3], &[1, 3]), None);
}

#[test]
fn a_source_is_shown_by_its_file_and_line() {
    assert_eq!(short_source("http://localhost:5173/src/main.ts:12:5"), "main.ts:12");
    assert_eq!(short_source("http://localhost:5173/src/main.ts?t=1700:12:5"), "main.ts:12");
    assert_eq!(short_source("http://localhost:8783/:3"), "localhost:8783:3");
    assert_eq!(short_source("https://cdn.example/lib.js"), "lib.js");
}
