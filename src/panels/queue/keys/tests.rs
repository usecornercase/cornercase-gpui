use super::super::data::Body;
use super::{KeysTo, follows, keys_go};

#[test]
fn the_keys_go_to_the_prompt_else_the_filter_else_the_pages_button_and_wait_while_loading() {
    for body in [Body::Loading, Body::Empty, Body::NoMatch, Body::List] {
        assert_eq!(keys_go(true, body), KeysTo::Prompt, "{body:?}");
    }
    assert_eq!(keys_go(false, Body::Loading), KeysTo::Wait);
    assert_eq!(keys_go(false, Body::Empty), KeysTo::First);
    assert_eq!(keys_go(false, Body::NoMatch), KeysTo::Filter);
    assert_eq!(keys_go(false, Body::List), KeysTo::Filter);
}

#[test]
fn the_keys_follow_from_the_filter_to_the_pages_button_and_back_and_nowhere_else() {
    assert!(follows(KeysTo::Filter, KeysTo::First));
    assert!(follows(KeysTo::First, KeysTo::Filter));
    assert!(!follows(KeysTo::Filter, KeysTo::Filter));
    assert!(!follows(KeysTo::Prompt, KeysTo::Filter));
    assert!(!follows(KeysTo::Filter, KeysTo::Prompt));
    assert!(!follows(KeysTo::Wait, KeysTo::First));
}
