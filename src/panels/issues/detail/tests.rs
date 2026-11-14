use super::scroller_id;
use crate::panels::issues::data::Issue;

fn issue(source: &str, key: &str) -> Issue {
    Issue { source: source.into(), key: key.into(), ..Issue::default() }
}

#[test]
fn each_issue_scrolls_a_header_box_of_its_own() {
    let name = "issue-head";
    assert_ne!(scroller_id(&issue("github", "#1"), name), scroller_id(&issue("github", "#2"), name));
}

#[test]
fn an_issue_of_another_tracker_with_the_same_key_scrolls_its_own_header_box() {
    let name = "issue-head";
    assert_ne!(scroller_id(&issue("github", "#7"), name), scroller_id(&issue("gitlab", "#7"), name));
}

#[test]
fn the_reading_area_of_an_issue_scrolls_apart_from_its_header() {
    let issue = issue("github", "#7");
    assert_ne!(scroller_id(&issue, "issue-head"), scroller_id(&issue, "issue-unread"));
}
