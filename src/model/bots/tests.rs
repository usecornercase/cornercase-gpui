use serde_json::{Value, json};

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

#[test]
fn an_empty_chat_asks_for_everything_before_a_line_that_is_not_the_first() {
    let mut chat = read_chat(Vec::new());
    assert_eq!(chat.heard(line(4, "bot", "late", false)), Added::Gap(0));
    let mut fresh = read_chat(Vec::new());
    assert_eq!(fresh.heard(line(1, "you", "first", false)), Added::Yes);
}

#[test]
fn a_bot_that_stops_thinking_or_gets_an_answer_is_settled() {
    let mut bots = two();
    let mut value = app(fixture!("bots/app-two.json"));
    value["bots"][1]["state"] = json!("idle");
    assert_eq!(bots.take_app(&value).settled, vec!["ops".to_string()]);
    value["bots"][0]["unread"] = json!(3);
    assert_eq!(bots.take_app(&value).settled, vec!["prs".to_string()]);
    assert_eq!(bots.take_app(&value).settled, Vec::<String>::new());
}

#[test]
fn each_new_answer_asks_for_seen_again_even_at_the_same_count() {
    let mut bots = two();
    bots.chat = Some(read_chat(plain(&[(1, "you")])));
    bots.seen = Some(("prs".into(), 2));
    assert_eq!(bots.seen_due(Some("prs")), None);
    bots.heard("prs", line(2, "step", "s", false));
    assert_eq!(bots.seen_due(Some("prs")), None);
    bots.heard("prs", line(3, "bot", "answer", false));
    assert_eq!(bots.seen_due(Some("prs")), Some("prs".to_string()));
}

#[test]
fn showing_a_chat_again_asks_for_seen_again() {
    let mut bots = two();
    bots.seen = Some(("prs".into(), 2));
    bots.forget_seen();
    assert_eq!(bots.seen_due(Some("prs")), Some("prs".to_string()));
}

#[test]
fn a_line_for_a_chat_that_is_not_shown_changes_nothing() {
    let mut bots = two();
    bots.chat = Some(read_chat(plain(&[(1, "you")])));
    assert_eq!(bots.heard("ops", line(2, "bot", "x", false)), None);
}

#[test]
fn a_bot_without_an_id_is_left_out() {
    let list = in_app(&json!({"bots": [{"id": "", "name": "ghost", "agent": "claude"}, {"id": "ops", "name": "ops"}]}));
    assert_eq!(list.iter().map(|b| b.id.as_str()).collect::<Vec<_>>(), ["ops"]);
}

#[test]
fn an_answer_to_an_earlier_ask_or_another_bot_is_not_awaited() {
    let chat = Chat::new("prs", 3);
    assert!(chat.awaits("prs", 3));
    assert!(!chat.awaits("prs", 2));
    assert!(!chat.awaits("ops", 3));
}

#[test]
fn a_chat_is_shown_only_when_rows_are_offered_and_the_bot_is_listed() {
    let bots = two();
    assert!(bots.can_show("prs", None, Os::Mac));
    assert!(!bots.can_show("nobody", None, Os::Mac));
    assert!(!bots.can_show("prs", Some(lacking(73)), Os::Mac));
    assert!(!Bots::new().can_show("prs", None, Os::Mac));
}
