use super::*;
use crate::test_util::fixture;

fn session() -> Session {
    serde_json::from_str(fixture!("merge_check/view.json")).expect("a view line")
}

fn conflicting() -> Reading {
    reading_of(Ok(fixture!("merge_check/conflicts.json").to_string()))
}

#[test]
fn it_names_each_pair_by_its_workspace_in_the_session() {
    let shown = Shown::take(&session(), 15, &conflicting(), Some(true)).expect("a workspace");
    assert_eq!(shown.name, "mc/checkout");
    assert!(shown.lines.contains(&Line::Heading("With mc/pricing".into(), Some("2 shared files".into()))));
    assert!(shown.lines.contains(&Line::Heading("With main".into(), Some("1 shared file".into()))));
}

#[test]
fn the_agent_is_told_the_pairs_by_name() {
    let shown = Shown::take(&session(), 15, &conflicting(), Some(true)).expect("a workspace");
    let ask = shown.ask.expect("something to ask");
    assert!(ask.contains("with mc/pricing: README.md (contents)"), "{ask}");
}

#[test]
fn nothing_is_shown_for_a_workspace_the_session_does_not_list() {
    assert!(Shown::take(&session(), 99, &conflicting(), Some(true)).is_none());
}

#[test]
fn it_stays_open_while_its_workspace_is_listed() {
    assert!(!gone(Some(&session()), 15, false));
}

#[test]
fn it_closes_when_its_workspace_leaves_or_is_being_removed() {
    assert!(gone(Some(&session()), 99, false));
    assert!(gone(Some(&session()), 15, true));
    assert!(gone(None, 15, false));
}
