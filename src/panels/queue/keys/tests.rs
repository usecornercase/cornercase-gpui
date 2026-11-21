use super::super::data::Body;
use super::{KeysTo, follows, keys_go, takes_keys};

#[test]
fn the_keys_go_to_the_prompt_else_the_filter_else_the_pages_button_and_wait_while_loading() {
    for body in [Body::Loading, Body::Empty, Body::NoMatch, Body::List] {
        assert_eq!(keys_go(true, true, body), KeysTo::Prompt, "{body:?}");
    }
    assert_eq!(keys_go(true, false, Body::Loading), KeysTo::Wait);
    assert_eq!(keys_go(true, false, Body::Empty), KeysTo::First);
    assert_eq!(keys_go(true, false, Body::NoMatch), KeysTo::Filter);
    assert_eq!(keys_go(true, false, Body::List), KeysTo::Filter);
}

#[test]
fn the_keys_wait_while_a_dialog_covers_the_sheet_or_another_sheet_shows() {
    for form in [true, false] {
        for body in [Body::Loading, Body::Empty, Body::NoMatch, Body::List] {
            assert_eq!(keys_go(false, form, body), KeysTo::Wait, "{form} {body:?}");
        }
    }
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

#[test]
fn a_change_of_target_across_a_dialog_takes_the_keys_and_one_on_top_only_when_the_old_target_had_them() {
    assert!(takes_keys(KeysTo::Filter, KeysTo::First, (true, false)));
    assert!(takes_keys(KeysTo::First, KeysTo::Filter, (true, false)));
    assert!(takes_keys(KeysTo::Filter, KeysTo::First, (false, true)));
    assert!(!takes_keys(KeysTo::Filter, KeysTo::First, (false, false)));
    assert!(!takes_keys(KeysTo::Filter, KeysTo::Filter, (true, true)));
    assert!(!takes_keys(KeysTo::Prompt, KeysTo::Filter, (true, true)));
}
