use super::{Minded, minded, told};

#[test]
fn the_first_notify_redraws_a_part() {
    let mut last = None;
    assert!(told(&mut last, 3_u64));
    assert_eq!(last, Some(3));
}

#[test]
fn a_part_redraws_only_when_its_key_moves() {
    let mut last = Some(3_u64);
    assert!(!told(&mut last, 3));
    assert!(told(&mut last, 4));
}

#[test]
fn the_first_frame_records_the_key() {
    let mut last = None;
    assert_eq!(minded(&mut last, 1_u64), Minded::First);
    assert_eq!(minded(&mut last, 1), Minded::Same);
}

#[test]
fn a_key_moved_without_a_notify_is_missed() {
    let mut last = Some(1_u64);
    assert_eq!(minded(&mut last, 2), Minded::Missed);
    assert_eq!(minded(&mut last, 2), Minded::Same);
}
