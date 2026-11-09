use super::*;
use crate::test_util::fixture;

fn read() -> IssueThere {
    serde_json::from_str(fixture!("machines/there_issue.json")).expect("what the machine answered")
}

#[test]
fn a_machines_issue_gives_the_prompt_and_branch_its_templates_made() {
    let read = read();
    assert_eq!(read.branch(), "ana/7-fix-the-login");
    assert_eq!(read.task(), "Fix #7: Fix the login\n\nRead CLAUDE.md before you start, then open a pull request.");
}

#[test]
fn an_answer_without_its_issue_cannot_be_read() {
    assert!(serde_json::from_str::<IssueThere>(r#"{"prompt": "Fix it"}"#).is_err());
    assert!(serde_json::from_str::<IssueThere>("[1]").is_err());
}

#[test]
fn the_prompt_line_is_one_short_line_and_a_blank_prompt_has_none() {
    assert_eq!(
        read().prompt_line().as_deref(),
        Some("Fix #7: Fix the login Read CLAUDE.md before you start, then open a pull request.")
    );
    let long = IssueThere { prompt: "word ".repeat(100), ..IssueThere::default() };
    assert!(long.prompt_line().is_some_and(|line| line.chars().count() <= 161 && line.ends_with('…')));
    let blank = IssueThere { prompt: " \n".into(), ..IssueThere::default() };
    assert_eq!(blank.prompt_line(), None);
}

#[test]
fn an_untouched_field_takes_the_new_prefill() {
    assert_eq!(follow("ana/7", "ana/7", "bob/7").as_deref(), Some("bob/7"));
    assert_eq!(follow("ana/7 \n", "ana/7", "").as_deref(), Some(""));
}

#[test]
fn an_edited_field_keeps_what_was_typed() {
    assert_eq!(follow("mine", "ana/7", "bob/7"), None);
}

#[test]
fn an_empty_field_takes_a_prefill_that_came_late() {
    assert_eq!(follow("", "", "bob/7").as_deref(), Some("bob/7"));
}

#[test]
fn a_field_already_showing_the_prefill_is_left_alone() {
    assert_eq!(follow("bob/7", "bob/7", "bob/7"), None);
    assert_eq!(follow("bob/7", "ana/7", "bob/7"), None);
}
