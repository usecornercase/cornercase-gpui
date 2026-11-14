use super::*;
use crate::cli::failure;

#[test]
fn a_start_there_refused_for_its_run_says_the_machine_restarted() {
    let stale = failure("new-tab", Some(4), b"error: this request is for another run\n");
    assert_eq!(answer_there("vps", Err(stale)).err().as_deref(), Some("vps restarted: nothing was sent"));
}

#[test]
fn a_start_there_failing_otherwise_reads_as_before() {
    let other = failure("new-tab", Some(1), b"there is no workspace 7");
    assert_eq!(answer_there("vps", Err(other)).err().as_deref(), Some("vps: there is no workspace 7"));
}

#[test]
fn a_start_there_answered_with_nothing_readable_names_the_machine() {
    let said = answer_there("vps", Ok("not json".into())).err().unwrap_or_default();
    assert!(said.starts_with("vps: "), "{said}");
}
