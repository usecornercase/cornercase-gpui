use super::*;
use crate::test_util::{vps_read as read, vps_target};

fn up() -> Reach {
    Reach::Up(vps_target())
}

const NOW: i64 = 1_000;
const TRIES_AT: &str = "1970-01-01T00:16:44.139Z";

fn down(retry_at: Option<&str>) -> MachineRead {
    MachineRead { retry_at: retry_at.map(Into::into), ..read(Reach::Down("vps unreachable".into())) }
}

fn says(read: &MachineRead, retry: &Retry, now: i64) -> Option<String> {
    note_at(read, retry, now).as_ref().map(Note::text)
}

fn counts(read: &MachineRead, retry: &Retry, now: i64) -> bool {
    note_at(read, retry, now).as_ref().is_some_and(Note::counts)
}

fn waiting() -> Retry {
    let mut retry = Retry::default();
    retry.failed("ssh: connection refused".into(), Instant::now());
    retry
}

#[test]
fn a_machine_that_answers_needs_no_note() {
    assert_eq!(note_at(&read(up()), &Retry::default(), NOW), None);
}

#[test]
fn a_machine_down_with_no_time_says_it_reconnects() {
    assert_eq!(says(&down(None), &Retry::default(), NOW).as_deref(), Some("vps unreachable · reconnecting…"));
}

#[test]
fn a_machine_down_counts_down_to_its_next_try() {
    let said = says(&down(Some(TRIES_AT)), &Retry::default(), NOW);
    assert_eq!(said.as_deref(), Some("vps unreachable · reconnecting in 4 s"));
}

#[test]
fn a_machine_down_whose_time_has_come_says_its_try_is_under_way() {
    for now in [NOW + 4, NOW + 60] {
        let said = says(&down(Some(TRIES_AT)), &Retry::default(), now);
        assert_eq!(said.as_deref(), Some("vps unreachable · reconnecting…"));
    }
}

#[test]
fn a_machine_that_stopped_answering_while_linked_says_it_reconnects() {
    let said = says(&read(up()), &waiting(), NOW);
    assert_eq!(said.as_deref(), Some("vps did not answer · reconnecting…"));
}

#[test]
fn a_machine_that_cannot_be_read_from_this_window_does_not_promise_to_come_back() {
    let refused =
        MachineRead { retry_at: Some(TRIES_AT.into()), ..read(Reach::Refused("vps runs an older cornercase".into())) };
    let note = note_at(&refused, &Retry::default(), NOW);
    assert_eq!(note, Some(Note::Refused("vps runs an older cornercase".into())));
}

#[test]
fn the_strip_asks_for_a_tick_only_while_it_shows_a_time() {
    let none = Retry::default();
    assert!(counts(&down(Some(TRIES_AT)), &none, NOW));
    assert!(!counts(&down(Some(TRIES_AT)), &none, NOW + 4));
    assert!(!counts(&down(None), &none, NOW));
    assert!(!counts(&read(Reach::Refused("old".into())), &none, NOW));
    assert!(!counts(&read(up()), &waiting(), NOW));
    assert!(!counts(&read(up()), &none, NOW));
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

fn older_note() -> OlderNote {
    OlderNote {
        line: "vps runs cornercase 0.13.39 and this Mac 0.13.57: update it".into(),
        tip: "Update it from this Mac with cornercase --remote vps.invalid update".into(),
        ..OlderNote::default()
    }
}

#[test]
fn a_place_waiting_for_the_machines_list_says_so_and_asks_nothing() {
    let waiting = read(Reach::Waiting("waiting for the machines list".into()));
    assert_eq!(says(&waiting, &Retry::default(), NOW).as_deref(), Some("waiting for the machines list…"));
    assert!(!counts(&waiting, &Retry::default(), NOW));
    assert_eq!(may_ask(&waiting, &Retry::default()), Err("waiting for the machines list".into()));
}

#[test]
fn an_older_machine_says_both_releases_and_asks_nothing() {
    let older = read(Reach::Older(older_note()));
    assert_eq!(says(&older, &Retry::default(), NOW), Some(older_note().line));
    assert!(!counts(&older, &Retry::default(), NOW));
    assert_eq!(may_ask(&older, &Retry::default()), Err(older_note().line));
}

#[test]
fn an_older_note_wins_over_a_failed_ask() {
    let said = says(&read(Reach::Older(older_note())), &waiting(), NOW);
    assert_eq!(said, Some(older_note().line));
}

#[test]
fn the_older_note_tips_how_to_update() {
    let note = note_at(&read(Reach::Older(older_note())), &Retry::default(), NOW);
    assert_eq!(note, Some(Note::Older(older_note())));
}

#[test]
fn an_older_machine_forgets_the_wait_so_the_next_ask_goes_at_once() {
    let mut retry = waiting();
    retry.forget_while_older(&read(up()).reach);
    assert!(retry.waiting());
    retry.forget_while_older(&Reach::Down("d".into()));
    assert!(retry.waiting());
    retry.forget_while_older(&Reach::Older(older_note()));
    assert!(!retry.waiting());
    assert!(retry.due(Instant::now()));
}
