use std::time::Duration;

use super::*;

fn stamp(secs: u64, len: u64) -> (SystemTime, u64) {
    (SystemTime::UNIX_EPOCH + Duration::from_secs(secs), len)
}

fn seen(entries: &[(&str, (SystemTime, u64))]) -> Seen {
    entries.iter().map(|(path, stamp)| (PathBuf::from(path), *stamp)).collect()
}

#[test]
fn a_file_whose_time_or_size_moved_is_read_again() {
    let before = seen(&[("a", stamp(1, 10)), ("b", stamp(1, 10))]);
    let now = seen(&[("a", stamp(2, 10)), ("b", stamp(1, 11))]);
    assert_eq!(changed(&before, &now), vec![PathBuf::from("a"), PathBuf::from("b")]);
    assert!(changed(&before, &before).is_empty());
}

#[test]
fn a_file_that_went_away_is_reported() {
    let before = seen(&[("a", stamp(1, 10))]);
    assert_eq!(changed(&before, &Seen::new()), vec![PathBuf::from("a")]);
}

#[test]
fn a_new_file_in_the_folder_is_listed() {
    let now = seen(&[("new", stamp(1, 1))]);
    assert_eq!(changed(&Seen::new(), &now), vec![PathBuf::from("new")]);
}
