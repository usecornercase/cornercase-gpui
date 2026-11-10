use anyhow::anyhow;

use super::{Answer, Said, answer, printed, said, said_of};
use crate::test_util::fixture;

fn reply(code: i32, stdout: &str, stderr: &str) -> Answer {
    Answer { code: Some(code), stdout: stdout.to_string(), stderr: stderr.to_string() }
}

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
fn a_read_refused_for_another_run_reads_as_a_stale_boot() {
    let err = answer("context", Some(4), b"", b"error: this request is for the cornercase server started as 1-2\n")
        .expect_err("a stale boot");
    assert!(crate::cli::stale_boot(&err));
}

#[test]
fn any_other_failed_read_is_not_a_stale_boot() {
    let err = answer("context", Some(1), b"", b"error: no such pane\n").expect_err("a failure");
    assert!(!crate::cli::stale_boot(&err));
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

#[test]
fn an_update_that_exits_0_is_done() {
    let text = fixture!("remote/update_done.txt");
    assert_eq!(said(&reply(0, text, "")), Said::Done(text.trim().to_string()));
}

#[test]
fn an_update_cornercase_cannot_run_says_what_to_run() {
    let Said::Failed(text) = said(&reply(1, fixture!("remote/update_by_hand.txt"), "")) else {
        panic!("expected a failure")
    };
    assert!(text.contains("brew update && brew upgrade cornercase"), "{text}");
}

#[test]
fn a_failed_update_gives_its_reason() {
    let stderr = "Error: could not reach GitHub: io: Connection refused (os error 61)";
    assert_eq!(
        said(&reply(1, "", stderr)),
        Said::Failed("could not reach GitHub: io: Connection refused (os error 61)".into())
    );
}

#[test]
fn an_update_that_could_not_start_says_so() {
    let said = said_of(Err(anyhow!("could not run `cornercase`")));
    assert_eq!(said, Said::Failed("could not run `cornercase`".into()));
}

#[test]
fn an_update_that_says_nothing_still_says_something() {
    let Said::Failed(text) = said(&reply(1, "", "")) else { panic!("expected a failure") };
    assert_ne!(text, "");
}

#[test]
fn a_failed_restart_keeps_its_reason() {
    let said = said(&reply(1, "Restarting stops every program in the terminals.", "Error: could not stop the server"));
    let Said::Failed(text) = said else { panic!("expected a failure") };
    assert!(text.contains("could not stop the server"), "{text}");
    assert!(text.contains("Restarting stops"), "{text}");
}
