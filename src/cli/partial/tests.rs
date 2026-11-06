use super::{answer, printed};

#[test]
fn a_partial_answer_keeps_its_output() {
    let out = answer("usage", Some(3), b"{\"plans\":[]}", b"codex: not logged in").expect("a partial answer");
    assert_eq!(out, "{\"plans\":[]}");
}

#[test]
fn a_whole_answer_keeps_its_output() {
    assert_eq!(answer("issue", Some(0), b"[]\n", b"").expect("an answer"), "[]\n");
}

#[test]
fn an_error_drops_its_output_and_names_the_command() {
    let err = answer("issue", Some(1), b"{}", b"Error: no such project 4\n").expect_err("an error");
    assert_eq!(err.to_string(), "cornercase issue: no such project 4");
}

#[test]
fn an_error_without_text_still_says_it_failed() {
    let err = answer("start", None, b"", b"").expect_err("killed by a signal");
    assert_eq!(err.to_string(), "cornercase start: failed");
}

#[test]
fn a_failure_that_printed_its_outcome_keeps_it() {
    let out = printed("plugin", Some(1), b"{\"error\":\"exit status: 3\"}", b"").expect("an outcome");
    assert_eq!(out, "{\"error\":\"exit status: 3\"}");
}

#[test]
fn a_refusal_that_printed_nothing_is_an_error() {
    let err = printed("plugin", Some(1), b"", b"Error: no window is attached\n").expect_err("a refusal");
    assert_eq!(err.to_string(), "cornercase plugin: no window is attached");
}

#[test]
fn a_success_that_printed_nothing_is_no_error() {
    assert_eq!(printed("plugin", Some(0), b"", b"").expect("an answer"), "");
}
