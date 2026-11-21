use serde_json::Value;

use super::*;
use crate::panels::queue::{Entry, entries};
use crate::test_util::fixture;

fn queue() -> Vec<Entry> {
    let list = entries(serde_json::from_str::<Value>(fixture!("queue/bots.json")).ok().as_ref());
    assert!(list.len() > 1, "the fixture");
    list
}

fn with(id: u64, change: impl FnOnce(&mut Entry)) -> Vec<Entry> {
    let mut list = queue();
    if let Some(entry) = list.iter_mut().find(|entry| entry.id == id) {
        change(entry);
    }
    list
}

#[test]
fn the_runs_are_read_from_the_queue() {
    let runs = Runs::of(&queue());
    assert_eq!(runs.label(5), Some("Tell me which of my PRs are blocked…"));
    assert_eq!(runs.label(7), Some("Broken check"));
    assert!(!runs.answers(5) && !runs.answering("prs"));
    let starting = Runs::of(&with(5, |entry| entry.state = "starting".into()));
    assert!(starting.answers(5) && starting.answering("prs") && !starting.answering("ops"));
}

#[test]
fn what_a_bot_scheduled_leaves_its_runs_out() {
    let runs = Runs::of(&queue());
    assert_eq!(runs.scheduled("prs"), ["Review PRs", "Broken check", "Stand-up reminder"]);
    assert_eq!(runs.scheduled("ops"), [] as [String; 0]);
}

#[test]
fn a_projects_entries_are_ignored() {
    let runs = Runs::of(&queue());
    assert_eq!((runs.label(9), runs.again(9)), (None, None));
}

#[test]
fn run_again_starts_a_routine_that_still_exists() {
    assert_eq!(Runs::of(&queue()).again(7), Some(3));
    let gone: Vec<Entry> = queue().into_iter().filter(|entry| entry.id != 3).collect();
    assert_eq!(Runs::of(&gone).again(7), None);
}

#[test]
fn run_again_starts_a_failed_or_missed_one_off_but_not_one_that_answered() {
    for state in ["failed", "missed"] {
        assert_eq!(Runs::of(&with(4, |entry| entry.state = state.into())).again(4), Some(4), "{state}");
    }
    assert_eq!(Runs::of(&with(4, |entry| entry.state = "answered".into())).again(4), None);
    assert_eq!(Runs::of(&queue()).again(99), None);
}

fn anchor(run: u64, serial: u64) -> Anchor {
    Anchor { bot: "prs".into(), run, serial }
}

fn done(serial: u64) -> Aimed {
    Aimed { done: serial, early: serial }
}

#[test]
fn a_read_chat_goes_to_the_rows_block_once() {
    let rows = [None, Some(7), None, Some(5)];
    let (go, aimed) = aim(&rows, Some(&anchor(5, 3)), Aimed::default(), true);
    assert_eq!((go, aimed.done), (Some(3), 3));
    assert_eq!(aim(&rows, Some(&anchor(5, 3)), aimed, true).0, None);
    assert_eq!(aim(&rows, None, Aimed::default(), true), (None, Aimed::default()));
}

#[test]
fn a_read_chat_without_the_block_takes_the_anchor_and_moves_nothing() {
    let (go, aimed) = aim(&[None, Some(7)], Some(&anchor(5, 3)), Aimed::default(), true);
    assert_eq!((go, aimed.done), (None, 3));
}

#[test]
fn while_the_chat_is_read_the_block_is_gone_to_once_and_again_once_it_is_read() {
    let rows = [None, Some(5)];
    let (first, aimed) = aim(&rows, Some(&anchor(5, 3)), Aimed::default(), false);
    assert_eq!((first, aimed.done), (Some(1), 0));
    let (again, aimed) = aim(&rows, Some(&anchor(5, 3)), aimed, false);
    assert_eq!(again, None, "a render while it is read leaves the reader's scroll alone");
    let (settled, aimed) = aim(&[None, None, Some(5)], Some(&anchor(5, 3)), aimed, true);
    assert_eq!((settled, aimed), (Some(2), done(3)));
}

#[test]
fn a_failed_read_settles_the_anchor() {
    let rows = [None, Some(5)];
    let (go, aimed) = aim(&rows, Some(&anchor(5, 3)), Aimed { done: 0, early: 3 }, true);
    assert_eq!((go, aimed), (Some(1), done(3)));
    assert_eq!(aim(&rows, Some(&anchor(5, 3)), aimed, true).0, None, "no render after it scrolls back");
}

#[test]
fn a_chat_still_read_without_the_block_waits() {
    assert_eq!(aim(&[None, Some(7)], Some(&anchor(5, 3)), Aimed::default(), false), (None, Aimed::default()));
}
