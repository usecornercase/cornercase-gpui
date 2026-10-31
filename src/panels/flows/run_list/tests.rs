use gpui::{ListOffset, px};

use super::*;

fn page(newest_first: &[u64]) -> Vec<Part> {
    let messages = newest_first.iter().map(|seq| Part::Message(*seq));
    [Part::Top, Part::Feed].into_iter().chain(messages).chain([Part::Files]).collect()
}

#[test]
fn a_new_message_is_one_insertion_at_the_top_of_the_feed() {
    assert_eq!(splices(&page(&[2, 1]), &page(&[3, 2, 1])), Some(vec![(2..2, 1)]));
}

#[test]
fn a_new_message_past_the_cap_drops_the_oldest() {
    assert_eq!(splices(&page(&[3, 2, 1]), &page(&[4, 3, 2])), Some(vec![(2..2, 1), (5..6, 0)]));
}

#[test]
fn the_same_page_needs_no_splice() {
    assert_eq!(splices(&page(&[2, 1]), &page(&[2, 1])), Some(Vec::new()));
}

#[test]
fn parts_out_of_order_need_a_reset() {
    assert_eq!(splices(&page(&[2, 1]), &page(&[1, 2])), None);
}

#[test]
fn a_new_message_keeps_the_view_on_the_message_it_showed() {
    let mut runs = RunList::new();
    runs.sync(Some(7), page(&[3, 2, 1]));
    runs.list.scroll_to(ListOffset { item_ix: 3, offset_in_item: px(6.) });
    runs.sync(Some(7), page(&[4, 3, 2, 1]));
    let top = runs.list.logical_scroll_top();
    assert_eq!((runs.list.item_count(), top.item_ix, top.offset_in_item), (7, 4, px(6.)));
}

#[test]
fn another_run_opens_at_its_top() {
    let mut runs = RunList::new();
    runs.sync(Some(7), page(&[2, 1]));
    runs.list.scroll_to(ListOffset { item_ix: 3, offset_in_item: px(6.) });
    runs.sync(Some(8), page(&[2, 1]));
    assert_eq!(runs.list.logical_scroll_top().item_ix, 0);
}
