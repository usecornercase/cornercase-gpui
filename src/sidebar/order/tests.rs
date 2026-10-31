use super::*;

#[test]
fn a_row_dragged_down_lands_after_the_target() {
    let ids = [1, 2, 3, 4];
    assert_eq!(drop_place(&ids, 1, 3), Some(Place::Before(4)));
    assert_eq!(drop_place(&ids, 1, 4), Some(Place::End));
    assert_eq!(drop_place(&ids, 3, 4), Some(Place::End));
    assert!(lands_after(&ids, 1, 3));
}

#[test]
fn a_row_dragged_up_or_from_elsewhere_lands_before() {
    let ids = [1, 2, 3];
    assert_eq!(drop_place(&ids, 3, 1), Some(Place::Before(1)));
    assert_eq!(drop_place(&ids, 9, 2), Some(Place::Before(2)));
    assert_eq!(drop_place(&ids, 2, 2), None);
    assert_eq!(drop_place(&ids, 2, 7), None);
    assert!(!lands_after(&ids, 3, 1));
    assert!(!lands_after(&ids, 9, 1));
}
