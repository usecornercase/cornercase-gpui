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

#[test]
fn the_aim_goes_to_the_rows_block_once() {
    let rows = [None, Some(7), None, Some(5)];
    assert_eq!(aim(&rows, Some(&anchor(5, 3)), 0, true), Aim::Go(3));
    assert_eq!(aim(&rows, Some(&anchor(5, 3)), 3, true), Aim::Wait);
    assert_eq!(aim(&rows, None, 0, true), Aim::Wait);
}

#[test]
fn the_aim_waits_while_the_chat_is_read_and_misses_a_read_chat_without_the_block() {
    let rows = [None, Some(7)];
    assert_eq!(aim(&rows, Some(&anchor(5, 3)), 0, false), Aim::Wait);
    assert_eq!(aim(&rows, Some(&anchor(5, 3)), 0, true), Aim::Miss);
}

#[test]
fn the_aim_keeps_its_anchor_while_the_chat_is_read_and_takes_it_once_read() {
    let rows = [None, Some(5)];
    let reading = aim(&rows, Some(&anchor(5, 3)), 0, false);
    assert_eq!(reading, Aim::Go(1));
    assert!(!consumed(reading, false));
    let read = aim(&rows, Some(&anchor(5, 3)), 0, true);
    assert!(consumed(read, true));
    assert!(consumed(Aim::Miss, true));
    assert!(!consumed(Aim::Wait, true));
}
