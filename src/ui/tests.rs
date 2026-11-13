use gpui::{Display, Styled as _, div};

use super::field_slot;

#[test]
fn a_fields_slot_in_a_row_is_a_flex_box() {
    assert_eq!(field_slot(div()).style().display, Some(Display::Flex));
}
