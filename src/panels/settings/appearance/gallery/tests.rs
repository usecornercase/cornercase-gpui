use super::*;

#[test]
fn four_cards_share_a_line_where_four_fit() {
    assert_eq!(per_line(686.), 4);
    assert_eq!(per_line(710.), 4);
}

#[test]
fn a_pixel_under_four_cards_a_line_holds_three() {
    assert_eq!(per_line(685.), 3);
}

#[test]
fn the_least_card_holds_one_theme_a_line() {
    assert_eq!(per_line(229.), 1);
}

#[test]
fn a_card_of_no_width_still_holds_one_theme_a_line() {
    assert_eq!(per_line(0.), 1);
}
