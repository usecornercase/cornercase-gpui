use super::*;
use crate::panels::flows::library::parse_library;

const LIST: &str = crate::test_util::fixture!("flows/personas/list.json");

fn picking(chosen: &str) -> PersonaPicking {
    PersonaPicking::new("1".into(), chosen, Some(&parse_library(LIST).expect("the list")))
}

#[test]
fn the_picker_offers_none_then_the_library_by_emoji_and_name() {
    let names: Vec<String> = picking("").rows().into_iter().map(|row| row.name).collect();
    assert_eq!(names, ["none", "🧪 Careful Tester", "👁️ Code Reviewer"]);
}

#[test]
fn a_role_without_a_persona_shows_none() {
    let picking = picking("");
    assert_eq!(picking.shown().name, "none");
    assert_eq!(picking.chosen_at(), Some(0));
    assert_eq!(picking.gone(), None);
}

#[test]
fn a_persona_the_library_lacks_is_not_among_yours() {
    let picking = picking("gone-one");
    assert_eq!(picking.gone(), Some(format!("gone-one: {GONE}")));
    assert_eq!(picking.shown().name, "gone-one");
    assert_eq!(picking.chosen_at(), None);
}

#[test]
fn nothing_is_called_missing_before_the_library_is_read() {
    assert_eq!(PersonaPicking::new("1".into(), "gone-one", None).gone(), None);
}

#[test]
fn choosing_none_clears_the_persona() {
    let mut picking = picking("careful-tester");
    picking.choose("");
    assert_eq!(picking.chosen(), "");
}
