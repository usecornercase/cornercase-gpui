use super::*;

#[test]
fn a_script_keeps_its_width_where_the_card_has_room_for_it() {
    assert_eq!(script_width(300.), 300.);
    assert_eq!(script_width(710.), 300.);
}

#[test]
fn a_script_narrows_to_a_card_under_its_width() {
    assert_eq!(script_width(299.), 299.);
    assert_eq!(script_width(229.), 229.);
}
