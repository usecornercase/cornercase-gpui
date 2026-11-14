use std::path::PathBuf;

use super::*;
use crate::test_util::fixture;

fn link(json: &str) -> Link {
    Link::parse(json).expect("a link")
}

#[test]
fn a_file_of_the_shown_workspace_opens_by_its_relative_path() {
    assert_eq!(link_path(&link(fixture!("link/inside.json")), Some(36)), PathBuf::from("src/main.rs"));
}

#[test]
fn a_file_of_another_workspace_opens_by_its_absolute_path() {
    assert_eq!(link_path(&link(fixture!("link/inside.json")), Some(5)), PathBuf::from("/Users/me/shop/src/main.rs"));
}

#[test]
fn a_file_outside_the_workspace_opens_by_its_absolute_path() {
    let json = r#"{"link":{"path":"/Users/me/plan.md","inside":false,"workspace":36,"row":0,"start":0,"end":9}}"#;
    assert_eq!(link_path(&link(json), Some(36)), PathBuf::from("/Users/me/plan.md"));
}

fn lines(text: &[&str]) -> Vec<String> {
    text.iter().map(ToString::to_string).collect()
}

#[test]
fn with_cut_lines_a_spot_is_found_by_counting_lines() {
    let lines = lines(&["a"; 100]);
    assert_eq!(top_for(&lines, (50, 0), Fit::Cut, 10), (40, 0));
}

#[test]
fn a_spot_near_the_start_leaves_the_file_at_its_top() {
    let lines = lines(&["a"; 100]);
    assert_eq!(top_for(&lines, (3, 0), Fit::Cut, 10), (0, 0));
}

#[test]
fn long_lines_above_a_spot_count_for_the_rows_they_take() {
    let long = "x".repeat(100);
    let lines = lines(&["a", &long, &long, "a", "a"]);
    assert_eq!(top_for(&lines, (4, 0), Fit::Wrap { columns: 10 }, 10), (2, 1));
}

#[test]
fn a_spot_far_inside_a_long_line_scrolls_into_that_line() {
    let long = "x".repeat(1000);
    let lines = lines(&["a", &long]);
    assert_eq!(top_for(&lines, (1, 505), Fit::Wrap { columns: 10 }, 10), (1, 40));
}

#[test]
fn a_refused_read_keeps_a_file_on_screen_and_says_why_when_there_is_none() {
    let refused = Content::Restarted("vps restarted".into());
    assert!(keeps_what_shows(Some(&refused), true));
    assert!(!keeps_what_shows(Some(&refused), false));
    assert!(keeps_what_shows(Some(&Content::Lost("x".into())), false));
    assert!(!keeps_what_shows(None, true));
}
