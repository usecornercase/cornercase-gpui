use super::team_columns;

#[test]
fn the_team_takes_two_columns_where_two_cards_fit() {
    assert_eq!(team_columns(640.), 2);
    assert_eq!(team_columns(480.), 2);
}

#[test]
fn a_narrow_page_stacks_the_team_in_one_column() {
    assert_eq!(team_columns(479.), 1);
    assert_eq!(team_columns(330.), 1);
}
