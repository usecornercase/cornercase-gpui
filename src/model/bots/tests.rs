use serde_json::{Value, json};

use super::entries::{Entry, EntryKey, STEPS_SHOWN, entries, folded, held, keys, spliced, turn};
use super::places::{Moved, Showing};
use super::*;
use crate::cli::{Lacks, Version};
use crate::test_util::fixture;

fn app(text: &str) -> Value {
    serde_json::from_str(text).expect("fixture")
}

fn line(id: u64, kind: &str, text: &str, held: bool) -> Line {
    Line { id, at: 1000 + i64::try_from(id).expect("small"), kind: Kind::from(kind), text: text.to_string(), held }
}

fn plain(ids: &[(u64, &str)]) -> Vec<Line> {
    ids.iter().map(|(id, kind)| line(*id, kind, "x", false)).collect()
}

fn read_chat(lines: Vec<Line>) -> Chat {
    let mut chat = Chat::new("prs", 1);
    chat.take(lines);
    chat
}

fn two() -> Bots {
    let mut bots = Bots::new();
    bots.take_app(&app(fixture!("bots/app-two.json")));
    bots
}

#[test]
fn the_bots_are_read_from_the_app_line() {
    let list = in_app(&app(fixture!("bots/app-two.json")));
    assert_eq!(list.iter().map(|b| b.id.as_str()).collect::<Vec<_>>(), ["prs", "ops"]);
    assert_eq!((list[0].model.clone(), list[0].unread, list[0].thinking()), (None, 2, false));
    assert_eq!(
        (list[1].model.as_deref(), list[1].effort.as_deref(), list[1].thinking()),
        (Some("opus"), Some("high"), true)
    );
}

#[test]
fn an_app_line_without_bots_lists_none() {
    assert_eq!(in_app(&app(fixture!("bots/app-none.json"))), Vec::new());
}

#[test]
fn an_unknown_field_of_a_bot_is_ignored() {
    let mut value = app(fixture!("bots/app-two.json"));
    value["bots"][0]["colour"] = json!("blue");
    assert_eq!(in_app(&value).len(), 2);
}

#[test]
fn a_bot_that_does_not_read_is_left_out_and_the_others_stay() {
    let mut value = app(fixture!("bots/app-two.json"));
    value["bots"][0]["unread"] = json!("many");
    let list = in_app(&value);
    assert_eq!(list.iter().map(|b| b.id.as_str()).collect::<Vec<_>>(), ["ops"]);
}

#[test]
fn a_bots_agent_and_model_are_said_in_one_line() {
    let list = in_app(&app(fixture!("bots/app-two.json")));
    assert_eq!((list[0].detail(), list[1].detail()), ("claude".to_string(), "claude · opus".to_string()));
    assert_eq!(list[1].facts(), "claude · opus · high");
}

#[test]
fn a_history_is_read_with_every_kind_of_line() {
    let lines = history_of(fixture!("bots/history.json")).expect("history");
    assert_eq!(lines.len(), 17);
    assert!(lines.iter().any(|l| l.kind == Kind::You && l.held));
    for kind in [Kind::Step, Kind::Bot, Kind::Error, Kind::Notice] {
        assert!(lines.iter().any(|l| l.kind == kind), "{kind:?}");
    }
}

#[test]
fn a_line_of_an_unknown_kind_reads_as_plain_text() {
    let read = line_of(&json!({"id": 3, "at": 5, "kind": "routine", "text": "ran"})).expect("line");
    assert_eq!(read.kind, Kind::Other("routine".into()));
    assert_eq!(read.text, "ran");
}

#[test]
fn a_line_without_an_id_is_left_out() {
    assert!(line_of(&json!({"kind": "bot", "text": "x"})).is_none());
}

fn events() -> Vec<BotEvent> {
    fixture!("bots/events.jsonl").lines().map(|l| serde_json::from_str(l).expect("event")).collect()
}

#[test]
fn a_line_event_is_heard_as_a_line() {
    assert!(matches!(events()[0].heard(), Heard::Line(bot, l) if bot == "ops" && l.kind == Kind::Step));
}

#[test]
fn a_state_event_is_heard_as_thinking_or_not() {
    assert_eq!(events()[1].heard(), Heard::State("ops".into(), true));
    let idle = BotEvent { bot: "ops".into(), state: Some("idle".into()), ..BotEvent::default() };
    assert_eq!(idle.heard(), Heard::State("ops".into(), false));
}

#[test]
fn a_removal_is_heard() {
    assert_eq!(events()[2].heard(), Heard::Removed("tmp".into()));
}

#[test]
fn an_unknown_change_is_heard_as_nothing() {
    let event = BotEvent { bot: "a".into(), change: Some("renamed".into()), ..BotEvent::default() };
    assert_eq!(event.heard(), Heard::Nothing);
    let created = BotEvent { bot: "a".into(), change: Some("created".into()), ..BotEvent::default() };
    assert_eq!(created.heard(), Heard::Nothing);
}

#[test]
fn an_event_with_none_of_the_three_keys_is_heard_as_nothing() {
    assert_eq!(BotEvent { bot: "a".into(), ..BotEvent::default() }.heard(), Heard::Nothing);
}

#[test]
fn a_bot_event_without_a_bot_does_not_read() {
    assert!(serde_json::from_str::<BotEvent>(r#"{"event":"bot","state":"idle"}"#).is_err());
}

#[test]
fn a_line_heard_twice_is_kept_once() {
    let mut chat = read_chat(plain(&[(1, "you")]));
    assert_eq!(chat.heard(line(2, "bot", "a", false)), Added::Yes);
    assert_eq!(chat.heard(line(2, "bot", "a", false)), Added::No);
    assert_eq!(chat.lines.len(), 2);
}

#[test]
fn lines_heard_out_of_order_are_kept_in_order() {
    let mut chat = Chat::new("prs", 1);
    chat.heard(line(3, "bot", "c", false));
    chat.heard(line(1, "you", "a", false));
    chat.heard(line(2, "step", "b", false));
    assert_eq!(chat.lines.iter().map(|l| l.id).collect::<Vec<_>>(), [1, 2, 3]);
}

#[test]
fn the_history_and_the_lines_heard_while_it_was_read_merge_without_a_double() {
    let mut chat = Chat::new("prs", 1);
    chat.heard(line(3, "bot", "c", false));
    chat.take(plain(&[(1, "you"), (2, "step"), (3, "bot")]));
    assert_eq!(chat.lines.iter().map(|l| l.id).collect::<Vec<_>>(), [1, 2, 3]);
    assert_eq!(chat.read, Read::Done);
}

#[test]
fn a_line_that_skips_ids_asks_for_the_lines_between() {
    let mut chat = read_chat(plain(&[(1, "you"), (2, "step")]));
    assert_eq!(chat.heard(line(5, "bot", "e", false)), Added::Gap(2));
}

#[test]
fn no_gap_is_asked_before_the_history_is_read() {
    let mut chat = Chat::new("prs", 1);
    chat.heard(line(2, "step", "b", false));
    assert_eq!(chat.heard(line(9, "bot", "z", false)), Added::Yes);
}

#[test]
fn only_the_newest_1000_lines_are_kept() {
    let lines: Vec<Line> = (1..=1_005).map(|id| line(id, "step", "s", false)).collect();
    let chat = read_chat(lines);
    assert_eq!((chat.lines.len(), chat.lines[0].id), (KEPT, 6));
}

#[test]
fn earlier_lines_are_said_when_the_first_kept_line_is_not_the_first() {
    assert!(read_chat(plain(&[(4, "you")])).earlier());
    assert!(!read_chat(plain(&[(1, "you")])).earlier());
}

#[test]
fn the_revision_of_a_chat_moves_only_with_a_new_line() {
    let mut chat = read_chat(plain(&[(1, "you")]));
    let before = chat.revision();
    chat.heard(line(1, "you", "x", false));
    assert_eq!(chat.revision(), before);
    chat.heard(line(2, "bot", "y", false));
    assert!(chat.revision() > before);
}

#[test]
fn a_new_server_run_reads_the_chat_again() {
    let mut bots = Bots::new();
    let mut value = app(fixture!("bots/app-two.json"));
    assert!(!bots.take_app(&value).boot_changed);
    assert!(!bots.take_app(&value).boot_changed);
    value["boot"] = json!("other-boot");
    assert!(bots.take_app(&value).boot_changed);
}

fn steps(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("s{i}")).collect()
}

#[test]
fn three_steps_or_fewer_stay_in_view() {
    for n in 1..=STEPS_SHOWN {
        assert_eq!(folded(&steps(n), false), (0, &steps(n)[..]));
    }
}

#[test]
fn more_than_three_steps_fold_to_the_last_three() {
    let all = steps(5);
    assert_eq!(folded(&all, false), (2, &all[2..]));
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
    assert_eq!(folded(&all, true), (0, &all[..]));
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
    assert_eq!(turn(&lines, true), Some(entries::Turn { since: 1002, steps: 2 }));
}

#[test]
fn a_turn_started_by_held_messages_starts_at_the_answer_before_it() {
    let lines = vec![
        line(1, "you", "go", false),
        line(2, "you", "more", true),
        line(3, "bot", "a", false),
        line(4, "step", "b", false),
    ];
    assert_eq!(turn(&lines, true), Some(entries::Turn { since: 1003, steps: 1 }));
}

#[test]
fn an_idle_bot_has_no_turn() {
    assert_eq!(turn(&plain(&[(1, "you")]), false), None);
}

fn place() -> crate::model::MachinePlace {
    let seen = crate::model::MachineSeen::default();
    crate::model::MachinePlace::at(&seen, &crate::model::ThereWorkspace::default(), None)
}

#[test]
fn showing_a_bot_leaves_a_machines_place() {
    let shown = Showing::Machine(place()).after(Moved::Bot("prs".into()));
    assert_eq!((shown.bot(), shown.machine()), (Some("prs"), None));
}

#[test]
fn picking_a_machines_place_leaves_the_chat() {
    assert_eq!(Showing::Bot("prs".into()).after(Moved::Machine(place())).bot(), None);
}

#[test]
fn a_move_to_a_local_place_leaves_the_chat() {
    assert_eq!(Showing::Bot("prs".into()).after(Moved::Local), Showing::Local);
}

#[test]
fn the_removal_of_the_shown_bot_leaves_its_chat() {
    assert_eq!(Showing::Bot("prs".into()).after(Moved::Removed("prs".into())), Showing::Local);
}

#[test]
fn the_removal_of_another_bot_keeps_the_chat() {
    assert_eq!(Showing::Bot("prs".into()).after(Moved::Removed("ops".into())).bot(), Some("prs"));
}

#[test]
fn a_list_without_the_shown_bot_leaves_its_chat() {
    let shown = Showing::Bot("prs".into());
    assert_eq!(shown.clone().after(Moved::Listed(vec!["ops".into()])), Showing::Local);
    assert_eq!(shown.after(Moved::Listed(vec!["prs".into()])).bot(), Some("prs"));
}

#[test]
fn seen_is_due_for_the_shown_chat_with_unread_answers_in_the_active_window() {
    assert_eq!(two().seen_due(Some("prs")), Some("prs".to_string()));
}

#[test]
fn no_seen_while_the_window_is_behind() {
    let mut bots = two();
    bots.active = false;
    assert_eq!(bots.seen_due(Some("prs")), None);
}

#[test]
fn no_seen_for_a_bot_that_is_not_shown() {
    let bots = two();
    assert_eq!(bots.seen_due(None), None);
    assert_eq!(bots.seen_due(Some("ops")), None);
}

#[test]
fn seen_goes_once_per_count() {
    let mut bots = two();
    bots.seen = Some(("prs".into(), 2));
    assert_eq!(bots.seen_due(Some("prs")), None);
    let mut value = app(fixture!("bots/app-two.json"));
    value["bots"][0]["unread"] = json!(3);
    bots.take_app(&value);
    assert_eq!(bots.seen_due(Some("prs")), Some("prs".to_string()));
}

#[test]
fn a_count_that_returns_to_the_same_number_after_none_asks_again() {
    let mut bots = two();
    bots.seen = Some(("prs".into(), 2));
    let mut value = app(fixture!("bots/app-two.json"));
    value["bots"][0]["unread"] = json!(0);
    bots.take_app(&value);
    value["bots"][0]["unread"] = json!(2);
    bots.take_app(&value);
    assert_eq!(bots.seen_due(Some("prs")), Some("prs".to_string()));
}

#[test]
fn a_step_changes_no_count_and_sends_no_seen() {
    let mut bots = two();
    bots.seen = Some(("prs".into(), 2));
    let mut chat = read_chat(plain(&[(1, "you")]));
    chat.heard(line(2, "step", "s", false));
    assert_eq!(bots.seen_due(Some("prs")), None);
}

#[test]
fn the_list_revision_moves_when_a_name_a_state_or_an_unread_count_changes() {
    for (key, value) in [("name", json!("renamed")), ("state", json!("idle")), ("unread", json!(7))] {
        let mut bots = two();
        let before = bots.revision();
        let mut line = app(fixture!("bots/app-two.json"));
        line["bots"][1][key] = value;
        bots.take_app(&line);
        assert!(bots.revision() > before, "{key}");
    }
}

#[test]
fn the_same_app_line_again_keeps_the_revision() {
    let mut bots = two();
    let before = bots.revision();
    bots.take_app(&app(fixture!("bots/app-two.json")));
    assert_eq!(bots.revision(), before);
}

#[test]
fn a_state_event_holds_until_the_next_app_line() {
    let mut bots = two();
    assert!(bots.state_heard("ops", false));
    assert!(!bots.find("ops").expect("ops").thinking());
    assert!(!bots.state_heard("ops", false));
    bots.take_app(&app(fixture!("bots/app-two.json")));
    assert!(bots.find("ops").expect("ops").thinking());
}

fn lacking(has: u64) -> Lacks<'static> {
    Lacks::ThisComputer(Some(Version::new(0, 13, has)))
}

#[test]
fn an_older_command_line_hides_the_rows_and_says_what_it_needs() {
    assert_eq!(
        two().offer(Some(lacking(73)), Os::Mac),
        Offer::Lacks("Showing your bots needs cornercase 0.13.77 or newer on this Mac (it has 0.13.73).".into())
    );
}

#[test]
fn an_unread_command_line_release_shows_the_rows() {
    assert_eq!(two().offer(None, Os::Mac), Offer::Rows);
}

#[test]
fn a_server_without_bots_offers_nothing() {
    assert_eq!(Bots::new().offer(Some(lacking(73)), Os::Mac), Offer::Hidden);
}

#[test]
fn a_command_line_release_that_could_not_be_read_counts_as_older() {
    let none = Some(Lacks::ThisComputer(None));
    assert_eq!(
        two().offer(none, Os::Linux),
        Offer::Lacks("Showing your bots needs cornercase 0.13.77 or newer on this computer.".into())
    );
}

fn key_of(entries: &[Entry]) -> Vec<EntryKey> {
    keys(entries, &std::collections::HashSet::new())
}

#[test]
fn the_list_splices_only_the_entries_that_changed() {
    let old = key_of(&entries(&plain(&[(1, "you"), (2, "bot"), (3, "you")])));
    let new = key_of(&entries(&plain(&[(1, "you"), (2, "bot"), (3, "you"), (4, "bot")])));
    assert_eq!(spliced(&old, &new), vec![(3..3, 1)]);
    assert_eq!(spliced(&old, &old), Vec::new());
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
    assert_eq!(spliced(&old, &new), vec![(1..2, 2)]);
}
