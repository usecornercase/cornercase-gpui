use serde_json::{Value, json};

use super::{answer, line, lost};
use crate::test_util::fixture;

fn args(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_string()).collect()
}

fn sent(line: &str) -> Value {
    serde_json::from_str(line).expect("json")
}

#[test]
fn a_line_is_the_command_line_after_cornercase() {
    let line = line(&args(&["focus", "--tab", "6", "--view", "21"]), None).expect("a line");
    assert_eq!(sent(&line), json!({ "args": ["focus", "--tab", "6", "--view", "21"] }));
}

#[test]
fn a_line_carries_what_the_command_reads_from_standard_input() {
    let line = line(&args(&["send", "--pane", "5", "-"]), Some(b"cargo test")).expect("a line");
    assert_eq!(sent(&line), json!({ "args": ["send", "--pane", "5", "-"], "stdin": "cargo test" }));
}

#[test]
fn a_line_is_one_line_whatever_its_input_holds() {
    let line = line(&args(&["todo", "add", "-"]), Some(b"two\nlines\r\n")).expect("a line");
    assert!(!line.contains('\n'));
    assert_eq!(sent(&line)["stdin"], "two\nlines\r\n");
}

#[test]
fn bytes_that_are_not_text_cannot_go_on_a_line() {
    assert!(line(&args(&["paste-file", "--pane", "5", "-"]), Some(&[0x89, 0x50, 0xff, 0x00])).is_none());
}

#[test]
fn an_answer_gives_what_the_command_printed_with_json() {
    let out = answer("focus", r#"{"status":0,"json":{"pane":5,"tab":6}}"#).expect("an answer");
    assert_eq!(serde_json::from_str::<Value>(&out).expect("json"), json!({ "pane": 5, "tab": 6 }));
}

#[test]
fn an_answer_gives_its_text_as_printed() {
    assert_eq!(answer("status", r#"{"status":0,"text":"project 4\n"}"#).expect("an answer"), "project 4\n");
    assert_eq!(answer("scroll", r#"{"status":0}"#).expect("an answer"), "");
}

#[test]
fn a_refused_line_gives_its_error() {
    let error = answer("focus", r#"{"status":1,"error":"there is no tab 9"}"#).expect_err("refused");
    assert_eq!(error.to_string(), "cornercase focus: there is no tab 9");
}

#[test]
fn a_failed_line_without_an_error_gives_its_warnings() {
    let error = answer("agents", r#"{"status":3,"stderr":"warning: pane 7 is gone\n"}"#).expect_err("partial");
    assert_eq!(error.to_string(), "cornercase agents: warning: pane 7 is gone");
    let error = answer("focus", r#"{"status":2}"#).expect_err("refused");
    assert_eq!(error.to_string(), "cornercase focus: failed");
}

#[test]
fn an_answer_that_is_not_json_is_an_error() {
    assert_eq!(answer("focus", "oops\n").expect_err("not json").to_string(), "cornercase focus: oops");
}

#[test]
fn a_line_left_without_an_answer_says_the_last_thing_cornercase_wrote() {
    let error = lost("focus", "warning\nError: the connection to cornercase on `vps` dropped\n\n");
    assert_eq!(error.to_string(), "cornercase focus: Error: the connection to cornercase on `vps` dropped");
    assert_eq!(lost("focus", "").to_string(), "cornercase focus: cornercase requests ended");
}

#[test]
fn a_line_refused_for_another_run_reads_as_a_stale_boot() {
    let refused = r#"{"status":4,"error":"this request is for the cornercase server started as 1-2"}"#;
    let error = answer("close", refused).expect_err("a refusal");
    assert!(crate::cli::stale_boot(&error));
    assert_eq!(error.to_string(), "cornercase close: this request is for the cornercase server started as 1-2");
}

#[test]
fn any_other_refused_line_is_not_a_stale_boot() {
    let error = answer("close", r#"{"status":1,"error":"there is no pane 9"}"#).expect_err("a failure");
    assert!(!crate::cli::stale_boot(&error));
}

#[test]
fn a_line_answers_a_move_with_the_layout_the_command_prints() {
    let text = answer("move-pane", fixture!("move_pane/line_unchanged.json")).expect("an answer");
    let printed = serde_json::from_str::<Value>(fixture!("move_pane/unchanged.json")).expect("json");
    assert_eq!(sent(&text), printed);
}

#[test]
fn a_move_refused_on_a_line_keeps_its_reason_and_is_no_stale_boot() {
    let error = answer("move-pane", fixture!("move_pane/line_refused.json")).expect_err("refused");
    assert!(!crate::cli::stale_boot(&error));
    assert!(crate::cli::reason(&error).contains("cannot move onto itself"));
}
