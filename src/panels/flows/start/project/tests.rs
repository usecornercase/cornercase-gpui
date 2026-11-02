use super::{Shape, shape};

#[test]
fn no_project_to_choose_shows_why() {
    assert_eq!(shape(0, false), Shape::Empty);
}

#[test]
fn the_only_project_chosen_is_a_fact_not_a_menu() {
    assert_eq!(shape(1, true), Shape::Fixed);
}

#[test]
fn several_projects_or_none_chosen_yet_open_a_menu() {
    assert_eq!(shape(3, true), Shape::Choose);
    assert_eq!(shape(1, false), Shape::Choose);
    assert_eq!(shape(2, false), Shape::Choose);
}
