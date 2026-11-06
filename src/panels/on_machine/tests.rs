use super::*;
use crate::test_util::{vps_read as read, vps_target};

fn up() -> Reach {
    Reach::Up(vps_target())
}

#[test]
fn a_machine_that_answers_needs_no_note() {
    assert_eq!(note(&read(up()), &Retry::default()), None);
}

#[test]
fn a_machine_down_says_it_is_unreachable_and_reconnecting() {
    let note = note(&read(Reach::Down("vps unreachable".into())), &Retry::default());
    assert_eq!(note, Some(Note::Reconnecting("vps unreachable · reconnecting…".into())));
}

#[test]
fn a_machine_that_stopped_answering_while_linked_says_it_reconnects() {
    let mut retry = Retry::default();
    retry.failed("ssh: connection refused".into(), Instant::now());
    let note = note(&read(up()), &retry);
    assert_eq!(note, Some(Note::Reconnecting("vps did not answer · reconnecting…".into())));
}

#[test]
fn a_machine_that_cannot_be_read_from_this_window_does_not_promise_to_come_back() {
    let note = note(&read(Reach::Refused("vps runs an older cornercase".into())), &Retry::default());
    assert_eq!(note, Some(Note::Refused("vps runs an older cornercase".into())));
}

#[test]
fn a_machine_that_answers_may_be_asked_at_its_destination() {
    assert_eq!(may_ask(&read(up()), &Retry::default()), Ok(vps_target()));
}

#[test]
fn nothing_new_is_asked_of_a_machine_that_stopped_answering() {
    let mut retry = Retry::default();
    retry.failed("vps restarted".into(), Instant::now());
    assert_eq!(may_ask(&read(up()), &retry), Err("vps did not answer".to_string()));
}

#[test]
fn hearing_from_the_machine_lets_it_be_asked_again() {
    let mut retry = Retry::default();
    retry.failed("vps restarted".into(), Instant::now());
    retry.heard();
    assert_eq!(may_ask(&read(up()), &retry), Ok(vps_target()));
}

#[test]
fn a_machine_down_is_not_asked_and_says_why() {
    let why = may_ask(&read(Reach::Down("vps unreachable".into())), &Retry::default());
    assert_eq!(why, Err("vps unreachable".to_string()));
}

#[test]
fn a_panel_with_nothing_to_show_names_the_machine_it_waits_for() {
    assert_eq!(nothing_read(&read(up())), "Nothing read from vps");
}

#[test]
fn each_failure_waits_longer_as_a_link_coming_back_does() {
    let mut retry = Retry::default();
    let start = Instant::now();
    assert_eq!(retry.failed("no".into(), start), Duration::from_secs(1));
    assert_eq!(retry.failed("no".into(), start + Duration::from_secs(1)), Duration::from_secs(2));
    assert_eq!(retry.failed("no".into(), start + Duration::from_secs(3)), Duration::from_secs(4));
}

#[test]
fn failures_that_come_while_waiting_count_once() {
    let mut retry = Retry::default();
    let start = Instant::now();
    retry.failed("the listing".into(), start);
    assert_eq!(retry.failed("the file".into(), start), Duration::from_secs(1));
    assert_eq!(retry.failed("no".into(), start + Duration::from_secs(1)), Duration::from_secs(2));
}

#[test]
fn nothing_is_asked_until_the_wait_is_over() {
    let mut retry = Retry::default();
    let start = Instant::now();
    assert!(retry.due(start));
    let wait = retry.failed("no".into(), start);
    assert!(!retry.due(start));
    assert!(retry.due(start + wait));
}

#[test]
fn hearing_from_the_machine_starts_the_waits_over() {
    let mut retry = Retry::default();
    let start = Instant::now();
    retry.failed("no".into(), start);
    assert!(retry.heard());
    assert!(!retry.waiting());
    assert!(retry.due(start));
    assert_eq!(retry.failed("no".into(), start), Duration::from_secs(1));
}

#[test]
fn hearing_when_nothing_failed_changes_nothing() {
    assert!(!Retry::default().heard());
}
