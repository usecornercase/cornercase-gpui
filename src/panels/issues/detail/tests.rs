use super::head_id;

#[test]
fn each_issue_scrolls_a_header_box_of_its_own() {
    assert_ne!(head_id("github", "#1"), head_id("github", "#2"));
}

#[test]
fn an_issue_of_another_tracker_with_the_same_key_scrolls_its_own_header_box() {
    assert_ne!(head_id("github", "#7"), head_id("gitlab", "#7"));
}
