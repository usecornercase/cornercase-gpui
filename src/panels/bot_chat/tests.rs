use std::collections::{HashMap, HashSet};

use super::entries::{
    Again, Control, Entry, EntryKey, Give, Retry, STEPS_SHOWN, Turn, arrived, asked, entries, folded, givable, held,
    keys, new_conversation_control, retry, row_splices, send_control, spliced, stop_control, turn,
};
use super::words::{folded_label, time_label, turn_label};
use super::{Scroll, scroll_distance, scroll_of};

const NOON: i64 = 1_794_744_000;

#[test]
fn a_line_of_today_shows_its_time_of_day() {
    let label = time_label(NOON, NOON + 60);
    assert_eq!(label.len(), 5);
    assert_eq!(label.as_bytes()[2], b':');
}

#[test]
fn an_older_line_shows_its_day_and_time() {
    let label = time_label(NOON, NOON + 3 * 86_400);
    assert!(label.len() > 5 && label.ends_with(&time_label(NOON, NOON + 1)), "{label}");
}

#[test]
fn a_folded_run_reads_plus_n_steps() {
    assert_eq!(folded_label(4), "+4 steps");
}

#[test]
fn one_hidden_step_reads_plus_1_step() {
    assert_eq!(folded_label(1), "+1 step");
}

#[test]
fn the_turn_row_reads_thinking_its_time_and_its_steps() {
    assert_eq!(turn_label(Turn { since: 100, steps: 3 }, 142), "Thinking · 42s · 3 steps");
    assert_eq!(turn_label(Turn { since: 100, steps: 1 }, 160), "Thinking · 1m · 1 step");
}

#[test]
fn the_turn_row_leaves_out_a_time_of_now() {
    assert_eq!(turn_label(Turn { since: 100, steps: 0 }, 102), "Thinking");
}

#[test]
fn the_arrows_and_the_pages_scroll_the_chat() {
    assert_eq!(scroll_of("up"), Some(Scroll::Line(-1.)));
    assert_eq!(scroll_of("down"), Some(Scroll::Line(1.)));
    assert_eq!(scroll_of("pageup"), Some(Scroll::Page(-1.)));
    assert_eq!(scroll_of("pagedown"), Some(Scroll::Page(1.)));
    assert!(scroll_distance(Scroll::Page(-1.), 600.) < -scroll_distance(Scroll::Line(1.), 600.));
}

#[test]
fn other_keys_scroll_nothing() {
    for key in ["a", "enter", "escape", "left", "space"] {
        assert_eq!(scroll_of(key), None, "{key}");
    }
}

#[test]
fn an_answer_keeps_its_raw_markdown_for_its_copy() {
    let lines = vec![line(1, "bot", "**bold** and `code`", false)];
    let entries = entries(&lines);
    assert!(matches!(&entries[0], Entry::Answer { text, .. } if text == "**bold** and `code`"));
}

use crate::model::bots::{Kind, Line, Runs};

fn line(id: u64, kind: &str, text: &str, held: bool) -> Line {
    Line {
        id,
        at: 1000 + i64::try_from(id).expect("small"),
        kind: Kind::from(kind),
        text: text.to_string(),
        held,
        ask: None,
        routine: None,
    }
}

fn plain(ids: &[(u64, &str)]) -> Vec<Line> {
    ids.iter().map(|(id, kind)| line(*id, kind, "x", false)).collect()
}

fn steps(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("s{i}")).collect()
}

#[test]
fn three_steps_or_fewer_stay_in_view() {
    for n in 1..=STEPS_SHOWN {
        assert_eq!(folded(&steps(n), false, STEPS_SHOWN), (0, &steps(n)[..]));
    }
}

#[test]
fn more_than_three_steps_fold_to_the_last_three() {
    let all = steps(5);
    assert_eq!(folded(&all, false, STEPS_SHOWN), (2, &all[2..]));
}

#[test]
fn each_run_of_steps_folds_on_its_own() {
    let mut lines = plain(&[(1, "you")]);
    lines.extend((2..=6).map(|id| line(id, "step", "a", false)));
    lines.push(line(7, "bot", "r", false));
    lines.extend((8..=9).map(|id| line(id, "step", "b", false)));
    let runs: Vec<usize> = entries(&lines)
        .iter()
        .filter_map(|e| if let Entry::Steps { texts, .. } = e { Some(texts.len()) } else { None })
        .collect();
    assert_eq!(runs, [5, 2]);
}

#[test]
fn an_opened_run_shows_every_step() {
    let all = steps(6);
    assert_eq!(folded(&all, true, STEPS_SHOWN), (0, &all[..]));
}

#[test]
fn a_message_sent_during_a_turn_is_held_until_that_turn_ends() {
    let lines = vec![line(1, "you", "go", false), line(2, "step", "s", false), line(3, "you", "more", true)];
    assert!(held(&lines).contains(&3));
    let mut ended = lines;
    ended.push(line(4, "bot", "done", false));
    assert!(held(&ended).is_empty());
}

#[test]
fn messages_dropped_by_a_stop_are_no_longer_held() {
    let lines = vec![line(1, "you", "go", false), line(2, "you", "more", true), line(3, "notice", "stopped", false)];
    assert!(held(&lines).is_empty());
    let many = vec![line(1, "you", "m", true), line(2, "notice", "3 held messages dropped", false)];
    assert!(held(&many).is_empty());
}

#[test]
fn a_message_held_while_a_stop_was_on_its_way_is_not_held() {
    let lines = vec![
        line(1, "you", "go", false),
        line(2, "notice", "stopped", false),
        line(3, "you", "late", true),
        line(4, "bot", "answer", false),
    ];
    assert!(held(&lines).is_empty());
}

#[test]
fn a_thinking_bot_has_a_turn_from_its_message_with_the_steps_since() {
    let lines = vec![
        line(1, "bot", "old", false),
        line(2, "you", "go", false),
        line(3, "step", "a", false),
        line(4, "step", "b", false),
    ];
    assert_eq!(turn(&lines, true), Some(Turn { since: 1002, steps: 2 }));
}

#[test]
fn a_turn_started_by_held_messages_starts_at_the_answer_before_it() {
    let lines = vec![
        line(1, "you", "go", false),
        line(2, "you", "more", true),
        line(3, "bot", "a", false),
        line(4, "step", "b", false),
    ];
    assert_eq!(turn(&lines, true), Some(Turn { since: 1003, steps: 1 }));
}

#[test]
fn an_idle_bot_has_no_turn() {
    assert_eq!(turn(&plain(&[(1, "you")]), false), None);
}

fn key_of(entries: &[Entry]) -> Vec<EntryKey> {
    keys(entries, &HashSet::new(), None, (&HashMap::new(), Give::No), &Runs::default())
}

#[test]
fn the_list_splices_only_the_entries_that_changed() {
    let old = key_of(&entries(&plain(&[(1, "you"), (2, "bot"), (3, "you")])));
    let new = key_of(&entries(&plain(&[(1, "you"), (2, "bot"), (3, "you"), (4, "bot")])));
    assert_eq!(spliced(&old, &new), vec![(3..3, 1)]);
    assert_eq!(spliced(&old, &old), Vec::new());
}

fn a_running_run(opened: &[u64]) -> Vec<EntryKey> {
    let lines = plain(&[(1, "you"), (2, "bot"), (3, "you"), (4, "step"), (5, "step"), (6, "step"), (7, "step")]);
    keys(&entries(&lines), &ids(opened), None, (&HashMap::new(), Give::No), &Runs::default())
}

#[test]
fn opening_the_last_run_of_steps_is_not_a_new_message() {
    let (shut, open) = (a_running_run(&[]), a_running_run(&[4]));
    assert_ne!(shut, open);
    assert!(!arrived(&shut, &open));
    assert!(!arrived(&open, &shut));
}

#[test]
fn a_line_after_the_last_one_is_a_new_message() {
    let old = key_of(&entries(&plain(&[(1, "you"), (2, "step")])));
    let joined = key_of(&entries(&plain(&[(1, "you"), (2, "step"), (3, "step")])));
    let answered = key_of(&entries(&plain(&[(1, "you"), (2, "step"), (3, "bot")])));
    assert!(arrived(&old, &joined));
    assert!(arrived(&old, &answered));
}

#[test]
fn a_chat_with_nothing_new_has_no_new_message() {
    let same = key_of(&entries(&plain(&[(1, "you"), (2, "bot")])));
    assert!(!arrived(&same, &same));
    assert!(!arrived(&[], &[]));
}

#[test]
fn a_step_joining_a_run_splices_the_run_alone() {
    let old = key_of(&entries(&plain(&[(1, "you"), (2, "step")])));
    let new = key_of(&entries(&plain(&[(1, "you"), (2, "step"), (3, "step")])));
    assert_eq!(spliced(&old, &new), vec![(1..2, 1)]);
}

#[test]
fn lines_dropped_at_the_front_splice_the_front() {
    let old = key_of(&entries(&plain(&[(1, "you"), (2, "bot"), (3, "you")])));
    let new = key_of(&entries(&plain(&[(2, "bot"), (3, "you"), (4, "bot")])));
    assert_eq!(spliced(&old, &new), vec![(0..1, 0), (2..2, 1)]);
}

#[test]
fn a_held_bubble_that_stops_being_held_is_measured_again() {
    let old = key_of(&entries(&[line(1, "you", "a", false), line(2, "you", "b", true)]));
    let new = key_of(&entries(&[line(1, "you", "a", false), line(2, "you", "b", false), line(3, "bot", "c", false)]));
    assert_eq!(spliced(&old, &new), vec![(1..2, 1), (2..2, 1)]);
}

fn capped(from: u64) -> Vec<Line> {
    (from..from + 1000).map(|id| line(id, if id % 7 == 0 { "bot" } else { "step" }, "t", false)).collect()
}

fn keys_of(lines: &[Line]) -> Vec<EntryKey> {
    key_of(&entries(lines))
}

#[test]
fn a_new_line_in_a_capped_chat_with_its_earlier_note_splices_the_front_and_the_end_only() {
    let (old, new) = (keys_of(&capped(201)), keys_of(&capped(202)));
    let splices = row_splices((true, &old), (true, &new));
    assert!(splices.iter().all(|(range, count)| range.len() <= 3 && *count <= 3), "{splices:?}");
}

#[test]
fn the_earlier_note_appearing_splices_one_row_at_the_top() {
    let old = keys_of(&capped(2)[..999]);
    let new = keys_of(&capped(2));
    let splices = row_splices((false, &old), (true, &new));
    assert_eq!(splices[0], (0..0, 1));
    assert!(splices.iter().all(|(range, _)| range.len() <= 2), "{splices:?}");
}

#[test]
fn a_run_of_steps_that_lost_its_oldest_step_is_measured_alone() {
    let old = keys_of(&plain(&[(1, "step"), (2, "step"), (3, "step"), (4, "bot")]));
    let new = keys_of(&plain(&[(2, "step"), (3, "step"), (4, "bot"), (5, "you")]));
    let splices = spliced(&old, &new);
    assert!(splices.iter().all(|(range, _)| range.len() <= 1), "{splices:?}");
}

#[test]
fn a_step_run_keeps_its_real_last_id_so_a_capped_front_keeps_the_reader() {
    let old = keys_of(&plain(&[(10, "step"), (12, "step"), (13, "bot")]));
    let new = keys_of(&plain(&[(12, "step"), (13, "bot"), (14, "you")]));
    let splices = spliced(&old, &new);
    assert!(splices.iter().all(|(range, _)| range.len() <= 1), "{splices:?}");
    assert!(!splices.contains(&(0..3, 0)));
}

#[test]
fn a_chat_is_fresh_for_another_bot_or_a_new_generation_but_not_a_new_revision() {
    use super::is_fresh;
    use crate::model::bots::BotKey;
    let prs = BotKey::here("prs");
    let stamp = |born, revision| (BotKey::here("prs"), born, revision, None, Vec::new(), Give::No);
    let mark = stamp(1, 3);
    assert!(is_fresh(None, None, &mark));
    assert!(is_fresh(Some(&BotKey::here("ops")), Some(&mark), &mark));
    assert!(is_fresh(Some(&BotKey::on("vps", "prs")), Some(&mark), &mark));
    assert!(is_fresh(Some(&prs), Some(&mark), &stamp(5, 3)));
    assert!(!is_fresh(Some(&prs), Some(&mark), &stamp(1, 4)));
    assert!(!is_fresh(Some(&prs), Some(&mark), &(prs.clone(), 1, 3, None, vec![4], Give::Button)));
}

#[test]
fn retry_is_offered_on_an_error_that_is_the_last_line_while_no_turn_runs() {
    let lines = [line(1, "you", "ask", false), line(2, "error", "failed", false)];
    assert_eq!(retry(&lines, false, true), Some(Retry { error: 2, again: Again::Say("ask".into()) }));
}

#[test]
fn retry_sends_the_last_message_before_the_error() {
    let lines = [
        line(1, "you", "old", false),
        line(2, "bot", "a", false),
        line(3, "you", "new", false),
        line(4, "error", "x", false),
    ];
    assert_eq!(retried(&lines), Some("new".to_string()));
}

#[test]
fn no_retry_while_a_turn_runs() {
    let lines = [line(1, "you", "ask", false), line(2, "error", "failed", false)];
    assert_eq!(retry(&lines, true, true), None);
}

#[test]
fn no_retry_when_the_error_is_not_the_last_line() {
    let lines = [line(1, "you", "ask", false), line(2, "error", "failed", false), line(3, "notice", "stopped", false)];
    assert_eq!(retry(&lines, false, true), None);
}

#[test]
fn no_retry_without_a_message_kept_before_the_error() {
    assert_eq!(retry(&[line(1, "error", "failed", false)], false, true), None);
}

#[test]
fn an_error_that_gains_or_loses_retry_is_measured_again() {
    let lines = [line(1, "you", "ask", false), line(2, "error", "failed", false)];
    let entries = entries(&lines);
    let without = keys(&entries, &HashSet::new(), None, (&HashMap::new(), Give::No), &Runs::default());
    let with = keys(&entries, &HashSet::new(), Some(2), (&HashMap::new(), Give::No), &Runs::default());
    assert_eq!(spliced(&without, &with), vec![(1..2, 1)]);
    assert_eq!(spliced(&with, &without), vec![(1..2, 1)]);
}

#[test]
fn new_conversation_rests_while_a_turn_runs() {
    assert_eq!(new_conversation_control(true, false), Control::Resting);
    assert_eq!(new_conversation_control(false, false), Control::Ready);
}

#[test]
fn new_conversation_shows_that_it_runs() {
    assert_eq!(new_conversation_control(false, true), Control::Running);
    assert_eq!(new_conversation_control(true, true), Control::Running);
}

#[test]
fn send_rests_while_the_field_holds_only_blanks() {
    assert_eq!(send_control(true, false), Control::Resting);
    assert_eq!(send_control(false, false), Control::Ready);
}

#[test]
fn send_shows_that_a_message_is_on_its_way() {
    assert_eq!(send_control(false, true), Control::Running);
    assert_eq!(send_control(true, true), Control::Running);
}

#[test]
fn stop_reads_stopping_while_it_runs() {
    assert_eq!(stop_control(true), Control::Running);
    assert_eq!(stop_control(false), Control::Ready);
}

#[test]
fn the_placeholder_names_the_bot() {
    assert_eq!(super::words::placeholder("PRs"), "Message PRs…");
}

#[test]
fn a_resting_new_conversation_says_why() {
    assert_eq!(super::words::answering("PRs"), "PRs is answering: stop it first");
}

#[test]
fn a_chat_that_follows_its_end_is_anchored_there_each_time_it_is_drawn() {
    use super::re_anchor;
    assert_eq!(re_anchor(true, 12), Some(12));
}

#[test]
fn a_chat_the_reader_scrolled_up_is_left_where_it_is() {
    use super::re_anchor;
    assert_eq!(re_anchor(false, 12), None);
}

#[test]
fn retry_sends_the_last_message_not_the_steps_between_it_and_the_error() {
    let lines = [
        line(1, "you", "ask", false),
        line(2, "step", "cornercase status --json", false),
        line(3, "error", "x", false),
    ];
    assert_eq!(retried(&lines), Some("ask".to_string()));
}

#[test]
fn the_headers_menu_button_names_whose_menu_it_opens() {
    assert_eq!(super::words::menu_tip("PRs"), "More for PRs");
}

fn retried(lines: &[Line]) -> Option<String> {
    match retry(lines, false, true)?.again {
        Again::Say(text) => Some(text),
        Again::Setup => None,
    }
}

fn ids(list: &[u64]) -> HashSet<u64> {
    list.iter().copied().collect()
}

#[test]
fn retry_after_two_held_messages_sends_both_joined_by_a_blank_line() {
    let lines = [
        line(1, "you", "first", false),
        line(2, "you", "a", true),
        line(3, "you", "b", true),
        line(4, "bot", "done", false),
        line(5, "error", "failed", false),
    ];
    assert_eq!(retried(&lines), Some("a\n\nb".to_string()));
}

#[test]
fn retry_after_a_stop_sends_what_ran_after_it_not_what_was_dropped() {
    let lines = [
        line(1, "you", "go", false),
        line(2, "you", "a", true),
        line(3, "you", "b", true),
        line(4, "you", "c", true),
        line(5, "notice", "stopped", false),
        line(6, "notice", "2 held messages dropped", false),
        line(7, "error", "failed", false),
    ];
    assert_eq!(retried(&lines), Some("c".to_string()));
    assert_eq!(asked(&lines).dropped, ids(&[2, 3]));
}

#[test]
fn a_message_held_while_a_stop_was_on_its_way_is_not_dropped() {
    let lines = [
        line(1, "you", "go", false),
        line(2, "you", "late", true),
        line(3, "notice", "stopped", false),
        line(4, "error", "failed", false),
    ];
    assert_eq!(retried(&lines), Some("late".to_string()));
    assert!(asked(&lines).dropped.is_empty());
}

#[test]
fn messages_dropped_by_a_stop_are_the_first_held() {
    let one = [
        line(1, "you", "go", false),
        line(2, "you", "a", true),
        line(3, "you", "b", true),
        line(4, "notice", "stopped", false),
        line(5, "notice", "1 held message dropped", false),
    ];
    assert_eq!(asked(&one).dropped, ids(&[2]));
    let more = [
        line(1, "you", "go", false),
        line(2, "you", "a", true),
        line(3, "notice", "stopped", false),
        line(4, "notice", "3 held messages dropped", false),
    ];
    assert_eq!(asked(&more).dropped, ids(&[2]));
}

#[test]
fn a_restart_drops_what_was_held() {
    let lines = [
        line(1, "you", "go", false),
        line(2, "you", "a", true),
        line(3, "you", "b", true),
        line(4, "notice", "interrupted by a restart", false),
        line(5, "notice", "2 held messages dropped", false),
    ];
    assert_eq!(asked(&lines).dropped, ids(&[2, 3]));
    assert_eq!(asked(&lines).ended, vec![1]);
}

#[test]
fn no_retry_when_the_failed_turn_took_no_message() {
    let lines = [line(1, "you", "old", false), line(2, "bot", "done", false), line(3, "error", "failed", false)];
    assert_eq!(retried(&lines), None);
}

#[test]
fn a_dropped_bubble_is_marked_and_measured_again() {
    let before = vec![
        line(1, "you", "go", false),
        line(2, "you", "a", true),
        line(3, "you", "b", true),
        line(4, "notice", "stopped", false),
    ];
    let mut after = before.clone();
    after.push(line(5, "notice", "1 held message dropped", false));
    let dropped: Vec<u64> = entries(&after)
        .iter()
        .filter_map(|entry| match entry {
            Entry::You { id, dropped: true, .. } => Some(*id),
            _ => None,
        })
        .collect();
    assert_eq!(dropped, [2]);
    assert_eq!(spliced(&keys_of(&before), &keys_of(&after)), vec![(1..2, 1), (4..4, 1)]);
}

#[test]
fn the_example_that_gets_the_keys_is_revealed_once() {
    assert_eq!(super::example_to_reveal(Some(0), None), Some(0));
    assert_eq!(super::example_to_reveal(Some(0), Some(0)), None);
}

#[test]
fn the_keys_moving_to_another_example_reveal_it() {
    assert_eq!(super::example_to_reveal(Some(1), Some(0)), Some(1));
    assert_eq!(super::example_to_reveal(Some(0), Some(2)), Some(0));
}

#[test]
fn the_keys_leaving_the_examples_reveal_nothing() {
    assert_eq!(super::example_to_reveal(None, Some(1)), None);
    assert_eq!(super::example_to_reveal(None, None), None);
}

#[test]
fn a_setup_turn_is_timed_from_its_notice_with_the_steps_since() {
    let lines = [line(1, "notice", "setting up", false), line(2, "step", "a", false), line(3, "step", "b", false)];
    assert_eq!(turn(&lines, true), Some(Turn { since: 1001, steps: 2 }));
}

#[test]
fn a_message_held_during_a_setup_does_not_move_its_start() {
    let lines = [
        line(1, "notice", "setting up", false),
        line(2, "step", "a", false),
        line(3, "you", "wait", true),
        line(4, "step", "b", false),
    ];
    assert_eq!(turn(&lines, true), Some(Turn { since: 1001, steps: 2 }));
    assert!(held(&lines).contains(&3));
}

#[test]
fn a_setup_run_again_is_timed_from_its_own_notice() {
    let lines = [
        line(1, "notice", "setting up", false),
        line(2, "error", "failed", false),
        line(3, "notice", "setting up", false),
        line(4, "step", "a", false),
    ];
    assert_eq!(turn(&lines, true), Some(Turn { since: 1003, steps: 1 }));
}

#[test]
fn the_turn_after_a_setup_that_held_a_message_starts_at_its_answer() {
    let lines = [
        line(1, "notice", "setting up", false),
        line(2, "you", "wait", true),
        line(3, "bot", "done", false),
        line(4, "step", "a", false),
    ];
    assert_eq!(turn(&lines, true), Some(Turn { since: 1003, steps: 1 }));
}

#[test]
fn another_notice_opens_no_turn() {
    assert_eq!(turn(&[line(1, "notice", "new conversation", false)], true), None);
}

fn failed_setup() -> [Line; 3] {
    [line(1, "notice", "setting up", false), line(2, "step", "a", false), line(3, "error", "failed", false)]
}

#[test]
fn retry_after_a_failed_setup_runs_the_setup_again() {
    assert_eq!(retry(&failed_setup(), false, true), Some(Retry { error: 3, again: Again::Setup }));
}

#[test]
fn no_retry_after_a_failed_setup_without_the_feature() {
    assert_eq!(retry(&failed_setup(), false, false), None);
}

#[test]
fn retry_after_a_setup_never_sends_its_notice_as_a_message() {
    let again = retry(&failed_setup(), false, true).map(|retry| retry.again);
    assert!(!matches!(again, Some(Again::Say(_))), "{again:?}");
}

#[test]
fn retry_after_messages_held_during_a_setup_sends_those_messages() {
    let lines = [
        line(1, "notice", "setting up", false),
        line(2, "you", "a", true),
        line(3, "bot", "done", false),
        line(4, "error", "failed", false),
    ];
    for setup in [true, false] {
        assert_eq!(retry(&lines, false, setup).map(|retry| retry.again), Some(Again::Say("a".into())), "{setup}");
    }
}

#[test]
fn no_retry_while_a_setup_runs() {
    let lines = [line(1, "notice", "setting up", false), line(2, "error", "failed", false)];
    assert_eq!(retry(&lines, true, true), None);
}

#[test]
fn a_stop_during_a_setup_drops_what_was_held() {
    let lines = [
        line(1, "notice", "setting up", false),
        line(2, "you", "wait", true),
        line(3, "notice", "stopped", false),
        line(4, "notice", "1 held message dropped", false),
    ];
    let asked = asked(&lines);
    assert_eq!(asked.dropped, ids(&[2]));
    assert_eq!(asked.ended, vec![1]);
}

fn notice(id: u64, text: &str, ask: Option<u64>) -> Line {
    Line { ask, ..line(id, "notice", text, false) }
}

fn asking_lines() -> Vec<Line> {
    vec![
        line(1, "you", "log in", false),
        notice(2, "asks for a secret: campus-user", Some(4)),
        line(3, "bot", "waiting for it", false),
        notice(4, "secret saved: campus-user", Some(4)),
    ]
}

#[test]
fn the_first_line_of_a_waiting_ask_gets_give() {
    assert_eq!(givable(&asking_lines(), &[4]), HashMap::from([(2, 4)]));
}

#[test]
fn the_saved_notice_gets_no_button() {
    assert_eq!(givable(&asking_lines(), &[]), HashMap::new());
    assert_eq!(givable(&asking_lines(), &[9]), HashMap::new());
}

#[test]
fn a_notice_entry_keeps_its_ask() {
    let shown = entries(&asking_lines());
    assert_eq!(shown[1], Entry::Notice { id: 2, text: "asks for a secret: campus-user".into(), ask: Some(4) });
}

#[test]
fn a_row_is_measured_again_when_its_ask_ends() {
    let shown = entries(&asking_lines());
    let waiting = givable(&asking_lines(), &[4]);
    let with = keys(&shown, &HashSet::new(), None, (&waiting, Give::Button), &Runs::default());
    let ended = keys(&shown, &HashSet::new(), None, (&HashMap::new(), Give::Button), &Runs::default());
    assert_ne!(with[1], ended[1]);
    assert_eq!(with[0], ended[0]);
    assert_eq!(with[3], ended[3]);
}

#[test]
fn below_the_gate_the_line_says_what_it_needs() {
    let shown = entries(&asking_lines());
    let waiting = givable(&asking_lines(), &[4]);
    let needs = keys(&shown, &HashSet::new(), None, (&waiting, Give::Needs), &Runs::default());
    let button = keys(&shown, &HashSet::new(), None, (&waiting, Give::Button), &Runs::default());
    assert_ne!(needs[1], button[1]);
    let words = super::give_key(&crate::model::bots::Give::Here("Giving it needs cornercase 0.13.101.".into()));
    assert_eq!(words, Give::Needs);
}

mod runs;
