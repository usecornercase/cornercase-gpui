use super::*;

#[test]
fn the_table_keeps_its_columns_where_every_part_just_fits_at_its_least() {
    assert_eq!(Shape::of(410.), Shape::Columns);
}

#[test]
fn a_pixel_less_stacks_the_table() {
    assert_eq!(Shape::of(409.), Shape::Stacked);
}

#[test]
fn the_table_keeps_its_columns_at_900_where_it_is_whole_today() {
    assert_eq!(Shape::of(442.), Shape::Columns);
}

#[test]
fn the_least_card_stacks_the_table() {
    assert_eq!(Shape::of(229.), Shape::Stacked);
}

#[test]
fn a_card_not_yet_measured_keeps_the_columns() {
    assert_eq!(Shape::of(widgets::WIDEST_CARD), Shape::Columns);
}

#[test]
fn a_card_of_no_width_stacks_the_table() {
    assert_eq!(Shape::of(0.), Shape::Stacked);
}
