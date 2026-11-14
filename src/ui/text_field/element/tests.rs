use gpui::px;

use super::reveal_room;

#[test]
fn a_field_asking_for_room_below_it_gets_that_room_revealed() {
    assert_eq!(reveal_room(Some(px(74.)), false), px(74.));
}

#[test]
fn a_field_asking_for_nothing_keeps_the_margin_of_its_kind() {
    assert_eq!(reveal_room(None, false), px(12.));
    assert_eq!(reveal_room(None, true), px(32.));
}
