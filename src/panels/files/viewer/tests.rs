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
