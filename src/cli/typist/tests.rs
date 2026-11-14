use serde_json::{Value, json};

use super::{answer, args, ended, line, told_end};

#[test]
fn a_line_names_the_pane_it_types_into() {
    let sent: Value = serde_json::from_str(&line(14, &json!({ "key": { "code": "enter" } }))).expect("json");
    assert_eq!(sent, json!({ "pane": 14, "key": { "code": "enter" } }));
}

#[test]
fn a_line_is_one_line_whatever_the_paste_holds() {
    let sent = line(3, &json!({ "paste": "two\nlines\r\n" }));
    assert!(!sent.contains('\n'));
    assert_eq!(serde_json::from_str::<Value>(&sent).expect("json")["paste"], "two\nlines\r\n");
}

#[test]
fn an_answer_says_how_many_lines_it_answers() {
    assert_eq!(answer(r#"{"bytes":3,"events":3,"pane":45,"skipped":[]}"#), (3, None));
}

#[test]
fn a_refused_line_gives_its_error() {
    let (events, error) = answer(r#"{"error":"there is no pane 9","events":1,"pane":9}"#);
    assert_eq!((events, error.as_deref()), (1, Some("there is no pane 9")));
}

#[test]
fn an_answer_that_is_not_json_counts_as_one_failed_line() {
    assert_eq!(answer("oops\n"), (1, Some("oops".to_string())));
}

#[test]
fn an_ended_typist_says_the_last_thing_it_wrote() {
    assert_eq!(
        ended("warning\nError: no cornercase server is running\n\n"),
        "cornercase input --follow: Error: no cornercase server is running"
    );
    assert_eq!(ended(""), "cornercase input --follow: it ended");
}

#[test]
fn a_machines_typist_follows_with_its_runs_boot() {
    assert_eq!(args(Some("b1")), ["input", "--follow", "--json", "-", "--boot", "b1"]);
    assert_eq!(args(None), ["input", "--follow", "--json", "-"]);
}

#[test]
fn a_typist_that_ended_on_a_refused_run_says_nothing() {
    assert_eq!(told_end("error: stale", Some(4), false), None);
}

#[test]
fn a_typist_that_ended_otherwise_says_its_last_line() {
    assert_eq!(
        told_end("Error: no cornercase server is running\n", Some(1), false).as_deref(),
        Some("cornercase input --follow: Error: no cornercase server is running")
    );
}

#[test]
fn a_typist_the_window_stopped_says_nothing() {
    assert_eq!(told_end("killed", None, true), None);
}
