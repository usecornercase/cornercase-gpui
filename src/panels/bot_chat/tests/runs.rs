use std::collections::{HashMap, HashSet};

use serde_json::{Value, json};

use super::super::entries::{
    Block, Entry, EntryKey, Give, arrived, asked, entries, folded, held, keys, retry, spliced, turn,
};
use super::super::runs::NO_TITLE;
use super::super::words::{EXAMPLES, empty_lines, examples_shown, row_detail};
use super::super::{NAME_ROOM, routines_fit};
use crate::model::bots::{Bot, Kind, Line, Runs, history_of};
use crate::panels::queue;
use crate::test_util::fixture;

fn history() -> Vec<Line> {
    history_of(fixture!("bots/run-history.json")).expect("the fixture")
}

fn line(id: u64, kind: &str, text: &str, routine: Option<u64>) -> Line {
    let at = 1000 + i64::try_from(id).expect("small");
    Line { id, at, kind: Kind::from(kind), text: text.to_string(), held: false, ask: None, routine }
}

fn held_line(id: u64, text: &str) -> Line {
    Line { held: true, ..line(id, "you", text, None) }
}

fn blocks(entries: &[Entry]) -> Vec<&Block> {
    entries
        .iter()
        .filter_map(|entry| match entry {
            Entry::Run(block) => Some(block),
            _ => None,
        })
        .collect()
}

fn runs_of(state: &str, routine_kept: bool) -> Runs {
    let mut queue = vec![json!({"id": 7, "label": "Broken check", "state": state, "routine": 3,
                                "place": {"bot": "prs", "project": ""}})];
    if routine_kept {
        queue.push(json!({"id": 3, "label": "Broken check", "state": "pending", "repeat": "30m",
                          "place": {"bot": "prs", "project": ""}}));
    }
    Runs::of(&queue::entries(Some(&json!({ "queue": Value::Array(queue) }))))
}

#[test]
fn a_runs_lines_make_one_block_where_its_header_is_with_a_chat_turn_between() {
    let entries = entries(&history());
    let at = entries.iter().position(|entry| entry.run() == Some(5)).expect("the block");
    assert!(matches!(&entries[at - 1], Entry::Answer { id: 10, .. }));
    assert!(matches!(&entries[at + 1], Entry::You { id: 14, .. }));
    let Entry::Run(block) = &entries[at] else { panic!("a block") };
    assert_eq!(
        (block.head, block.tail, block.title.as_str()),
        (11, 19, "↻ Tell me which of my PRs are blocked… · 14:35")
    );
    assert_eq!(block.steps.len(), 3);
    assert_eq!(block.answer.as_ref().map(|answer| answer.0), Some(19));
    assert_eq!((block.error.as_deref(), block.end.as_deref()), (None, None));
    let failed = blocks(&entries).into_iter().find(|block| block.run == 7).expect("the failed run");
    assert_eq!(failed.error.as_deref(), Some("The fake was told to fail."));
}

#[test]
fn two_runs_of_one_routine_make_two_blocks() {
    let lines = [line(1, "notice", "↻ PRs · 09:00", Some(5)), line(2, "notice", "↻ PRs · 09:30", Some(8))];
    let runs: Vec<u64> = blocks(&entries(&lines)).iter().map(|block| block.run).collect();
    assert_eq!(runs, [5, 8]);
}

#[test]
fn a_block_without_its_header_reads_a_routine() {
    let lines = [line(1, "step", "Read x", Some(5)), line(2, "bot", "done", Some(5))];
    assert_eq!(blocks(&entries(&lines))[0].title, NO_TITLE);
    let unnamed = [line(1, "notice", "stopped", Some(5))];
    let read = entries(&unnamed);
    let block = blocks(&read)[0];
    assert_eq!((block.title.as_str(), block.end.as_deref()), (NO_TITLE, Some("stopped")));
}

#[test]
fn a_chat_step_after_a_block_starts_a_run_of_steps_of_its_own() {
    let lines = [line(1, "step", "a", None), line(2, "notice", "↻ PRs · 09:00", Some(5)), line(3, "step", "b", None)];
    let kinds: Vec<bool> = entries(&lines).iter().map(|entry| matches!(entry, Entry::Steps { .. })).collect();
    assert_eq!(kinds, [true, false, true]);
}

#[test]
fn a_runs_answer_does_not_end_the_chats_turn() {
    let lines = [
        line(1, "you", "go", None),
        line(2, "notice", "↻ PRs · 09:00", Some(5)),
        held_line(3, "more"),
        line(4, "bot", "the run's answer", Some(5)),
        line(5, "step", "s", None),
    ];
    assert!(held(&lines).contains(&3));
    assert_eq!(turn(&lines, true).map(|turn| (turn.since, turn.steps)), Some((1001, 1)));
}

#[test]
fn a_runs_stop_between_a_stop_and_its_dropped_notice_keeps_what_was_dropped() {
    let lines = [
        line(1, "you", "go", None),
        held_line(2, "a"),
        held_line(3, "b"),
        line(4, "notice", "stopped", None),
        line(5, "notice", "stopped", Some(9)),
        line(6, "notice", "2 held messages dropped", None),
    ];
    assert_eq!(asked(&lines).dropped, HashSet::from([2, 3]));
}

#[test]
fn retry_shows_for_a_chat_error_followed_by_a_runs_lines_and_never_for_a_runs_error() {
    let chat = [line(1, "you", "ask", None), line(2, "error", "failed", None), line(3, "error", "run", Some(5))];
    assert_eq!(retry(&chat, false, true).map(|retry| retry.error), Some(2));
    let run = [line(1, "you", "ask", None), line(2, "bot", "ok", None), line(3, "error", "run", Some(5))];
    assert_eq!(retry(&run, false, true), None);
}

#[test]
fn a_blocks_steps_fold_to_none_and_a_chats_keep_three() {
    let entries = entries(&history());
    let block = blocks(&entries).into_iter().find(|block| block.run == 5).expect("the block");
    assert_eq!(folded(&block.steps, false, 0), (3, &[][..]));
    assert_eq!(folded(&block.steps, true, 0).0, 0);
}

fn keys_with(lines: &[Line], runs: &Runs) -> Vec<EntryKey> {
    keys(&entries(lines), &HashSet::new(), None, (&HashMap::new(), Give::No), runs)
}

fn failed_run() -> Vec<Line> {
    vec![line(1, "you", "hi", None), line(2, "notice", "↻ Broken check · 09:00", Some(7)), line(3, "bot", "ok", None)]
}

#[test]
fn a_block_that_grows_splices_only_its_own_row() {
    let runs = Runs::default();
    let old = keys_with(&failed_run(), &runs);
    let mut grown = failed_run();
    grown.push(line(4, "error", "failed", Some(7)));
    assert_eq!(spliced(&old, &keys_with(&grown, &runs)), vec![(1..2, 1)]);
}

#[test]
fn a_blocks_key_moves_when_it_starts_or_stops_answering_and_when_run_again_comes_or_goes() {
    let mut lines = failed_run();
    lines.push(line(4, "error", "failed", Some(7)));
    let answering = keys_with(&lines, &runs_of("starting", true));
    let again = keys_with(&lines, &runs_of("failed", true));
    let gone = keys_with(&lines, &runs_of("failed", false));
    assert_eq!(spliced(&answering, &again), vec![(1..2, 1)]);
    assert_eq!(spliced(&again, &gone), vec![(1..2, 1)]);
    assert_eq!(spliced(&gone, &gone), vec![]);
}

fn bot(state: &str, next: Option<i64>) -> Bot {
    Bot { agent: "claude".into(), model: Some("opus".into()), state: state.into(), next, ..Bot::default() }
}

#[test]
fn an_idle_bots_row_names_its_next_run_and_a_busy_one_its_agent() {
    let now = 1_794_744_000;
    let next = row_detail(&bot("idle", Some(now + 600)), false, now);
    assert!(next.starts_with("next ") && next.contains(':'), "{next}");
    assert_eq!(row_detail(&bot("thinking", Some(now + 600)), true, now), "claude · opus");
    assert_eq!(row_detail(&bot("idle", Some(now + 600)), true, now), "claude · opus");
    assert_eq!(row_detail(&bot("idle", None), false, now), "claude · opus");
}

#[test]
fn the_empty_chat_names_a_schedule_and_offers_four_examples_only_with_routines() {
    assert!(empty_lines(true)[1].contains("on a schedule it sets itself"));
    assert!(empty_lines(false)[1].starts_with("It runs only when you write to it"));
    assert_eq!((examples_shown(true), examples_shown(false)), (4, 3));
    assert!(EXAMPLES[3].starts_with("Every weekday at 9"));
}

const BUTTON: f32 = 75.;

#[test]
fn routines_stays_while_the_name_keeps_its_room_and_goes_below_it() {
    assert!(routines_fit(NAME_ROOM, true, BUTTON));
    assert!(routines_fit(400., true, BUTTON));
    assert!(!routines_fit(NAME_ROOM - 1., true, BUTTON));
}

#[test]
fn a_hidden_routines_comes_back_only_once_the_room_holds_it_its_gap_and_the_slack() {
    let back = NAME_ROOM + BUTTON + 10. + 8.;
    assert!(routines_fit(back, false, BUTTON));
    assert!(!routines_fit(back - 1., false, BUTTON));
}

#[test]
fn no_width_in_the_slacks_band_flips_routines() {
    for step in 0..=80 {
        let drawn_room = NAME_ROOM - 4. + f32::from(u8::try_from(step).expect("small")) * 0.25;
        let hidden_room = drawn_room + BUTTON + 10.;
        let drawn = routines_fit(drawn_room, true, BUTTON);
        let back = routines_fit(hidden_room, false, BUTTON);
        assert!(!back || drawn, "comes back at {hidden_room} and would go again at {drawn_room}");
        assert!(drawn || !back, "goes at {drawn_room} and would come back at {hidden_room}");
    }
}

#[test]
fn the_least_windows_room_hides_routines_and_keeps_it_hidden() {
    assert!(!routines_fit(26., true, BUTTON));
    assert!(!routines_fit(111., false, BUTTON));
}

#[test]
fn a_line_that_lands_before_a_blocks_newest_is_measured_again() {
    let runs = Runs::default();
    let old = keys_with(&[line(1, "notice", "↻ PRs · 09:00", Some(5)), line(4, "bot", "done", Some(5))], &runs);
    let caught_up =
        [line(1, "notice", "↻ PRs · 09:00", Some(5)), line(2, "step", "a", Some(5)), line(4, "bot", "done", Some(5))];
    assert_eq!(spliced(&old, &keys_with(&caught_up, &runs)), vec![(0..1, 1)]);
}

#[test]
fn a_line_of_an_unknown_kind_in_a_run_stays_in_its_block_as_plain_text() {
    let lines = [line(1, "notice", "↻ PRs · 09:00", Some(5)), line(2, "future-note", "Approval required", Some(5))];
    let read = entries(&lines);
    assert_eq!(blocks(&read)[0].others, ["Approval required"]);
    assert_eq!(read.len(), 1);
}

#[test]
fn a_line_joining_the_last_block_is_news_and_one_joining_an_earlier_block_is_not() {
    let runs = Runs::default();
    let header = line(1, "notice", "↻ PRs · 09:00", Some(5));
    let old = keys_with(std::slice::from_ref(&header), &runs);
    let answered = keys_with(&[header.clone(), line(2, "bot", "done", Some(5))], &runs);
    assert!(arrived(&old, &answered));
    let chat = line(3, "you", "hi", None);
    let before = keys_with(&[header.clone(), chat.clone()], &runs);
    let after = keys_with(&[header, chat, line(4, "bot", "done", Some(5))], &runs);
    assert!(!arrived(&before, &after));
}
